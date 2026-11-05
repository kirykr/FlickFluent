//! Dub, language and benchmark commands (feature 002). The dub commands drive
//! `flickfluent_core::pipeline`; the language and benchmark commands return `NOT_IMPLEMENTED`
//! until the story that owns them lands, so the UI can be built against the final names.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use flickfluent_core::error::{
    AppError, NOTHING_TO_RESUME, NOT_IMPLEMENTED, RUN_NOT_COMPLETE, SAME_LANGUAGE,
};
use flickfluent_core::hardware;
use flickfluent_core::models::{bundled_catalogue, installed};
use flickfluent_core::pipeline::ffmpeg::{Ffmpeg, SourceInfo};
use flickfluent_core::pipeline::plan::{machine_class, plan_models_full};
use flickfluent_core::pipeline::preflight::{check_start, preflight, Preflight};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunConfig, RunEvent};
use flickfluent_core::pipeline::stages::{
    export, fingerprint, probe_source, ExportRequest, ExportResult,
};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{
    read_from, write_into, DubData, ExportKind, Options, PipelineRun, Quality, RunState, Source,
};
use flickfluent_core::sidecar::SidecarConfig;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use super::models::models_dir;
use super::project::{handle_of, ProjectHandle, ProjectState};
use super::settings::SettingsState;

/// Names of the events the dub pipeline sends. Payloads are in `contracts/ipc-commands.md`.
pub mod event {
    pub const PROGRESS: &str = "dub:progress";
    pub const STAGE: &str = "dub:stage";
    pub const LINE: &str = "dub:line";
    pub const ERROR: &str = "dub:error";
    pub const DONE: &str = "dub:done";
    pub const DETECTED: &str = "dub:detected";
    /// Sent by the benchmark story (US5).
    #[allow(dead_code)]
    pub const BENCH_PROGRESS: &str = "bench:progress";
}

/// Lines go out at most this often, so a long transcript does not flood the window.
const LINE_EVERY: Duration = Duration::from_millis(200);

struct Active {
    handle: String,
    cancel: CancelToken,
}

/// Runs in progress, by run id.
#[derive(Default)]
pub struct DubState {
    runs: Mutex<HashMap<String, Active>>,
}

impl DubState {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Active>> {
        self.runs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn running_for(&self, handle: &str) -> bool {
        self.lock().values().any(|a| a.handle == handle)
    }
}

fn not_implemented<T>() -> Result<T, AppError> {
    Err(AppError::new(NOT_IMPLEMENTED, "dub.error.notImplemented"))
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "macos"
    }
}

fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    }
}

/// The sidecar launcher: bundled with the app, or in the source tree during development.
fn launcher(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join("sidecar").join("launcher.py");
        if bundled.exists() {
            return bundled;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../sidecar/launcher.py")
}

/// Arguments that start the sidecar, allowed to read and write the given folders.
pub fn sidecar_args(app: &tauri::AppHandle, allow: &[&Path]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![launcher(app).into_os_string()];
    for dir in allow {
        args.push("--allow".into());
        args.push(dir.as_os_str().to_owned());
    }
    args
}

fn ffmpeg_for(models: &Path) -> Result<Ffmpeg, AppError> {
    Ffmpeg::locate(models).ok_or_else(|| {
        let missing: Vec<String> = bundled_catalogue()
            .into_iter()
            .filter(|e| e.step == flickfluent_core::models::Step::Ffmpeg)
            .filter(|e| {
                e.platforms
                    .as_ref()
                    .is_none_or(|p| p.supports(os_name(), arch_name()))
            })
            .map(|e| e.id)
            .collect();
        AppError::new(
            flickfluent_core::error::MODELS_MISSING,
            "dub.error.modelsMissing",
        )
        .with_details(json!({ "missing": missing }))
    })
}

/// The project data, its file, and its stable id (not the window handle).
fn dub_of(state: &ProjectState, handle: &str) -> Result<(DubData, PathBuf, String), AppError> {
    state.with(handle, |s| {
        Ok((
            read_from(&s.project.data),
            s.path.clone(),
            s.project.manifest.id.to_string(),
        ))
    })
}

fn store(state: &ProjectState, handle: &str, dub: &DubData) -> Result<(), AppError> {
    state.with(handle, |s| {
        let mut data = s.project.data.clone();
        write_into(&mut data, dub);
        s.set_data(data);
        Ok(())
    })
}

// --- benchmark (US5): stub --------------------------------------------

#[tauri::command]
pub fn bench_run(fixture_id: String, simulated: Option<bool>) -> Result<String, AppError> {
    let _ = (fixture_id, simulated);
    not_implemented()
}

#[tauri::command]
pub fn bench_records() -> Result<Value, AppError> {
    not_implemented()
}

// --- dub -----------------------------------------------------------------------------------

#[tauri::command]
pub async fn dub_probe_source(
    app: tauri::AppHandle,
    settings: tauri::State<'_, SettingsState>,
    path: String,
) -> Result<SourceInfo, AppError> {
    let models = models_dir(&app, &settings)?;
    probe_source(&ffmpeg_for(&models)?, Path::new(&path)).await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsInput {
    #[serde(default)]
    mute_original: bool,
    #[serde(default)]
    export_kind: ExportKind,
    #[serde(default)]
    export_name: String,
    #[serde(default)]
    quality: Quality,
    #[serde(default)]
    fallbacks: flickfluent_core::project::dub::FallbackChoices,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn dub_configure(
    app: tauri::AppHandle,
    projects: tauri::State<'_, ProjectState>,
    dubs: tauri::State<'_, DubState>,
    settings: tauri::State<'_, SettingsState>,
    handle: String,
    source: String,
    source_language: String,
    target_language: String,
    options: OptionsInput,
    confirm_same: Option<bool>,
) -> Result<ProjectHandle, AppError> {
    if dubs.running_for(&handle) {
        return Err(AppError::new(
            flickfluent_core::error::IN_USE,
            "error.inUse",
        ));
    }
    if source_language != "auto" && source_language == target_language && confirm_same != Some(true)
    {
        return Err(AppError::new(SAME_LANGUAGE, "dub.error.sameLanguage"));
    }
    let models = models_dir(&app, &settings)?;
    let info = probe_source(&ffmpeg_for(&models)?, Path::new(&source)).await?;
    let fp = fingerprint(Path::new(&source))?;
    let (mut dub, _, _) = dub_of(&projects, &handle)?;
    let changed = dub.source.as_ref().map(|s| s.fingerprint.as_str()) != Some(fp.as_str())
        || dub.source_language != source_language
        || dub.target_language != target_language;
    dub.source = Some(Source {
        path: source,
        duration_ms: info.duration_ms,
        has_audio: info.has_audio,
        has_video: info.has_video,
        size_bytes: info.size_bytes,
        fingerprint: fp,
    });
    dub.source_language = source_language;
    dub.target_language = target_language;
    dub.options = Options {
        mute_original: options.mute_original,
        export_kind: options.export_kind,
        export_name: options.export_name,
        quality: options.quality,
        fallbacks: options.fallbacks,
        confirm_same: confirm_same == Some(true),
    };
    if changed {
        dub.detected_language = None;
    }
    store(&projects, &handle, &dub)?;
    projects.with(&handle, |s| Ok(handle_of(&handle, s)))
}

fn plan_for(
    dub: &DubData,
    models: &Path,
) -> (
    flickfluent_core::pipeline::plan::ModelPlan,
    flickfluent_core::models::MachineClass,
    String,
) {
    let machine = hardware::probe();
    let class = machine_class(&machine);
    let source = if dub.source_language == "auto" {
        dub.detected_language
            .clone()
            .unwrap_or_else(|| "auto".into())
    } else {
        dub.source_language.clone()
    };
    let have: Vec<String> = installed(models).into_iter().map(|m| m.id).collect();
    let plan = plan_models_full(
        &bundled_catalogue(),
        class,
        os_name(),
        arch_name(),
        &source,
        &dub.target_language,
        dub.options.quality == Quality::Fast,
        &have,
        &dub.options.fallbacks,
    );
    (plan, class, source)
}

fn preflight_for(state: &ProjectState, handle: &str, models: &Path) -> Result<Preflight, AppError> {
    let (dub, path, _) = dub_of(state, handle)?;
    let source = dub.source.clone().ok_or_else(|| {
        AppError::new(
            flickfluent_core::error::INVALID_PROJECT,
            "error.invalidProject",
        )
    })?;
    let (plan, class, src) = plan_for(&dub, models);
    let work = WorkFolder::folder_for(&path);
    Ok(preflight(
        &bundled_catalogue(),
        &plan,
        models,
        &work,
        class,
        os_name(),
        arch_name(),
        &src,
        &dub.target_language,
        dub.options.quality == Quality::Fast,
        source.duration_ms,
    ))
}

#[tauri::command]
pub fn dub_preflight(
    app: tauri::AppHandle,
    projects: tauri::State<ProjectState>,
    settings: tauri::State<SettingsState>,
    handle: String,
) -> Result<Preflight, AppError> {
    let models = models_dir(&app, &settings)?;
    preflight_for(&projects, &handle, &models)
}

fn payload(run_id: &str, event: &RunEvent) -> Option<(&'static str, Value)> {
    Some(match event {
        RunEvent::Stage { stage, status } => (
            event::STAGE,
            json!({ "runId": run_id, "stage": stage, "status": status }),
        ),
        RunEvent::Progress {
            stage,
            stage_progress,
            overall,
            section_index,
            section_count,
            eta_seconds,
        } => (
            event::PROGRESS,
            json!({
                "runId": run_id, "stage": stage, "stageProgress": stage_progress, "overallProgress": overall,
                "sectionIndex": section_index, "sectionCount": section_count, "etaSeconds": eta_seconds,
            }),
        ),
        RunEvent::Error(e) => (
            event::ERROR,
            json!({
                "runId": run_id, "code": e.code, "stage": e.stage, "sectionIndex": e.section_index,
                "message": e.message_key, "nextStep": e.next_step_key, "details": e.details,
            }),
        ),
        // `dub:done` is sent by the command after the project is saved, so the screen that
        // reloads on it sees the finished lines and a run that is no longer active.
        RunEvent::Detected(language) => (
            event::DETECTED,
            json!({ "runId": run_id, "language": language }),
        ),
        RunEvent::Done(_) => return None,
        RunEvent::Line(_) => return None, // sent by the line queue
    })
}

fn build_config(
    app: &tauri::AppHandle,
    models: &Path,
    project_path: &Path,
    project_id: &str,
    work: &Path,
) -> RunConfig {
    let machine = hardware::probe();
    let allow: Vec<&Path> = vec![models, work];
    RunConfig {
        project_path: project_path.to_path_buf(),
        project_id: project_id.to_string(),
        models_dir: models.to_path_buf(),
        catalogue: bundled_catalogue(),
        class: machine_class(&machine),
        os: os_name().into(),
        arch: arch_name().into(),
        ffmpeg: Ffmpeg::locate(models).unwrap_or_else(|| Ffmpeg::at(Path::new(""))),
        sidecar: SidecarConfig {
            program: PathBuf::new(), // the installed runtime's Python
            args: sidecar_args(app, &allow),
            hang_timeout: Duration::from_secs(60),
            start_timeout: Duration::from_secs(120),
        },
        code_map: serde_json::from_str(include_str!("../../../config/language-codes.json"))
            .unwrap_or(json!({})),
        params: json!({
            "device": "cpu",
            "computeType": "int8",
            "threads": machine.cpu_cores.unwrap_or(4).saturating_sub(1).max(1),
        }),
    }
}

fn start_run(app: tauri::AppHandle, handle: String, mode: Mode) -> Result<String, AppError> {
    let projects = app.state::<ProjectState>();
    let dubs = app.state::<DubState>();
    let settings = app.state::<SettingsState>();
    if dubs.running_for(&handle) {
        return Err(AppError::new(
            flickfluent_core::error::IN_USE,
            "error.inUse",
        ));
    }
    let models = models_dir(&app, &settings)?;
    let (dub, path, project_id) = dub_of(&projects, &handle)?;
    if mode == Mode::Fresh {
        check_start(&preflight_for(&projects, &handle, &models)?)?;
    } else if WorkFolder::open_for(&path, &project_id).is_none() {
        return Err(AppError::new(
            NOTHING_TO_RESUME,
            "dub.error.nothingToResume",
        ));
    }
    let work = WorkFolder::folder_for(&path);
    std::fs::create_dir_all(&work).map_err(AppError::io)?;
    let cfg = build_config(&app, &models, &path, &project_id, &work);

    let run_id = uuid::Uuid::new_v4().to_string();
    let cancel = CancelToken::default();
    dubs.lock().insert(
        run_id.clone(),
        Active {
            handle: handle.clone(),
            cancel: cancel.clone(),
        },
    );

    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<flickfluent_core::project::dub::TranscriptLine>();
    let emit_app = app.clone();
    let emit_id = run_id.clone();
    let lines_task = tauri::async_runtime::spawn(async move {
        while let Some(line) = rx.recv().await {
            let _ = emit_app.emit(event::LINE, json!({ "runId": emit_id, "line": line }));
            tokio::time::sleep(LINE_EVERY).await;
        }
    });

    let task_app = app.clone();
    let task_id = run_id.clone();
    let project_path = path.clone();
    let owner_id = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let sink_app = task_app.clone();
        let sink_id = task_id.clone();
        let result = run_dub(&cfg, dub, mode, cancel, &mut move |e| {
            if let RunEvent::Line(l) = &e {
                let _ = tx.send(l.clone());
            } else if let Some((name, body)) = payload(&sink_id, &e) {
                let _ = sink_app.emit(name, body);
            }
        })
        .await;
        let _ = lines_task.await; // drain queued lines before reporting the end
        let projects = task_app.state::<ProjectState>();
        let state = match result {
            Ok(dub) => {
                let state = dub.run.as_ref().map_or(RunState::Failed, |r| r.state);
                let _ = store(&projects, &handle, &dub);
                state
            }
            // The runner already sent `dub:error` with the details; the run file has the state.
            Err(_) => WorkFolder::open_for(&project_path, &owner_id)
                .and_then(|w| w.load_run().ok().flatten())
                .map_or(RunState::Failed, |r| r.state),
        };
        task_app.state::<DubState>().lock().remove(&task_id);
        let _ = task_app.emit(event::DONE, json!({ "runId": task_id, "state": state }));
    });
    Ok(run_id)
}

#[tauri::command]
pub fn dub_start(
    app: tauri::AppHandle,
    handle: String,
    fallback_choices: Option<Value>,
) -> Result<String, AppError> {
    let _ = fallback_choices; // fallbacks arrive with the language story (US2)
    start_run(app, handle, Mode::Fresh)
}

#[tauri::command]
pub fn dub_resume(app: tauri::AppHandle, handle: String) -> Result<String, AppError> {
    start_run(app, handle, Mode::Resume)
}

#[tauri::command]
pub fn dub_cancel(dubs: tauri::State<DubState>, run_id: String) -> Result<(), AppError> {
    if let Some(active) = dubs.lock().get(&run_id) {
        active.cancel.cancel();
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    run: Option<PipelineRun>,
    /// A run was cut short (the app closed or crashed) and can be resumed.
    interrupted: bool,
    running: bool,
    /// Languages, options, speakers and lines as saved in the project.
    dub: DubData,
}

#[tauri::command]
pub fn dub_status(
    projects: tauri::State<ProjectState>,
    dubs: tauri::State<DubState>,
    handle: String,
) -> Result<Status, AppError> {
    let (dub, path, project_id) = dub_of(&projects, &handle)?;
    let work = WorkFolder::open_for(&path, &project_id);
    let running = dubs.running_for(&handle);
    let run = work
        .as_ref()
        .and_then(|w| w.load_run().ok().flatten())
        .or(dub.run.clone());
    // The project may have been closed before it saved a finished run's lines: read them back.
    let mut dub = dub;
    if dub.lines.is_empty() && run.as_ref().is_some_and(|r| r.state == RunState::Completed) {
        if let (Some(w), Some(r)) = (&work, &run) {
            dub.lines = w.load_lines(r);
        }
    }
    let interrupted = !running && work.as_ref().is_some_and(|w| w.is_interrupted());
    Ok(Status {
        run,
        interrupted,
        running,
        dub,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn dub_export(
    app: tauri::AppHandle,
    projects: tauri::State<'_, ProjectState>,
    settings: tauri::State<'_, SettingsState>,
    handle: String,
    kind: ExportKind,
    file_name: String,
    folder: String,
    overwrite: Option<bool>,
) -> Result<ExportResult, AppError> {
    let models = models_dir(&app, &settings)?;
    let (dub, path, project_id) = dub_of(&projects, &handle)?;
    let source = dub
        .source
        .ok_or_else(|| AppError::new(RUN_NOT_COMPLETE, "dub.error.runNotComplete"))?;
    let work = WorkFolder::open_for(&path, &project_id)
        .ok_or_else(|| AppError::new(RUN_NOT_COMPLETE, "dub.error.runNotComplete"))?;
    let finished = work
        .load_run()?
        .is_some_and(|r| r.state == RunState::Completed);
    if !finished {
        return Err(AppError::new(RUN_NOT_COMPLETE, "dub.error.runNotComplete"));
    }
    if kind == ExportKind::Subtitles {
        // Subtitles come from the saved lines, so they also work for a run that did not speak.
        let lines = match dub.lines.is_empty() {
            false => dub.lines,
            true => work.load_lines(&work.load_run()?.unwrap_or_default()),
        };
        return flickfluent_core::pipeline::stages::export_subtitles(
            &lines,
            Path::new(&folder),
            &file_name,
            overwrite.unwrap_or(false),
        );
    }
    let mix = work.path().join("dub-mix.wav");
    if !mix.exists() {
        return Err(AppError::new(RUN_NOT_COMPLETE, "dub.error.runNotComplete"));
    }
    let ffmpeg = ffmpeg_for(&models)?;
    export(
        &ffmpeg,
        &ExportRequest {
            source: Path::new(&source.path),
            mix: &mix,
            folder: Path::new(&folder),
            kind,
            file_name: &file_name,
            overwrite: overwrite.unwrap_or(false),
            duration_ms: source.duration_ms,
        },
        &mut |_| {},
    )
    .await
}

#[tauri::command]
pub fn dub_discard_run(
    projects: tauri::State<ProjectState>,
    dubs: tauri::State<DubState>,
    handle: String,
) -> Result<(), AppError> {
    if dubs.running_for(&handle) {
        return Err(AppError::new(
            flickfluent_core::error::IN_USE,
            "error.inUse",
        ));
    }
    let (mut dub, path, project_id) = dub_of(&projects, &handle)?;
    if let Some(work) = WorkFolder::open_for(&path, &project_id) {
        std::fs::remove_dir_all(work.path()).map_err(AppError::io)?;
    }
    dub.run = None;
    dub.lines.clear();
    store(&projects, &handle, &dub)
}

/// Ids of models installed in the models folder, for the New dub screen's "what is missing" list.
#[tauri::command]
pub fn dub_installed_models(
    app: tauri::AppHandle,
    settings: tauri::State<SettingsState>,
) -> Result<Vec<String>, AppError> {
    Ok(installed(&models_dir(&app, &settings)?)
        .into_iter()
        .map(|m| m.id)
        .collect())
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use flickfluent_core::error::{AppError, IN_USE};
use flickfluent_core::models::{
    bundled_catalogue, delete_model, install_entry, installed, part_bytes, preflight_space,
    CatalogueEntry, Control, Outcome, Phase, Progress, Target,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

use super::settings::SettingsState;

#[derive(Clone)]
enum RunState {
    Downloading,
    Verifying,
    Failed(String),
}

struct Job {
    control: Control,
    state: RunState,
    bytes_done: u64,
}

/// Downloads running (or failed) in this session. Anything not here is derived from disk.
#[derive(Default)]
pub struct ModelsState {
    jobs: Mutex<HashMap<String, Job>>,
}

impl ModelsState {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Job>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecord {
    id: String,
    name: String,
    purpose: String,
    licence: String,
    /// `permissive`, `copyleftFile` or `nonCommercial`.
    licence_class: flickfluent_core::models::LicenceClass,
    /// i18n key of the notice to show before download, when the licence needs one.
    licence_notice_key: Option<String>,
    size_bytes: u64,
    /// `notInstalled`, `downloading`, `paused`, `verifying`, `installed` or `failed`.
    state: &'static str,
    bytes_done: u64,
    error_code: Option<String>,
}

pub fn models_dir(app: &tauri::AppHandle, settings: &SettingsState) -> Result<PathBuf, AppError> {
    if let Some(folder) = settings.models_folder() {
        return Ok(PathBuf::from(folder));
    }
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))
}

fn find(id: &str) -> Result<CatalogueEntry, AppError> {
    bundled_catalogue()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::new("NOT_FOUND", "error.notFound"))
}

fn record(
    entry: &CatalogueEntry,
    dir: &std::path::Path,
    jobs: &HashMap<String, Job>,
) -> ModelRecord {
    let installed_here = installed(dir).iter().any(|m| m.id == entry.id);
    let (state, bytes_done, error_code) = if installed_here {
        ("installed", entry.total_download_bytes(), None)
    } else if let Some(job) = jobs.get(&entry.id) {
        match &job.state {
            RunState::Downloading => ("downloading", job.bytes_done, None),
            RunState::Verifying => ("verifying", entry.size_bytes, None),
            RunState::Failed(code) => ("failed", part_bytes(dir, entry), Some(code.clone())),
        }
    } else {
        match part_bytes(dir, entry) {
            0 => ("notInstalled", 0, None),
            n => ("paused", n, None),
        }
    };
    ModelRecord {
        id: entry.id.clone(),
        name: entry.name.clone(),
        purpose: entry.purpose.clone(),
        licence: entry.licence.clone(),
        licence_class: entry.licence_class,
        licence_notice_key: entry.licence_notice_key.clone(),
        size_bytes: entry.total_download_bytes(),
        state,
        bytes_done,
        error_code,
    }
}

fn emit(app: &tauri::AppHandle, rec: &ModelRecord) {
    let _ = app.emit("model:progress", rec);
}

#[tauri::command]
pub fn models_catalogue(
    app: tauri::AppHandle,
    state: tauri::State<ModelsState>,
    settings: tauri::State<SettingsState>,
) -> Result<Vec<ModelRecord>, AppError> {
    let dir = models_dir(&app, &settings)?;
    let jobs = state.lock();
    Ok(bundled_catalogue()
        .iter()
        .map(|e| record(e, &dir, &jobs))
        .collect())
}

/// Start or resume a download. Disk space is checked here so the user hears about it at once.
#[tauri::command]
pub fn model_download_start(
    app: tauri::AppHandle,
    state: tauri::State<ModelsState>,
    settings: tauri::State<SettingsState>,
    id: String,
) -> Result<(), AppError> {
    let entry = find(&id)?;
    let dir = models_dir(&app, &settings)?;
    if state
        .lock()
        .get(&id)
        .is_some_and(|j| !matches!(j.state, RunState::Failed(_)))
    {
        return Ok(()); // already running
    }
    preflight_space(
        entry.size_bytes.saturating_sub(part_bytes(&dir, &entry)),
        &dir,
    )?;

    let control = Control::new();
    state.lock().insert(
        id.clone(),
        Job {
            control: control.clone(),
            state: RunState::Downloading,
            bytes_done: part_bytes(&dir, &entry),
        },
    );
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        run_download(handle, entry, dir, control).await;
    });
    Ok(())
}

async fn run_download(
    app: tauri::AppHandle,
    entry: CatalogueEntry,
    dir: PathBuf,
    control: Control,
) {
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut last_phase = Phase::Downloading;
    let progress_app = app.clone();
    let progress_entry = entry.clone();
    let progress_dir = dir.clone();
    let work = dir.join("work");
    let target = Target {
        models_dir: &dir,
        os: if cfg!(windows) { "windows" } else { "macos" },
        arch: if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        },
        sidecar_args: super::dub::sidecar_args(&app, &[&dir, &work]),
    };
    let result = install_entry(&entry, &target, &control, &mut move |p: Progress| {
        let state = progress_app.state::<ModelsState>();
        let mut jobs = state.lock();
        if let Some(job) = jobs.get_mut(&progress_entry.id) {
            job.bytes_done = p.bytes_done;
            job.state = match p.phase {
                Phase::Downloading => RunState::Downloading,
                Phase::Verifying => RunState::Verifying,
            };
        }
        // The UI needs about five updates a second, not one per chunk.
        if last_emit.elapsed() >= Duration::from_millis(200) || p.phase != last_phase {
            last_emit = Instant::now();
            last_phase = p.phase;
            emit(
                &progress_app,
                &record(&progress_entry, &progress_dir, &jobs),
            );
        }
    })
    .await;

    let state = app.state::<ModelsState>();
    match result {
        Ok(Outcome::Completed(_)) => {
            // `install_entry` has recorded it.
            state.lock().remove(&entry.id);
        }
        // Paused: the .part file on disk is the state. Cancelled: nothing is left.
        Ok(Outcome::Paused | Outcome::Cancelled) => {
            state.lock().remove(&entry.id);
        }
        Err(e) => {
            if let Some(job) = state.lock().get_mut(&entry.id) {
                job.state = RunState::Failed(e.code.to_string());
            }
        }
    }
    let rec = record(&entry, &dir, &state.lock());
    emit(&app, &rec);
}

#[tauri::command]
pub fn model_download_pause(state: tauri::State<ModelsState>, id: String) {
    if let Some(job) = state.lock().get(&id) {
        job.control.pause();
    }
}

#[tauri::command]
pub fn model_download_resume(
    app: tauri::AppHandle,
    state: tauri::State<ModelsState>,
    settings: tauri::State<SettingsState>,
    id: String,
) -> Result<(), AppError> {
    model_download_start(app, state, settings, id)
}

#[tauri::command]
pub fn model_download_cancel(
    app: tauri::AppHandle,
    state: tauri::State<ModelsState>,
    settings: tauri::State<SettingsState>,
    id: String,
) -> Result<(), AppError> {
    let running = state
        .lock()
        .get(&id)
        .filter(|j| !matches!(j.state, RunState::Failed(_)))
        .map(|j| j.control.clone());
    if let Some(control) = running {
        control.cancel(); // the download task removes the part file and clears the job
        return Ok(());
    }
    // Paused or failed: nothing is running, so remove the leftovers here.
    let entry = find(&id)?;
    let dir = models_dir(&app, &settings)?;
    state.lock().remove(&id);
    delete_model(&dir, &entry)?;
    emit(&app, &record(&entry, &dir, &state.lock()));
    Ok(())
}

#[tauri::command]
pub fn model_delete(
    app: tauri::AppHandle,
    state: tauri::State<ModelsState>,
    settings: tauri::State<SettingsState>,
    id: String,
) -> Result<(), AppError> {
    let entry = find(&id)?;
    let dir = models_dir(&app, &settings)?;
    if state
        .lock()
        .get(&id)
        .is_some_and(|j| !matches!(j.state, RunState::Failed(_)))
    {
        return Err(AppError::new(IN_USE, "error.inUse"));
    }
    state.lock().remove(&id);
    delete_model(&dir, &entry)?;
    emit(&app, &record(&entry, &dir, &state.lock()));
    Ok(())
}

//! The dub run: extract, then per section separate, transcribe, translate and synthesize, then
//! mix. Every result is a file in the work folder and `run-state.json` is saved after each
//! section stage, so a killed run resumes with at most one section redone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::errors::next_step_key;
use super::ffmpeg::Ffmpeg;
use super::mix::{mix, MixLine, MixSection};
use super::plan::{disk_needed_bytes, plan_models_full, ModelPlan, VoicePick};
use super::progress::{Eta, Pacer};
use super::sections::plan as plan_sections;
use super::stages::{extract, fingerprint};
use super::state::{input_hash, WorkFolder};
use crate::cloud::engine::{CloudEngine, SpeakJob};
use crate::cloud::estimate::estimate_chars;
use crate::cloud::rotation::PoolEvent;
use crate::cloud::runtime::CloudRuntime;
use crate::cloud::services::{Gender as CloudGender, Step as CloudStep};
use crate::error::{
    AppError, CANCELLED, CLOUD_CONSENT_REQUIRED, CLOUD_NO_KEY, MODELS_MISSING, NOTHING_TO_RESUME,
    NO_SPACE, NO_SPEECH_FOUND, SAME_LANGUAGE, SOURCE_CHANGED, STAGE_FAILED,
};
use crate::models::{CatalogueEntry, Gender, MachineClass};
use crate::project::dub::{
    CloudUsage, DubData, LineFlag, LineStatus, PipelineRun, Quality, RunError, RunState, Section,
    Speaker, SpeakerGender, Stage, StageState, StageStatus, TranscriptLine,
};
use crate::sidecar::{Event, Request, Sidecar, SidecarConfig};

/// A line is `tooLong` above this share of its slot, `tooShort` below the other one.
pub const TOO_LONG: f64 = 1.25;
pub const TOO_SHORT: f64 = 0.5;
/// Below this transcription confidence a line is flagged.
pub const LOW_CONFIDENCE: f64 = 0.5;

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Start over. Earlier outputs are ignored.
    Fresh,
    /// Continue an interrupted, failed or cancelled run.
    Resume,
}

pub struct RunConfig {
    pub project_path: PathBuf,
    pub project_id: String,
    pub models_dir: PathBuf,
    pub catalogue: Vec<CatalogueEntry>,
    pub class: MachineClass,
    pub os: String,
    pub arch: String,
    pub ffmpeg: Ffmpeg,
    pub sidecar: SidecarConfig,
    /// Language code mapping from `config/language-codes.json`, sent to every worker.
    pub code_map: Value,
    /// `threads`, `device`, `computeType`, `ramCeilingMb` and the like, sent to every worker.
    pub params: Value,
    /// Keys, consent and adapters for cloud steps. `None` means every step is local.
    pub cloud: Option<Arc<CloudRuntime>>,
}

/// What one worker call cost, for the benchmark.
#[derive(Debug, Clone)]
pub struct StageStat {
    pub stage: Stage,
    /// Name of the model folder the worker used (the catalogue id), empty when it had none.
    pub model_id: String,
    pub wall_ms: u64,
    pub peak_ram_mb: u64,
}

#[derive(Debug, Clone)]
pub enum RunEvent {
    /// A worker call finished.
    Stat(StageStat),
    Stage {
        stage: Stage,
        status: StageStatus,
    },
    Progress {
        stage: Stage,
        stage_progress: f32,
        overall: f32,
        section_index: u32,
        section_count: u32,
        eta_seconds: Option<u64>,
    },
    Line(TranscriptLine),
    Error(RunError),
    /// Something the user should hear about a cloud step: a key switch, a key set aside, a wait.
    Cloud(CloudNote),
    /// The spoken language found by auto-detect, as soon as it is known.
    Detected(String),
    Done(RunState),
}

/// `dub:cloud` payload (`contracts/ipc-commands.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudNote {
    /// `switched`, `setAside`, `waiting` or `resumed`.
    pub kind: &'static str,
    pub step: &'static str,
    pub service_id: String,
    pub key_label: Option<String>,
    pub reason: Option<String>,
    pub wait_seconds: Option<u64>,
}

fn cloud_note(event: PoolEvent, step: &'static str, service_id: &str) -> RunEvent {
    let mut note = CloudNote {
        kind: "resumed",
        step,
        service_id: service_id.to_string(),
        key_label: None,
        reason: None,
        wait_seconds: None,
    };
    match event {
        PoolEvent::Switched { label } => {
            note.kind = "switched";
            note.key_label = Some(label);
        }
        PoolEvent::SetAside { label, reason } => {
            note.kind = "setAside";
            note.key_label = Some(label);
            note.reason = Some(reason);
        }
        PoolEvent::Waiting { seconds } => {
            note.kind = "waiting";
            note.wait_seconds = Some(seconds);
        }
        PoolEvent::Resumed => {}
    }
    RunEvent::Cloud(note)
}

/// The engines for the cloud steps of this run, after checking that the user agreed to each.
/// Nothing is sent before this passes, and a step that is not cloud costs nothing here.
fn prepare_cloud(
    cfg: &RunConfig,
    dub: &DubData,
) -> Result<(Option<CloudEngine>, Option<CloudEngine>), AppError> {
    let engines = &dub.options.fallbacks.engines;
    let estimate = estimate_chars(dub.source.as_ref().map(|s| s.duration_ms).unwrap_or(0));
    let wanted: Vec<(CloudStep, String)> = [CloudStep::Translate, CloudStep::Speak]
        .into_iter()
        .filter(|step| !(*step == CloudStep::Speak && dub.options.fallbacks.subtitles_only))
        .filter_map(|step| engines.cloud(step).map(|id| (step, id.to_string())))
        .collect();
    if wanted.is_empty() {
        return Ok((None, None));
    }
    let rt = cfg
        .cloud
        .as_ref()
        .ok_or_else(|| AppError::new(CLOUD_NO_KEY, "dub.error.cloudNoKey"))?;
    let mut missing = Vec::new();
    for (step, id) in &wanted {
        if rt.consent.needs(id, *step, estimate)?.is_some() {
            missing.push(json!({ "step": step.as_str(), "serviceId": id }));
        }
    }
    if !missing.is_empty() {
        return Err(
            AppError::new(CLOUD_CONSENT_REQUIRED, "dub.error.cloudConsentRequired")
                .with_details(json!({ "steps": missing })),
        );
    }
    let (mut translate, mut speak) = (None, None);
    for (step, id) in wanted {
        let engine = rt.engine(&id, step)?;
        match step {
            CloudStep::Translate => translate = Some(engine),
            CloudStep::Speak => speak = Some(engine),
        }
    }
    Ok((translate, speak))
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

fn gender_of(g: SpeakerGender) -> Option<Gender> {
    match g {
        SpeakerGender::Male => Some(Gender::Male),
        SpeakerGender::Female => Some(Gender::Female),
        SpeakerGender::Unknown => None,
    }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn file_hash(path: &Path) -> String {
    std::fs::read(path)
        .map(|b| input_hash(&[&String::from_utf8_lossy(&b)]))
        .unwrap_or_default()
}

fn run_error(e: &AppError, stage: Option<Stage>, section: Option<u32>) -> RunError {
    // A source that vanished is still a failed stage, but the fix is a file, not a retry.
    let source_missing = e
        .details
        .as_ref()
        .is_some_and(|d| d["reason"] == "sourceMissing");
    RunError {
        code: e.code.to_string(),
        stage,
        section_index: section,
        message_key: if source_missing {
            "dub.error.sourceMissing".into()
        } else {
            e.message_key.clone()
        },
        next_step_key: if source_missing {
            "dub.next.chooseAnotherFile".into()
        } else {
            next_step_key(e.code).to_string()
        },
        details: e.details.clone(),
    }
}

/// Flags for one line from its synthesized length, its slot and its transcription confidence.
pub fn flags_for(
    audio_ms: Option<u64>,
    start_ms: u64,
    end_ms: u64,
    confidence: f64,
) -> BTreeSet<LineFlag> {
    let mut flags = BTreeSet::new();
    let slot = end_ms.saturating_sub(start_ms).max(1) as f64;
    if let Some(a) = audio_ms {
        if a as f64 > slot * TOO_LONG {
            flags.insert(LineFlag::TooLong);
        }
        if (a as f64) < slot * TOO_SHORT {
            flags.insert(LineFlag::TooShort);
        }
    }
    if confidence < LOW_CONFIDENCE {
        flags.insert(LineFlag::LowConfidence);
    }
    flags
}

struct Runner<'a> {
    cfg: &'a RunConfig,
    work: WorkFolder,
    dub: DubData,
    run: PipelineRun,
    plan: ModelPlan,
    cancel: CancelToken,
    sidecar: Option<Sidecar>,
    fingerprint: String,
    source_lang: String,
    pacer: Pacer,
    on_event: &'a mut (dyn FnMut(RunEvent) + Send),
    eta: Eta,
    translate_engine: Option<CloudEngine>,
    speak_engine: Option<CloudEngine>,
    /// Usage from before this process (a resumed run adds to it).
    usage_base: Vec<CloudUsage>,
}

/// Run (or resume) the dub. Returns the project data with `run` and `lines` filled in. A
/// cancelled run returns `Ok` with state `Cancelled`; failures are in `run.error` and also
/// returned as `Err`, after the state is saved.
pub async fn run_dub(
    cfg: &RunConfig,
    mut dub: DubData,
    mode: Mode,
    cancel: CancelToken,
    on_event: &mut (dyn FnMut(RunEvent) + Send),
) -> Result<DubData, AppError> {
    let source = dub
        .source
        .clone()
        .ok_or_else(|| AppError::new(STAGE_FAILED, "dub.error.stageFailed"))?;
    let work = WorkFolder::create(&cfg.project_path, &cfg.project_id)?;

    let fp = fingerprint(Path::new(&source.path))
        .map_err(|_| AppError::new(SOURCE_CHANGED, "dub.error.sourceChanged"))?;
    if fp != source.fingerprint {
        return Err(AppError::new(SOURCE_CHANGED, "dub.error.sourceChanged"));
    }

    let previous = work.load_run()?;
    let run = match (mode, previous) {
        (Mode::Resume, Some(prev))
            if prev.state != RunState::Completed && !prev.sections.is_empty() =>
        {
            PipelineRun {
                state: RunState::Running,
                error: None,
                ended_at: None,
                ..prev
            }
        }
        (Mode::Resume, _) => {
            return Err(AppError::new(
                NOTHING_TO_RESUME,
                "dub.error.nothingToResume",
            ))
        }
        (Mode::Fresh, _) => PipelineRun {
            id: uuid::Uuid::new_v4().to_string(),
            started_at: now(),
            state: RunState::Running,
            stages: Stage::ALL.iter().map(|s| StageState::new(*s)).collect(),
            ..Default::default()
        },
    };
    if mode == Mode::Fresh {
        let _ = std::fs::remove_file(work.path().join("run-state.json"));
    }

    let source_lang = if dub.source_language == "auto" {
        dub.detected_language
            .clone()
            .unwrap_or_else(|| "auto".into())
    } else {
        dub.source_language.clone()
    };
    let have: Vec<String> = crate::models::installed(&cfg.models_dir)
        .into_iter()
        .map(|m| m.id)
        .collect();
    let plan = plan_models_full(
        &cfg.catalogue,
        cfg.class,
        &cfg.os,
        &cfg.arch,
        &source_lang,
        &dub.target_language,
        dub.options.quality == Quality::Fast,
        &have,
        &dub.options.fallbacks,
    );
    dub.lines.clear();
    let (translate_engine, speak_engine) = prepare_cloud(cfg, &dub)?;
    let usage_base = if mode == Mode::Resume {
        read_json(&work.path().join("cloud-usage.json"))
            .filter(|v| v["runId"] == run.id.as_str())
            .and_then(|v| serde_json::from_value(v["usage"].clone()).ok())
            .unwrap_or_default()
    } else {
        let _ = std::fs::remove_file(work.path().join("cloud-usage.json"));
        Vec::new()
    };

    let mut runner = Runner {
        cfg,
        work,
        dub,
        run,
        plan,
        cancel,
        sidecar: None,
        fingerprint: fp,
        source_lang,
        pacer: Pacer::default(),
        on_event,
        eta: Eta::default(),
        translate_engine,
        speak_engine,
        usage_base,
    };
    runner.save()?;
    let outcome = runner.drive().await;
    runner.finish(outcome).await
}

impl Runner<'_> {
    fn save(&mut self) -> Result<(), AppError> {
        self.work.save_run(&self.run)
    }

    /// What the cloud steps used so far, including before a restart.
    fn merged_usage(&self) -> Vec<CloudUsage> {
        let mut out = self.usage_base.clone();
        for e in [&self.translate_engine, &self.speak_engine]
            .into_iter()
            .flatten()
        {
            let u = e.usage();
            match out
                .iter_mut()
                .find(|b| b.step == u.step && b.service_id == u.service_id)
            {
                Some(b) => {
                    b.chars += u.chars;
                    b.requests += u.requests;
                    b.switches += u.switches;
                    b.waited_seconds += u.waited_seconds;
                    for k in u.keys_used {
                        if !b.keys_used.contains(&k) {
                            b.keys_used.push(k);
                        }
                    }
                }
                None => out.push(CloudUsage {
                    run_id: self.run.id.clone(),
                    ..u
                }),
            }
        }
        out
    }

    /// Keep the usage next to the run, so a stop or a crash does not lose the count.
    fn save_usage(&self) {
        if self.translate_engine.is_none() && self.speak_engine.is_none() {
            return;
        }
        let body = json!({ "runId": self.run.id, "usage": self.merged_usage() });
        if let Ok(bytes) = serde_json::to_vec(&body) {
            let _ = crate::fsutil::write_atomic(&self.work.path().join("cloud-usage.json"), &bytes);
        }
    }

    fn cloud_gender(&self, speaker_id: &str) -> CloudGender {
        match self
            .dub
            .speakers
            .iter()
            .find(|s| s.id == speaker_id)
            .map(|s| s.gender)
        {
            Some(SpeakerGender::Male) => CloudGender::Male,
            _ => CloudGender::Female,
        }
    }

    fn emit(&mut self, event: RunEvent) {
        (self.on_event)(event);
    }

    fn set_stage(&mut self, stage: Stage, status: StageStatus) {
        if let Some(s) = self.run.stages.iter_mut().find(|s| s.stage == stage) {
            if s.status != status {
                s.status = status;
                match status {
                    StageStatus::Running => s.started_at = Some(now()),
                    StageStatus::Done | StageStatus::Failed => s.ended_at = Some(now()),
                    StageStatus::Pending => {}
                }
                self.emit(RunEvent::Stage { stage, status });
            }
        }
    }

    fn check_cancel(&self) -> Result<(), AppError> {
        if self.cancel.is_cancelled() {
            Err(AppError::new(CANCELLED, "dub.error.cancelled"))
        } else {
            Ok(())
        }
    }

    fn model_dir(&self, id: &Option<String>) -> Result<PathBuf, AppError> {
        let id = id
            .as_ref()
            .ok_or_else(|| AppError::new(MODELS_MISSING, "dub.error.modelsMissing"))?;
        let dir = self.cfg.models_dir.join(id);
        if dir.exists() {
            Ok(dir)
        } else {
            Err(AppError::new(MODELS_MISSING, "dub.error.modelsMissing")
                .with_details(json!({ "missing": [id] })))
        }
    }

    fn overall(&self, section: usize, stage_index: usize, fraction: f32) -> f32 {
        let sections = self.run.sections.len().max(1) as f32;
        let per = Stage::PER_SECTION.len() as f32;
        // Extract is 5 %, mix and export together 5 %, the section stages share the rest.
        0.05 + 0.90 * ((section as f32 * per + stage_index as f32 + fraction) / (sections * per))
    }

    /// Send a progress event unless one left less than a second ago.
    fn emit_progress(&mut self, event: RunEvent) {
        if self.pacer.progress_due(Instant::now()) {
            self.emit(event);
        }
    }

    /// The source must still be there for every stage: a file taken away mid-run fails the stage
    /// about to start, by name.
    fn check_source(&self, stage: Stage) -> Result<(), AppError> {
        let Some(source) = self.dub.source.as_ref() else {
            return Ok(());
        };
        if std::fs::metadata(&source.path).is_ok() {
            return Ok(());
        }
        Err(
            AppError::new(STAGE_FAILED, "dub.error.stageFailed").with_details(
                json!({ "stage": stage, "reason": "sourceMissing", "path": source.path }),
            ),
        )
    }

    /// A full disk reports what the rest of the run needs, what is free, and the difference.
    fn with_space_numbers(&self, mut e: AppError, from_section: usize) -> AppError {
        let left_ms: u64 = self
            .run
            .sections
            .iter()
            .skip(from_section)
            .map(|s| s.end_ms.saturating_sub(s.start_ms))
            .sum();
        let needed = disk_needed_bytes(left_ms);
        let free = crate::models::free_space(self.work.path()).unwrap_or(0);
        let mut details = e.details.take().unwrap_or_else(|| json!({}));
        if let Value::Object(map) = &mut details {
            map.insert("neededBytes".into(), json!(needed));
            map.insert("freeBytes".into(), json!(free));
            map.insert("shortfallBytes".into(), json!(needed.saturating_sub(free)));
        }
        e.with_details(details)
    }

    async fn sidecar(&mut self) -> Result<&mut Sidecar, AppError> {
        if self.sidecar.is_none() {
            let mut cfg = self.cfg.sidecar.clone();
            if cfg.program.as_os_str().is_empty() {
                cfg.program = super::super::models::runtime::python_exe(&self.cfg.models_dir)?;
            }
            self.sidecar = Some(Sidecar::start(cfg).await?);
        }
        Ok(self.sidecar.as_mut().expect("just started"))
    }

    fn params(&self) -> Value {
        let mut p = self.cfg.params.clone();
        if let Value::Object(map) = &mut p {
            map.insert("codeMap".into(), self.cfg.code_map.clone());
        }
        p
    }

    /// Send one request to the sidecar with crash retry, mapping its progress to run progress as
    /// it arrives. A worker that goes quiet still gets a heartbeat event.
    async fn call(
        &mut self,
        req: Request,
        section: usize,
        stage: Stage,
    ) -> Result<crate::sidecar::Done, AppError> {
        self.check_cancel()?;
        // Workers with one model name it `main`; speech lists voice models by their ids.
        let model_id = req
            .models
            .get("main")
            .and_then(Value::as_str)
            .and_then(|m| Path::new(m).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .or_else(|| {
                req.models.as_object().and_then(|m| {
                    m.keys()
                        .find(|k| !matches!(k.as_str(), "espeak" | "speaker"))
                        .cloned()
                })
            })
            .unwrap_or_default();
        let stage_index = Stage::PER_SECTION
            .iter()
            .position(|s| *s == stage)
            .unwrap_or(0);
        let count = self.run.sections.len();
        let eta = self.eta.seconds(count);
        let (from, to) = (
            self.overall(section, stage_index, 0.0),
            self.overall(section, stage_index, 1.0),
        );
        self.sidecar().await?;
        let mut sidecar = self.sidecar.take().expect("just started");
        let watcher = {
            let cancel = self.cancel.clone();
            let handle = sidecar.cancel_handle();
            tokio::spawn(async move {
                while !cancel.is_cancelled() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                handle.cancel().await;
            })
        };
        let on_event = &mut *self.on_event;
        let pacer = &mut self.pacer;
        let mut fraction = 0.0f32;
        let result = sidecar
            .run_with_retry(req, &mut |ev| {
                let now = Instant::now();
                let due = match ev {
                    Event::Progress(f) => {
                        fraction = f.clamp(0.0, 1.0);
                        pacer.progress_due(now)
                    }
                    Event::Tick => pacer.heartbeat_due(now),
                    _ => false,
                };
                if due {
                    on_event(RunEvent::Progress {
                        stage,
                        stage_progress: fraction,
                        overall: from + (to - from) * fraction,
                        section_index: section as u32,
                        section_count: count as u32,
                        eta_seconds: eta,
                    });
                }
            })
            .await;
        watcher.abort();
        self.sidecar = Some(sidecar);
        if let Ok(done) = &result {
            self.emit(RunEvent::Stat(StageStat {
                stage,
                model_id,
                wall_ms: done.stats.wall_ms,
                peak_ram_mb: done.stats.peak_ram_mb,
            }));
        }
        result
    }

    async fn drive(&mut self) -> Result<(), AppError> {
        let source = self.dub.source.clone().expect("checked in run_dub");
        let work = self.work.path().to_path_buf();

        // 1. extract
        self.set_stage(Stage::Extract, StageStatus::Running);
        let have = work.join("audio.wav").exists()
            && work.join("audio-full.wav").exists()
            && read_json(&work.join("source-info.json"))
                .is_some_and(|v| v["fingerprint"] == self.fingerprint.as_str());
        if !have {
            let on_event = &mut *self.on_event;
            let pacer = &mut self.pacer;
            extract(
                &self.cfg.ffmpeg,
                Path::new(&source.path),
                &work,
                source.duration_ms,
                &self.fingerprint,
                &mut |p| {
                    if pacer.progress_due(Instant::now()) {
                        on_event(RunEvent::Progress {
                            stage: Stage::Extract,
                            stage_progress: p,
                            overall: 0.05 * p,
                            section_index: 0,
                            section_count: 0,
                            eta_seconds: None,
                        });
                    }
                },
            )
            .await?;
        }
        self.set_stage(Stage::Extract, StageStatus::Done);
        self.check_cancel()?;

        // 2. sections
        if self.run.sections.is_empty() {
            let silences = self
                .cfg
                .ffmpeg
                .silences(&work.join("audio.wav"), -35, 0.4)
                .await?;
            self.run.sections = plan_sections(source.duration_ms, &silences);
        }
        self.save()?;

        // 3. language detection when the source language is `auto`
        if self.source_lang == "auto" {
            self.set_stage(Stage::Transcribe, StageStatus::Running);
            if let Err(e) = self.detect_language().await {
                self.set_stage(Stage::Transcribe, StageStatus::Failed);
                return Err(e);
            }
        }

        // 4. per-section stages
        let fast = self.dub.options.quality == Quality::Fast;
        let subtitles_only = self.dub.options.fallbacks.subtitles_only;
        let count = self.run.sections.len();
        for stage in Stage::PER_SECTION {
            if !(fast && stage == Stage::Separate) {
                self.set_stage(stage, StageStatus::Pending);
            }
        }
        for index in 0..count {
            let started = Instant::now();
            let already = self.section_complete(index, fast, subtitles_only);
            let mut lines = Vec::new();
            for (stage_index, stage) in Stage::PER_SECTION.into_iter().enumerate() {
                if (fast && stage == Stage::Separate)
                    || (subtitles_only && stage == Stage::Synthesize)
                {
                    continue;
                }
                self.check_cancel()?;
                self.set_stage(stage, StageStatus::Running);
                let outcome = match self.check_source(stage) {
                    Err(e) => Err(e),
                    Ok(()) => match stage {
                        Stage::Separate => self.separate(index).await.map(|_| ()),
                        Stage::Transcribe => self.transcribe(index).await.map(|l| lines = l),
                        Stage::Translate => self.translate(index).await.map(|l| lines = l),
                        Stage::Synthesize => self.synthesize(index).await.map(|l| lines = l),
                        _ => Ok(()),
                    },
                };
                if let Err(e) = outcome {
                    self.mark(index, stage, StageStatus::Failed);
                    self.set_stage(stage, StageStatus::Failed);
                    return Err(if e.code == NO_SPACE {
                        self.with_space_numbers(e, index)
                    } else {
                        e
                    });
                }
                self.mark(index, stage, StageStatus::Done);
                self.save()?;
                let overall = self.overall(index, stage_index + 1, 0.0);
                let eta = self.eta.seconds(count);
                self.emit_progress(RunEvent::Progress {
                    stage,
                    stage_progress: 1.0,
                    overall,
                    section_index: index as u32,
                    section_count: count as u32,
                    eta_seconds: eta,
                });
            }
            for line in &lines {
                self.emit(RunEvent::Line(line.clone()));
            }
            self.dub.lines.extend(lines);
            if already {
                self.eta.already_done(1);
            } else {
                self.eta.section_done(started.elapsed());
            }
        }
        for stage in Stage::PER_SECTION {
            self.set_stage(stage, StageStatus::Done);
        }

        // 5. mix
        self.check_cancel()?;
        self.set_stage(Stage::Mix, StageStatus::Running);
        // A run that does not speak has nothing to mix: its result is the translated lines.
        if !subtitles_only {
            self.mix_all().await?;
        }
        self.set_stage(Stage::Mix, StageStatus::Done);
        let count = self.run.sections.len() as u32;
        self.emit_progress(RunEvent::Progress {
            stage: Stage::Mix,
            stage_progress: 1.0,
            overall: 1.0,
            section_index: count.saturating_sub(1),
            section_count: count,
            eta_seconds: Some(0),
        });
        Ok(())
    }

    /// Every stage this run would do for the section is already done (a resume skipping it).
    fn section_complete(&self, index: usize, fast: bool, subtitles_only: bool) -> bool {
        let sec = &self.run.sections[index];
        Stage::PER_SECTION
            .into_iter()
            .filter(|s| {
                !(fast && *s == Stage::Separate || subtitles_only && *s == Stage::Synthesize)
            })
            .all(|s| sec.status.get(&s) == Some(&StageStatus::Done))
    }

    fn mark(&mut self, index: usize, stage: Stage, status: StageStatus) {
        if let Some(sec) = self.run.sections.get_mut(index) {
            sec.status.insert(stage, status);
        }
    }

    fn record_hash(&mut self, index: usize, stage: Stage, hash: &str) {
        if let Some(sec) = self.run.sections.get_mut(index) {
            sec.input_hash.insert(stage, hash.to_string());
        }
    }

    fn section(&self, index: usize) -> Section {
        self.run.sections[index].clone()
    }

    fn skip(&self, index: usize, stage: Stage, hash: &str, outputs: &[PathBuf]) -> bool {
        let refs: Vec<&Path> = outputs.iter().map(PathBuf::as_path).collect();
        self.work.can_skip(
            &self.run,
            self.run.sections[index].index,
            stage,
            hash,
            &refs,
        )
    }

    fn request(
        &self,
        worker: &str,
        input: Value,
        dir: &Path,
        prefix: &str,
        models: Value,
    ) -> Request {
        Request {
            worker: worker.into(),
            input,
            output: json!({ "dir": dir, "prefix": prefix }),
            models,
            params: self.params(),
        }
    }

    async fn detect_language(&mut self) -> Result<(), AppError> {
        let model = self.plan.transcribe.clone();
        let dir = self.model_dir(&model)?;
        let work = self.work.path().to_path_buf();
        let end = self
            .dub
            .source
            .as_ref()
            .map(|s| s.duration_ms.min(120_000))
            .unwrap_or(0);
        let req = self.request(
            "detect_language",
            json!({ "audio": work.join("audio.wav"), "startMs": 0, "endMs": end }),
            &work,
            "detect",
            json!({ "main": dir }),
        );
        let done = self.call(req, 0, Stage::Transcribe).await?;
        let language = done.result["language"]
            .as_str()
            .unwrap_or("auto")
            .to_string();
        self.dub.detected_language = Some(language.clone());
        self.source_lang = language.clone();
        self.emit(RunEvent::Detected(language.clone()));
        let have: Vec<String> = crate::models::installed(&self.cfg.models_dir)
            .into_iter()
            .map(|m| m.id)
            .collect();
        self.plan = plan_models_full(
            &self.cfg.catalogue,
            self.cfg.class,
            &self.cfg.os,
            &self.cfg.arch,
            &language,
            &self.dub.target_language,
            self.dub.options.quality == Quality::Fast,
            &have,
            &self.dub.options.fallbacks,
        );
        // Detection can show that nothing needs translating. That is worth a question, not a
        // silent run: the user confirms and resumes.
        let same =
            language.split(['-', '_']).next() == self.dub.target_language.split(['-', '_']).next();
        if same && !self.dub.options.confirm_same {
            return Err(AppError::new(SAME_LANGUAGE, "dub.error.sameLanguage")
                .with_details(json!({ "detected": language })));
        }
        Ok(())
    }

    async fn separate(&mut self, index: usize) -> Result<(), AppError> {
        let sec = self.section(index);
        let dir = self.work.section_dir(sec.index);
        std::fs::create_dir_all(&dir).map_err(AppError::io)?;
        let model = self.plan.separate.clone().unwrap_or_default();
        let hash = input_hash(&[
            "separate",
            &model,
            &self.fingerprint,
            &sec.start_ms.to_string(),
            &sec.end_ms.to_string(),
        ]);
        let outputs = [dir.join("sec-vocals.wav"), dir.join("sec-music.wav")];
        if self.skip(index, Stage::Separate, &hash, &outputs) {
            return Ok(());
        }
        let model_dir = self.model_dir(&self.plan.separate.clone())?;
        let req = self.request(
            "separate",
            json!({ "audio": self.work.path().join("audio-full.wav"), "startMs": sec.start_ms, "endMs": sec.end_ms }),
            &dir,
            "sec",
            json!({ "main": model_dir }),
        );
        self.call(req, index, Stage::Separate).await?;
        self.record_hash(index, Stage::Separate, &hash);
        Ok(())
    }

    fn lines_path(&self, sec: &Section) -> PathBuf {
        self.work.section_dir(sec.index).join("lines.json")
    }

    fn save_lines(&self, sec: &Section, lines: &[TranscriptLine]) -> Result<(), AppError> {
        crate::fsutil::write_atomic(
            &self.lines_path(sec),
            &serde_json::to_vec_pretty(lines).map_err(|e| AppError::io(e.into()))?,
        )
    }

    fn load_lines(&self, sec: &Section) -> Option<Vec<TranscriptLine>> {
        serde_json::from_slice(&std::fs::read(self.lines_path(sec)).ok()?).ok()
    }

    /// Add the speakers of a transcript file to the project, keeping the first seen per id.
    fn ingest_speakers(&mut self, data: &Value) {
        for s in data["speakers"].as_array().into_iter().flatten() {
            let id = s["id"].as_str().unwrap_or("spk1").to_string();
            if !self.dub.speakers.iter().any(|x| x.id == id) {
                self.dub.speakers.push(Speaker {
                    id,
                    gender: match s["gender"].as_str() {
                        Some("male") => SpeakerGender::Male,
                        Some("female") => SpeakerGender::Female,
                        _ => SpeakerGender::Unknown,
                    },
                    voice_id: String::new(),
                    confidence: s["confidence"].as_f64().unwrap_or(1.0) as f32,
                });
            }
        }
    }

    async fn transcribe(&mut self, index: usize) -> Result<Vec<TranscriptLine>, AppError> {
        let sec = self.section(index);
        let dir = self.work.section_dir(sec.index);
        std::fs::create_dir_all(&dir).map_err(AppError::io)?;
        let fast = self.dub.options.quality == Quality::Fast;
        let model = self.plan.transcribe.clone().unwrap_or_default();
        let speaker = self.plan.speaker.clone().unwrap_or_default();
        let hash = input_hash(&[
            "transcribe",
            &model,
            &speaker,
            &self.source_lang,
            if fast { "fast" } else { "standard" },
            &self.fingerprint,
            &sec.start_ms.to_string(),
            &sec.end_ms.to_string(),
        ]);
        let json_path = dir.join("sec.json");
        if self.skip(
            index,
            Stage::Transcribe,
            &hash,
            &[json_path.clone(), self.lines_path(&sec)],
        ) {
            if let (Some(lines), Some(data)) = (self.load_lines(&sec), read_json(&json_path)) {
                self.ingest_speakers(&data);
                return Ok(lines);
            }
        }
        let main = self.model_dir(&self.plan.transcribe.clone())?;
        let mut models = json!({ "main": main });
        if let Some(id) = &self.plan.speaker {
            let d = self.cfg.models_dir.join(id);
            if d.exists() {
                models["speaker"] = json!(d);
            }
        }
        let (audio, start, end) = if fast {
            (self.work.path().join("audio.wav"), sec.start_ms, sec.end_ms)
        } else {
            (dir.join("sec-vocals.wav"), 0, sec.end_ms - sec.start_ms)
        };
        let language = if self.source_lang == "auto" {
            "auto"
        } else {
            &self.source_lang
        };
        let req = self.request(
            "transcribe",
            json!({ "audio": audio, "startMs": start, "endMs": end, "language": language }),
            &dir,
            "sec",
            models,
        );
        self.call(req, index, Stage::Transcribe).await?;

        let data = read_json(&json_path)
            .ok_or_else(|| AppError::new(STAGE_FAILED, "dub.error.stageFailed"))?;
        let detected = data["language"].as_str().unwrap_or("").to_string();
        self.ingest_speakers(&data);
        let mut lines = Vec::new();
        let mut previous_end = 0u64;
        for (n, l) in data["lines"].as_array().into_iter().flatten().enumerate() {
            let (rel_start, rel_end) = (
                l["startMs"].as_u64().unwrap_or(0),
                l["endMs"].as_u64().unwrap_or(1),
            );
            // Fast mode transcribes the whole file's window, so times are already window-relative
            // too; both shift by the section start.
            let start_ms = sec.start_ms + rel_start;
            let end_ms = (sec.start_ms + rel_end).min(sec.end_ms).max(start_ms + 1);
            let confidence = l["confidence"].as_f64().unwrap_or(1.0);
            let mut flags = flags_for(None, start_ms, end_ms, confidence);
            if n > 0 && start_ms < previous_end {
                flags.insert(LineFlag::Overlap);
            }
            let language = l["language"].as_str().unwrap_or("").to_string();
            if !detected.is_empty() && !language.is_empty() && language != detected {
                flags.insert(LineFlag::MixedLanguage);
            }
            previous_end = end_ms;
            lines.push(TranscriptLine {
                id: format!("s{:04}-l{:04}", sec.index, n),
                section_index: sec.index,
                start_ms,
                end_ms,
                speaker_id: l["speakerHint"].as_str().unwrap_or("spk1").to_string(),
                language,
                text: l["text"].as_str().unwrap_or("").to_string(),
                translated: None,
                audio: None,
                audio_ms: None,
                flags,
                status: LineStatus::Transcribed,
            });
        }
        self.save_lines(&sec, &lines)?;
        self.record_hash(index, Stage::Transcribe, &hash);
        Ok(lines)
    }

    /// Translate `texts` with one model. The count must not change.
    async fn translate_texts(
        &mut self,
        texts: &[String],
        source: &str,
        target: &str,
        model: &Option<String>,
        dir: &Path,
        index: usize,
    ) -> Result<Vec<String>, AppError> {
        let main = self.model_dir(model)?;
        let req = self.request(
            "translate",
            json!({
                "source": source,
                "target": target,
                "lines": texts.iter().map(|t| json!({ "text": t })).collect::<Vec<_>>(),
            }),
            dir,
            "sec",
            json!({ "main": main }),
        );
        let done = self.call(req, index, Stage::Translate).await?;
        let out: Vec<String> = done.result["translated"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default();
        if out.len() != texts.len() {
            return Err(AppError::new(STAGE_FAILED, "dub.error.stageFailed")
                .with_details(json!({ "stage": "translate", "reason": "line count changed" })));
        }
        Ok(out)
    }

    async fn translate(&mut self, index: usize) -> Result<Vec<TranscriptLine>, AppError> {
        let sec = self.section(index);
        let dir = self.work.section_dir(sec.index);
        let mut lines = self.load_lines(&sec).unwrap_or_default();
        if lines.is_empty() {
            self.record_hash(index, Stage::Translate, "empty");
            return Ok(lines);
        }
        let model = self
            .translate_engine
            .as_ref()
            .map(|e| format!("cloud:{}", e.service_id()))
            .unwrap_or_else(|| self.plan.translate.clone().unwrap_or_default());
        let same = self.source_lang.split('-').next() == self.dub.target_language.split('-').next();
        let hash = input_hash(&[
            "translate",
            &model,
            self.plan.pivot.as_deref().unwrap_or(""),
            &self.source_lang,
            &self.dub.target_language,
            &file_hash(&dir.join("sec.json")),
        ]);
        let done_path = dir.join("translated.json");
        if self.skip(
            index,
            Stage::Translate,
            &hash,
            std::slice::from_ref(&done_path),
        ) {
            if let Some(Value::Array(items)) = read_json(&done_path) {
                if items.len() == lines.len() {
                    for (l, t) in lines.iter_mut().zip(&items) {
                        l.translated = t.as_str().map(String::from);
                        l.status = LineStatus::Translated;
                    }
                    return Ok(lines);
                }
            }
        }
        let translated: Vec<String> = if same {
            lines.iter().map(|l| l.text.clone()).collect()
        } else if let Some(mut engine) = self.translate_engine.take() {
            let texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
            let (source, target) = (self.source_lang.clone(), self.dub.target_language.clone());
            let cancel = self.cancel.clone();
            let stop = move || cancel.is_cancelled();
            let service = engine.service_id().to_string();
            let result = {
                let on_event = &mut *self.on_event;
                engine
                    .translate_batches(
                        &source,
                        &target,
                        &texts,
                        &dir.join("translated.partial.json"),
                        &mut |pe| on_event(cloud_note(pe, "translate", &service)),
                        &stop,
                    )
                    .await
            };
            self.translate_engine = Some(engine);
            self.save_usage();
            result?
        } else if let Some(second) = self.plan.pivot.clone() {
            // Through English: source to English, then English to the target.
            let texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
            let first = self
                .translate_texts(
                    &texts,
                    &self.source_lang.clone(),
                    "en",
                    &self.plan.translate.clone(),
                    &dir,
                    index,
                )
                .await?;
            self.translate_texts(
                &first,
                "en",
                &self.dub.target_language.clone(),
                &Some(second),
                &dir,
                index,
            )
            .await?
        } else {
            let texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
            self.translate_texts(
                &texts,
                &self.source_lang.clone(),
                &self.dub.target_language.clone(),
                &self.plan.translate.clone(),
                &dir,
                index,
            )
            .await?
        };
        crate::fsutil::write_atomic(
            &done_path,
            &serde_json::to_vec(&translated).map_err(|e| AppError::io(e.into()))?,
        )?;
        for (l, t) in lines.iter_mut().zip(&translated) {
            l.translated = Some(t.clone());
            l.status = LineStatus::Translated;
        }
        self.save_lines(&sec, &lines)?;
        self.record_hash(index, Stage::Translate, &hash);
        Ok(lines)
    }

    fn voice_for(&self, speaker_id: &str) -> Option<VoicePick> {
        let gender = self
            .dub
            .speakers
            .iter()
            .find(|s| s.id == speaker_id)
            .and_then(|s| gender_of(s.gender))
            .unwrap_or(Gender::Female);
        self.plan
            .voices
            .get(&gender)
            .or_else(|| self.plan.voices.values().next())
            .cloned()
    }

    async fn synthesize(&mut self, index: usize) -> Result<Vec<TranscriptLine>, AppError> {
        let sec = self.section(index);
        let dir = self.work.section_dir(sec.index);
        let mut lines = self.load_lines(&sec).unwrap_or_default();
        if lines.is_empty() {
            self.record_hash(index, Stage::Synthesize, "empty");
            return Ok(lines);
        }
        let picks: Vec<&VoicePick> = self.plan.voices.values().collect();
        // The models to load, and a description of each pick for the input hash.
        let mut voices: Vec<String> = picks.iter().map(|v| v.model.clone()).collect();
        voices.sort();
        voices.dedup();
        let mut parts: Vec<String> = vec![
            "synthesize".into(),
            self.dub.target_language.clone(),
            file_hash(&dir.join("translated.json")),
        ];
        parts.extend(
            picks
                .iter()
                .map(|v| format!("{}#{}#{}", v.model, v.speaker_id, v.lang)),
        );
        if let Some(e) = &self.speak_engine {
            parts.push(format!("cloud:{}", e.service_id()));
        }
        // Speakers and their genders are in this section's transcript file.
        parts.push(file_hash(&dir.join("sec.json")));
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        let hash = input_hash(&refs);
        let clips = dir.join("clips");
        let done_marker = dir.join("synth.json");
        let outputs: Vec<PathBuf> = std::iter::once(done_marker.clone())
            .chain(lines.iter().map(|l| clips.join(format!("{}.wav", l.id))))
            .collect();
        if self.skip(index, Stage::Synthesize, &hash, &outputs) {
            if let Some(Value::Array(items)) = read_json(&done_marker) {
                self.apply_clips(&mut lines, &items, &clips);
                return Ok(lines);
            }
        }
        std::fs::create_dir_all(&clips).map_err(AppError::io)?;

        if let Some(mut engine) = self.speak_engine.take() {
            let language = self
                .dub
                .target_language
                .split(['-', '_'])
                .next()
                .unwrap_or_default()
                .to_string();
            let jobs: Vec<SpeakJob> = lines
                .iter()
                .map(|l| SpeakJob {
                    id: l.id.clone(),
                    text: l.translated.clone().unwrap_or_else(|| l.text.clone()),
                    language: language.clone(),
                    gender: self.cloud_gender(&l.speaker_id),
                    target_ms: l.end_ms.saturating_sub(l.start_ms),
                })
                .collect();
            let cancel = self.cancel.clone();
            let stop = move || cancel.is_cancelled();
            let service = engine.service_id().to_string();
            let ffmpeg = &self.cfg.ffmpeg;
            let result = {
                let on_event = &mut *self.on_event;
                engine
                    .speak_lines(
                        &jobs,
                        &clips,
                        ffmpeg,
                        &mut |pe| on_event(cloud_note(pe, "speak", &service)),
                        &stop,
                    )
                    .await
            };
            self.speak_engine = Some(engine);
            self.save_usage();
            let done = result?;
            let items: Vec<Value> = done
                .iter()
                .map(|c| json!({ "id": c.id, "audioMs": c.audio_ms }))
                .collect();
            crate::fsutil::write_atomic(
                &done_marker,
                &serde_json::to_vec(&items).map_err(|e| AppError::io(e.into()))?,
            )?;
            self.apply_clips(&mut lines, &items, &clips);
            self.save_lines(&sec, &lines)?;
            self.record_hash(index, Stage::Synthesize, &hash);
            return Ok(lines);
        }

        let mut models = serde_json::Map::new();
        for id in &voices {
            models.insert(id.clone(), json!(self.model_dir(&Some(id.clone()))?));
        }
        if let Some(espeak) = self
            .cfg
            .models_dir
            .join("espeak-ng-data")
            .exists()
            .then(|| self.cfg.models_dir.join("espeak-ng-data"))
        {
            models.insert("espeak".into(), json!(espeak));
        }
        let mut request_lines = Vec::new();
        for l in &lines {
            let voice = self
                .voice_for(&l.speaker_id)
                .ok_or_else(|| AppError::new(MODELS_MISSING, "dub.error.modelsMissing"))?;
            request_lines.push(json!({
                "id": l.id,
                "text": l.translated.clone().unwrap_or_else(|| l.text.clone()),
                "voiceId": voice.model,
                "speakerId": voice.speaker_id,
                "lang": voice.lang,
                "lexicon": voice.lexicon,
                "targetMs": l.end_ms - l.start_ms,
            }));
        }
        let req = self.request(
            "synthesize",
            json!({ "lines": request_lines }),
            &clips,
            "sec",
            Value::Object(models),
        );
        let done = self.call(req, index, Stage::Synthesize).await?;
        let items = done.result["clips"].as_array().cloned().unwrap_or_default();
        crate::fsutil::write_atomic(
            &done_marker,
            &serde_json::to_vec(&items).map_err(|e| AppError::io(e.into()))?,
        )?;
        self.apply_clips(&mut lines, &items, &clips);
        self.save_lines(&sec, &lines)?;
        self.record_hash(index, Stage::Synthesize, &hash);
        Ok(lines)
    }

    fn apply_clips(&self, lines: &mut [TranscriptLine], items: &[Value], clips: &Path) {
        for l in lines.iter_mut() {
            let Some(item) = items.iter().find(|i| i["id"] == l.id.as_str()) else {
                continue;
            };
            let ms = item["audioMs"].as_u64().unwrap_or(0);
            l.audio = clips
                .join(format!("{}.wav", l.id))
                .strip_prefix(self.work.path())
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            l.audio_ms = Some(ms);
            l.status = LineStatus::Synthesized;
            let confidence = if l.flags.contains(&LineFlag::LowConfidence) {
                0.0
            } else {
                1.0
            };
            let fresh = flags_for(Some(ms), l.start_ms, l.end_ms, confidence);
            l.flags
                .retain(|f| !matches!(f, LineFlag::TooLong | LineFlag::TooShort));
            l.flags.extend(
                fresh
                    .into_iter()
                    .filter(|f| matches!(f, LineFlag::TooLong | LineFlag::TooShort)),
            );
        }
    }

    async fn mix_all(&mut self) -> Result<(), AppError> {
        let work = self.work.path().to_path_buf();
        let original = work.join("audio-full.wav");
        if self.dub.lines.is_empty() {
            // Nothing was said: the dub is the original audio (NO_SPEECH_FOUND is not fatal).
            let part = work.join("dub-mix.wav.part");
            std::fs::copy(&original, &part).map_err(AppError::io)?;
            std::fs::rename(&part, work.join("dub-mix.wav")).map_err(AppError::io)?;
            let e = AppError::new(NO_SPEECH_FOUND, "dub.error.noSpeechFound");
            let err = run_error(&e, None, None);
            self.run.error = Some(err.clone());
            self.emit(RunEvent::Error(err));
            return Ok(());
        }
        let fast = self.dub.options.quality == Quality::Fast;
        let mut sections = Vec::new();
        for sec in &self.run.sections {
            let dir = self.work.section_dir(sec.index);
            let music = dir.join("sec-music.wav");
            sections.push(MixSection {
                start_ms: sec.start_ms,
                end_ms: sec.end_ms,
                music: (!fast && music.exists()).then_some(music),
                lines: self
                    .dub
                    .lines
                    .iter()
                    .filter(|l| l.section_index == sec.index)
                    .filter_map(|l| {
                        l.audio.as_ref().map(|a| MixLine {
                            start_ms: l.start_ms,
                            clip: work.join(a),
                        })
                    })
                    .collect(),
            });
        }
        let mute = self.dub.options.mute_original;
        mix(
            &self.cfg.ffmpeg,
            &work,
            &original,
            &sections,
            mute,
            &mut |_| {},
        )
        .await?;
        Ok(())
    }

    async fn finish(mut self, outcome: Result<(), AppError>) -> Result<DubData, AppError> {
        if let Some(mut sc) = self.sidecar.take() {
            sc.shutdown().await;
        }
        self.run.ended_at = Some(now());
        let result = match outcome {
            Ok(()) => {
                self.run.state = RunState::Completed;
                self.set_stage(Stage::Mix, StageStatus::Done);
                Ok(())
            }
            Err(e) if e.code == CANCELLED => {
                self.run.state = RunState::Cancelled;
                Ok(())
            }
            Err(e) => {
                self.run.state = RunState::Failed;
                let stage = Stage::ALL.iter().copied().find(|s| {
                    self.run
                        .stages
                        .iter()
                        .any(|x| x.stage == *s && x.status == StageStatus::Failed)
                });
                let section = self
                    .run
                    .sections
                    .iter()
                    .find(|s| s.status.values().any(|v| *v == StageStatus::Failed))
                    .map(|s| s.index);
                let err = run_error(&e, stage, section);
                self.run.error = Some(err.clone());
                self.emit(RunEvent::Error(err));
                Err(e)
            }
        };
        let _ = self.save();
        self.emit(RunEvent::Done(self.run.state));
        self.dub.run = Some(self.run.clone());
        self.dub.cloud_usage = self.merged_usage();
        match result {
            Ok(()) => Ok(self.dub),
            Err(e) => Err(e),
        }
    }
}

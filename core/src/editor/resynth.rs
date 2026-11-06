//! Making speech again for the lines that need it (spec FR-005, FR-015, FR-017). Only those lines
//! reach the engine; each clip is swapped into its line when complete; the mix is rebuilt for the
//! sections that changed and spliced into the old mix file (research R3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::paths::{file_part, inside};
use super::post::make_line_clip;
use super::session::{EditSession, MadeClip, SynthJob};
use crate::cloud::audio::wav_duration_ms;
use crate::cloud::engine::SpeakJob;
use crate::cloud::guard::{guard_amount, used_chars};
use crate::cloud::services::{Gender as CloudGender, Step as CloudStep};
use crate::error::{
    AppError, CANCELLED, CLOUD_CONSENT_REQUIRED, CLOUD_NO_KEY, EDIT_WORK_FOLDER_MISSING,
    MODELS_MISSING,
};
use crate::pipeline::ffmpeg::Ffmpeg;
use crate::pipeline::mix::{mix, render_sections, splice};
use crate::pipeline::run::{cloud_note, CancelToken, CloudNote};
use crate::project::dub::TranscriptLine;
use crate::sidecar::{Request, Sidecar};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResynthState {
    Completed,
    Stopped,
    Failed,
}

#[derive(Debug)]
pub struct Resynth {
    pub state: ResynthState,
    /// Lines still changed or still needing the post chain, so the next call does exactly those.
    pub remaining_changed_ids: Vec<String>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub enum ResynthEvent {
    Progress {
        done: usize,
        total: usize,
        eta_seconds: Option<u64>,
    },
    /// A clip was swapped into its line.
    LineDone(TranscriptLine),
    Cloud(CloudNote),
}

pub type Shared = Arc<Mutex<EditSession>>;

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, EditSession> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

fn neutral(job: &SynthJob) -> bool {
    (job.speed - 1.0).abs() < 1e-9 && job.volume_db.abs() < 1e-9 && job.pitch_semitones.abs() < 1e-9
}

fn rel(work: &Path, path: &Path) -> String {
    path.strip_prefix(work)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// What the engines need, copied out of the session so no lock is held while they work.
struct Ctx {
    work: PathBuf,
    ffmpeg: Ffmpeg,
    sidecar: crate::sidecar::SidecarConfig,
    models_dir: PathBuf,
    params: Value,
    target_language: String,
    duration_ms: u64,
}

/// A progress event leaves at least this often while work runs, even if no line finished.
/// Under the contract's five seconds so a late timer tick cannot break it. Tests shorten it.
pub static HEARTBEAT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(4000);

fn heartbeat() -> Duration {
    Duration::from_millis(HEARTBEAT_MS.load(std::sync::atomic::Ordering::Relaxed))
}

struct Progress {
    total: usize,
    done: usize,
    started: Instant,
    last: Instant,
}

impl Progress {
    fn event(&mut self) -> ResynthEvent {
        self.last = Instant::now();
        let left = self.total.saturating_sub(self.done) as f64;
        let eta = (self.done > 0).then(|| {
            (self.started.elapsed().as_secs_f64() / self.done as f64 * left).round() as u64
        });
        ResynthEvent::Progress {
            done: self.done,
            total: self.total,
            eta_seconds: eta,
        }
    }
}

/// Everything a re-synthesis needs, checked before any work starts.
pub struct Prepared {
    jobs: Vec<SynthJob>,
    ctx: Ctx,
    cloud_service: Option<String>,
    cloud_rt: Option<Arc<crate::cloud::runtime::CloudRuntime>>,
    /// The ids asked for, if the caller named any.
    only: Option<Vec<String>>,
}

impl Prepared {
    pub fn line_count(&self) -> usize {
        self.jobs.len()
    }
}

/// Plan the work and check what it needs. A problem found here (no work folder, missing models,
/// no agreement for the amount) is an `Err` and nothing is written or sent.
pub fn prepare(shared: &Shared, ids: Option<Vec<String>>) -> Result<Prepared, AppError> {
    let s = lock(shared);
    let jobs = s.plan_synth(ids.as_deref());
    let rt = s.runtime();
    let cfg = rt.config();
    let work = match rt.work_folder() {
        Some(w) => w,
        None if jobs.is_empty() => PathBuf::new(),
        None => {
            return Err(AppError::new(
                EDIT_WORK_FOLDER_MISSING,
                "editor.error.workFolderMissing",
            ))
        }
    };
    let ctx = Ctx {
        work,
        ffmpeg: cfg.ffmpeg.clone(),
        sidecar: cfg.sidecar.clone(),
        models_dir: cfg.models_dir.clone(),
        params: with_code_map(&cfg.params, &cfg.code_map),
        target_language: s.dub().target_language.clone(),
        duration_ms: s.dub().source.as_ref().map(|x| x.duration_ms).unwrap_or(0),
    };
    let engine_jobs: Vec<&SynthJob> = jobs.iter().filter(|j| !j.post_only).collect();
    let cloud_service = rt.cloud_speak().map(String::from);
    if !engine_jobs.is_empty() {
        match &cloud_service {
            None => {
                let speakers: Vec<&str> =
                    engine_jobs.iter().map(|j| j.speaker_id.as_str()).collect();
                rt.check_models(&speakers)?;
            }
            Some(service) => {
                let cloud = rt
                    .cloud()
                    .ok_or_else(|| AppError::new(CLOUD_NO_KEY, "dub.error.cloudNoKey"))?;
                let chars: u64 = engine_jobs
                    .iter()
                    .map(|j| j.text.chars().count() as u64)
                    .sum();
                // No agreement yet, or the notice changed: the user is asked first.
                if cloud.consent.needs(service, CloudStep::Speak, 0)?.is_some() {
                    return Err(AppError::new(
                        CLOUD_CONSENT_REQUIRED,
                        "dub.error.cloudConsentRequired",
                    )
                    .with_details(json!({
                        "steps": [{ "step": "speak", "serviceId": service }],
                    })));
                }
                let used = used_chars(&s.dub().cloud_usage, CloudStep::Speak, service);
                guard_amount(
                    &cloud.consent,
                    &ctx.work,
                    CloudStep::Speak,
                    service,
                    used,
                    chars,
                    ctx.duration_ms,
                    ctx.duration_ms,
                )?;
            }
        }
    }
    let cloud_rt = rt.cloud().cloned();
    Ok(Prepared {
        jobs,
        ctx,
        cloud_service,
        cloud_rt,
        only: ids,
    })
}

/// Make speech again for `ids` (absent: every changed line, and every line that only needs the
/// post chain). Returns when the work is done, stopped or failed.
pub async fn resynthesize(
    shared: &Shared,
    ids: Option<Vec<String>>,
    cancel: CancelToken,
    on_event: &mut (dyn FnMut(ResynthEvent) + Send),
) -> Result<Resynth, AppError> {
    let prepared = prepare(shared, ids)?;
    Ok(run_prepared(shared, prepared, cancel, on_event).await)
}

/// Do the work `prepare` planned.
pub async fn run_prepared(
    shared: &Shared,
    prepared: Prepared,
    cancel: CancelToken,
    on_event: &mut (dyn FnMut(ResynthEvent) + Send),
) -> Resynth {
    let Prepared {
        jobs,
        ctx,
        cloud_service,
        cloud_rt,
        only: ids,
    } = prepared;
    let mut progress = Progress {
        total: jobs.len(),
        done: 0,
        started: Instant::now(),
        last: Instant::now(),
    };
    let mut state = ResynthState::Completed;
    let mut error: Option<AppError> = None;

    if !jobs.is_empty() {
        let tmp = ctx
            .work
            .join("edit")
            .join(&uuid::Uuid::new_v4().simple().to_string()[..8]);
        if let Err(e) = std::fs::create_dir_all(&tmp) {
            state = ResynthState::Failed;
            error = Some(AppError::io(e));
        }

        // 1. lines that only need speed, volume or pitch: no engine.
        for job in jobs.iter().filter(|j| j.post_only) {
            if state != ResynthState::Completed {
                break;
            }
            if cancel.is_cancelled() {
                state = ResynthState::Stopped;
                break;
            }
            match finish_line(shared, &ctx, job, None).await {
                Ok(Some(line)) => {
                    progress.done += 1;
                    on_event(ResynthEvent::LineDone(line));
                    on_event(progress.event());
                }
                Ok(None) => progress.done += 1,
                Err(e) => {
                    state = ResynthState::Failed;
                    error = Some(e);
                    break;
                }
            }
        }

        // 2. lines the engine has to speak.
        let engine_jobs: Vec<SynthJob> = jobs.iter().filter(|j| !j.post_only).cloned().collect();
        if state == ResynthState::Completed && !engine_jobs.is_empty() {
            let (outcome, usage) = run_engine(
                shared,
                &ctx,
                &engine_jobs,
                &tmp,
                &cancel,
                cloud_service.as_deref(),
                cloud_rt.as_ref(),
                &mut progress,
                on_event,
            )
            .await;
            if let Some(u) = usage {
                let run_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = lock(shared).record_cloud_usage(vec![u], &run_id) {
                    error.get_or_insert(e);
                }
            }
            if let Err(e) = outcome {
                if e.code == CANCELLED {
                    state = ResynthState::Stopped;
                } else {
                    state = ResynthState::Failed;
                    error = Some(e);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // 3. the mix shows what was made, even when the work was stopped half way.
    if let Err(e) = rebuild_mix(shared, &ctx).await {
        if state != ResynthState::Failed {
            state = ResynthState::Failed;
            error = Some(e);
        }
    }

    let remaining: Vec<String> = {
        let s = lock(shared);
        let mut ids = s.changed_ids();
        ids.extend(s.needs_post_ids());
        ids
    };
    let wanted: BTreeSet<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
    let remaining_changed_ids = remaining
        .into_iter()
        .filter(|id| ids.is_none() || wanted.contains(id.as_str()))
        .collect();
    Resynth {
        state,
        remaining_changed_ids,
        error,
    }
}

fn with_code_map(params: &Value, code_map: &Value) -> Value {
    let mut p = params.clone();
    if let Value::Object(map) = &mut p {
        map.insert("codeMap".into(), code_map.clone());
    }
    p
}

/// Run the engine for these jobs, swapping each clip into its line as soon as the file is
/// complete. Returns the engine's outcome and, for a cloud step, what it used.
#[allow(clippy::too_many_arguments)]
async fn run_engine(
    shared: &Shared,
    ctx: &Ctx,
    jobs: &[SynthJob],
    tmp: &Path,
    cancel: &CancelToken,
    cloud_service: Option<&str>,
    cloud_rt: Option<&Arc<crate::cloud::runtime::CloudRuntime>>,
    progress: &mut Progress,
    on_event: &mut (dyn FnMut(ResynthEvent) + Send),
) -> (
    Result<(), AppError>,
    Option<crate::project::dub::CloudUsage>,
) {
    let (note_tx, mut note_rx) = tokio::sync::mpsc::unbounded_channel::<CloudNote>();
    let mut pending: BTreeMap<String, &SynthJob> = jobs.iter().map(|j| (j.id.clone(), j)).collect();

    let engine = async {
        match (cloud_service, cloud_rt) {
            (Some(service), Some(rt)) => {
                let mut engine = match rt.engine(service, CloudStep::Speak) {
                    Ok(e) => e,
                    Err(e) => return (Err(e), None),
                };
                let language = ctx
                    .target_language
                    .split(['-', '_'])
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let speak: Vec<SpeakJob> = jobs
                    .iter()
                    .map(|j| SpeakJob {
                        id: j.id.clone(),
                        text: j.text.clone(),
                        language: language.clone(),
                        gender: if j.gender_male {
                            CloudGender::Male
                        } else {
                            CloudGender::Female
                        },
                        target_ms: j.target_ms,
                    })
                    .collect();
                let stop_token = cancel.clone();
                let stop = move || stop_token.is_cancelled();
                let service = service.to_string();
                let tx = note_tx.clone();
                let result = engine
                    .speak_lines(
                        &speak,
                        tmp,
                        &ctx.ffmpeg,
                        &mut |pe| {
                            if let crate::pipeline::run::RunEvent::Cloud(n) =
                                cloud_note(pe, "speak", &service)
                            {
                                let _ = tx.send(n);
                            }
                        },
                        &stop,
                    )
                    .await
                    .map(|_| ());
                (result, Some(engine.usage()))
            }
            _ => (run_local(ctx, jobs, tmp, cancel).await, None),
        }
    };
    tokio::pin!(engine);

    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut failed: Option<AppError> = None;
    let (result, usage) = loop {
        tokio::select! {
            r = &mut engine => break r,
            Some(note) = note_rx.recv() => on_event(ResynthEvent::Cloud(note)),
            _ = tick.tick() => {
                if progress.last.elapsed() >= heartbeat() {
                    on_event(progress.event());
                }
                if let Err(e) = collect_ready(shared, ctx, tmp, &mut pending, progress, on_event).await {
                    failed.get_or_insert(e);
                }
            }
        }
    };
    // Whatever the engine finished before it ended or was stopped still counts.
    if let Err(e) = collect_ready(shared, ctx, tmp, &mut pending, progress, on_event).await {
        failed.get_or_insert(e);
    }
    while let Ok(note) = note_rx.try_recv() {
        on_event(ResynthEvent::Cloud(note));
    }
    (failed.map_or(result, Err), usage)
}

/// One request to the sidecar's synthesize worker for all the lines, writing `<id>.wav` into
/// `tmp`. Stops when the token is set.
async fn run_local(
    ctx: &Ctx,
    jobs: &[SynthJob],
    tmp: &Path,
    cancel: &CancelToken,
) -> Result<(), AppError> {
    let mut models = serde_json::Map::new();
    let mut lines = Vec::new();
    for j in jobs {
        let voice = j
            .voice
            .as_ref()
            .ok_or_else(|| AppError::new(MODELS_MISSING, "dub.error.modelsMissing"))?;
        models.insert(
            voice.model.clone(),
            json!(ctx.models_dir.join(&voice.model)),
        );
        lines.push(json!({
            "id": j.id,
            "text": j.text,
            "voiceId": voice.model,
            "speakerId": voice.speaker_id,
            "lang": voice.lang,
            "lexicon": voice.lexicon,
            "targetMs": j.target_ms,
        }));
    }
    let espeak = ctx.models_dir.join("espeak-ng-data");
    if espeak.exists() {
        models.insert("espeak".into(), json!(espeak));
    }
    let req = Request {
        worker: "synthesize".into(),
        input: json!({ "lines": lines }),
        output: json!({ "dir": tmp, "prefix": "edit" }),
        models: Value::Object(models),
        params: ctx.params.clone(),
    };
    let mut cfg = ctx.sidecar.clone();
    if cfg.program.as_os_str().is_empty() {
        cfg.program = crate::models::runtime::python_exe(&ctx.models_dir)?;
    }
    let mut sidecar = Sidecar::start(cfg).await?;
    let handle = sidecar.cancel_handle();
    let token = cancel.clone();
    let watcher = tokio::spawn(async move {
        while !token.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        handle.cancel().await;
    });
    let result = sidecar.run_with_retry(req, &mut |_| {}).await.map(|_| ());
    watcher.abort();
    sidecar.shutdown().await;
    result
}

/// Swap in every clip the engine has finished and not yet handed over.
async fn collect_ready(
    shared: &Shared,
    ctx: &Ctx,
    tmp: &Path,
    pending: &mut BTreeMap<String, &SynthJob>,
    progress: &mut Progress,
    on_event: &mut (dyn FnMut(ResynthEvent) + Send),
) -> Result<(), AppError> {
    let ready: Vec<String> = pending
        .keys()
        .filter(|id| wav_duration_ms(&tmp.join(format!("{id}.wav"))).is_some())
        .cloned()
        .collect();
    for id in ready {
        let Some(job) = pending.remove(&id) else {
            continue;
        };
        let made = tmp.join(format!("{id}.wav"));
        match finish_line(shared, ctx, job, Some(&made)).await? {
            Some(line) => {
                progress.done += 1;
                on_event(ResynthEvent::LineDone(line));
                on_event(progress.event());
            }
            None => progress.done += 1,
        }
    }
    Ok(())
}

/// Turn a made clip into the line's clip and swap it in. `made` is the engine's file; `None`
/// for a post-only job, which starts from the line's raw clip. Returns the line, or `None` when
/// the line changed meanwhile and the new clip was dropped.
async fn finish_line(
    shared: &Shared,
    ctx: &Ctx,
    job: &SynthJob,
    made: Option<&Path>,
) -> Result<Option<TranscriptLine>, AppError> {
    let clips = ctx
        .work
        .join("sections")
        .join(format!("{:04}", job.section_index))
        .join("clips");
    std::fs::create_dir_all(&clips).map_err(AppError::io)?;

    let mut created: Vec<PathBuf> = Vec::new();
    let raw_abs = match made {
        Some(made) => {
            let raw = clips.join(format!("{}.{}.raw.wav", file_part(&job.id), job.raw_name));
            std::fs::rename(made, &raw).map_err(AppError::io)?;
            created.push(raw.clone());
            raw
        }
        None => match &job.raw_audio {
            Some(r) => match inside(&ctx.work, r) {
                Some(p) => p,
                None => return Ok(None),
            },
            None => return Ok(None),
        },
    };
    let raw_ms = wav_duration_ms(&raw_abs).unwrap_or(0);
    let (audio_abs, audio_ms) = if neutral(job) {
        (raw_abs.clone(), raw_ms)
    } else {
        let name = format!("{}.{}.wav", job.id, super::hash::hash8(&job.expected_hash));
        let out = clips.join(name);
        let ms = match make_line_clip(
            &ctx.ffmpeg,
            &raw_abs,
            &out,
            job.speed,
            job.volume_db,
            job.pitch_semitones,
        )
        .await
        {
            Ok(ms) => ms,
            Err(e) => {
                for f in &created {
                    let _ = std::fs::remove_file(f);
                }
                return Err(e);
            }
        };
        created.push(out.clone());
        (out, ms)
    };
    let clip = MadeClip {
        audio: rel(&ctx.work, &audio_abs),
        audio_ms,
        raw_audio: rel(&ctx.work, &raw_abs),
        raw_ms,
    };
    let swapped = lock(shared).swap_clip(job, clip);
    match swapped {
        Ok(Some((line, unused))) => {
            for path in unused {
                // Only files inside the work folder are ever removed.
                if let Some(p) = inside(&ctx.work, &path) {
                    let _ = std::fs::remove_file(p);
                }
            }
            Ok(Some(line))
        }
        Ok(None) => {
            for f in &created {
                let _ = std::fs::remove_file(f);
            }
            Ok(None)
        }
        Err(e) => {
            for f in &created {
                let _ = std::fs::remove_file(f);
            }
            Err(e)
        }
    }
}

/// Re-render the sections whose part of the mix is out of date and splice them into the old
/// mix file. With no mix file yet there is nothing to splice into, so nothing is done.
async fn rebuild_mix(shared: &Shared, ctx: &Ctx) -> Result<(), AppError> {
    let Some(plan) = lock(shared).mix_plan() else {
        return Ok(());
    };
    if plan.stale.is_empty() {
        return Ok(());
    }
    let old = ctx.work.join("dub-mix.wav");
    let original = ctx.work.join("audio-full.wav");
    if !old.exists() || !original.exists() {
        return Ok(());
    }
    let pieces = render_sections(
        &ctx.ffmpeg,
        &ctx.work,
        &original,
        &plan.sections,
        &plan.stale,
        plan.mute_original,
        &mut |_| {},
    )
    .await?;
    splice(&ctx.ffmpeg, &ctx.work, &old, &plan.sections, &pieces).await?;
    lock(shared).mix_rebuilt(&plan.stale, plan.epoch)
}

/// Build the whole mix again (a project whose mix file is gone). Not used by a normal edit.
pub async fn remix_all(shared: &Shared) -> Result<(), AppError> {
    let (plan, ffmpeg, work) = {
        let s = lock(shared);
        let rt = s.runtime();
        let Some(work) = rt.work_folder() else {
            return Err(AppError::new(
                EDIT_WORK_FOLDER_MISSING,
                "editor.error.workFolderMissing",
            ));
        };
        (s.mix_plan(), rt.config().ffmpeg.clone(), work)
    };
    let Some(plan) = plan else {
        return Ok(());
    };
    mix(
        &ffmpeg,
        &work,
        &work.join("audio-full.wav"),
        &plan.sections,
        plan.mute_original,
        &mut |_| {},
    )
    .await?;
    let all: Vec<usize> = (0..plan.sections.len()).collect();
    lock(shared).mix_rebuilt(&all, plan.epoch)
}

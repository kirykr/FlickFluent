//! Benchmarks: dub a fixed clip with the real models and record what each stage cost, so the
//! minimum and full-local specs rest on measurements. See `specs/002-core-dub-pipeline/data-model.md`
//! (Benchmark Record) and `docs/verification/002-benchmarks.md`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sysinfo::System;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{AppError, NO_AUDIO_TRACK, STAGE_FAILED};
use crate::hardware::{evaluate, Accelerator, MachineInput, Mode, OsInfo, SpecDefinition};
use crate::pipeline::run::{run_dub, CancelToken, Mode as RunMode, RunConfig, RunEvent};
use crate::pipeline::stages::{fingerprint, probe_source};
use crate::pipeline::state::WorkFolder;
use crate::project::dub::{DubData, Options, RunState, Source, Stage, StageStatus};

pub mod cli;

/// Fixture ids the benchmark knows. Both are French speech, dubbed into English.
pub const FIXTURES: &[&str] = &["clip-2min-fr", "clip-20s-fr"];
pub const DEFAULT_FIXTURE: &str = "clip-2min-fr";

/// The largest share of the spec's RAM that the heaviest stage may use. The rest is the
/// operating system, the app itself and whatever else the user has open.
pub const RAM_SHARE: f64 = 0.75;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchRecord {
    pub id: String,
    pub created_at: String,
    pub machine: BenchMachine,
    pub fixture: BenchFixture,
    pub results: Vec<BenchResult>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchMachine {
    pub os: String,
    pub os_version: Option<String>,
    pub arch: String,
    pub cpu_model: Option<String>,
    pub cores: Option<u32>,
    pub ram_gb: Option<f64>,
    pub free_disk_gb: Option<f64>,
    pub accelerator: String,
    pub simulated: bool,
    /// What the simulation held the run to, when `simulated`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulated_limits: Option<Limits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub cpu_cores: u32,
    pub ram_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchFixture {
    pub id: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchResult {
    pub stage: Stage,
    pub model_id: String,
    pub wall_ms: u64,
    /// Peak memory of the worker, from the sidecar. `None` for steps that run in the core.
    pub peak_ram_mb: Option<u64>,
    /// Processing time divided by media time: below 1 is faster than real time.
    pub real_time_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub can_run_minimum: bool,
    pub can_run_full_local: bool,
    pub notes: Vec<String>,
}

/// How to run one benchmark.
pub struct BenchOptions {
    pub fixture_id: String,
    pub fixture_path: PathBuf,
    /// A folder for the work files. Cleared when the run ends.
    pub work_dir: PathBuf,
    /// The numbers the run is held to, when this machine is made to look smaller.
    pub simulated: Option<Limits>,
    pub spec: SpecDefinition,
}

/// The real machine as a record entry. `limits` replaces cores and RAM for a simulated run, so a
/// record says what the run was held to, with the real figures kept in the cpu model line.
pub fn machine_info(input: &MachineInput, limits: Option<Limits>) -> BenchMachine {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    let accelerator = match input.accelerator {
        Some(Accelerator::None) => "none",
        Some(Accelerator::AppleSilicon) => "appleSilicon",
        Some(Accelerator::DiscreteGpu) => "discreteGpu",
        None => "unknown",
    };
    BenchMachine {
        os: input
            .os
            .as_ref()
            .map_or_else(|| std::env::consts::OS.to_string(), |o| o.family.clone()),
        os_version: input.os.as_ref().and_then(|o| o.version.clone()),
        arch: if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        }
        .to_string(),
        cpu_model: sys.cpus().first().map(|c| c.brand().trim().to_string()),
        cores: input.cpu_cores,
        ram_gb: input.ram_gb.map(round1),
        free_disk_gb: input.free_disk_gb.map(round1),
        accelerator: accelerator.to_string(),
        simulated: limits.is_some(),
        simulated_limits: limits,
    }
}

fn round1(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

/// The machine a simulated run stands for: the real one with the limits applied.
pub fn limited(input: &MachineInput, limits: Option<Limits>) -> MachineInput {
    let Some(l) = limits else {
        return input.clone();
    };
    MachineInput {
        cpu_cores: Some(l.cpu_cores),
        ram_gb: Some(l.ram_gb),
        // A simulated minimum machine has no accelerator either.
        accelerator: Some(Accelerator::None),
        os: input.os.as_ref().map(|o| OsInfo {
            family: o.family.clone(),
            version: o.version.clone(),
        }),
        ..input.clone()
    }
}

/// What the numbers say about the two specs.
pub fn verdict(
    results: &[BenchResult],
    completed: bool,
    machine: &MachineInput,
    spec: &SpecDefinition,
    total_rtf: f64,
) -> Verdict {
    let peak = results.iter().filter_map(|r| r.peak_ram_mb).max();
    let min_limit = spec.minimum.ram_gb * 1024.0 * RAM_SHARE;
    let full_limit = spec.full_local.ram_gb * 1024.0 * RAM_SHARE;
    let mut notes = Vec::new();
    if !completed {
        notes.push("The run did not finish, so nothing is proven.".to_string());
    }
    if let Some(p) = peak {
        notes.push(format!(
            "Heaviest stage used {p} MB; the minimum spec allows {:.0} MB ({:.0}% of {} GB) and the full-local spec {:.0} MB.",
            min_limit,
            RAM_SHARE * 100.0,
            spec.minimum.ram_gb,
            full_limit
        ));
    }
    notes.push(format!(
        "Whole dub ran at {total_rtf:.2}x real time (1.00 means one minute of video takes one minute)."
    ));
    let mode = evaluate(machine, spec).mode;
    if mode != Mode::FullLocal {
        notes.push(
            "This machine does not meet the full-local spec, so it says nothing about it."
                .to_string(),
        );
    }
    Verdict {
        can_run_minimum: completed && peak.is_some_and(|p| (p as f64) <= min_limit),
        can_run_full_local: completed
            && mode == Mode::FullLocal
            && peak.is_some_and(|p| (p as f64) <= full_limit),
        notes,
    }
}

#[derive(Default)]
struct Timing {
    started: Option<Instant>,
    wall_ms: u64,
}

/// Dub the fixture with the real models and measure it. `on_progress` gets `(stage, fraction)`.
pub async fn run_bench(
    mut cfg: RunConfig,
    machine_input: &MachineInput,
    opts: &BenchOptions,
    on_progress: &mut (dyn FnMut(Stage, f32) + Send),
) -> Result<BenchRecord, AppError> {
    let info = probe_source(&cfg.ffmpeg, &opts.fixture_path).await?;
    if !info.has_audio {
        return Err(AppError::new(NO_AUDIO_TRACK, "dub.error.noAudioTrack"));
    }
    let fp = fingerprint(&opts.fixture_path)?;
    let dub = DubData {
        source: Some(Source {
            path: opts.fixture_path.to_string_lossy().into_owned(),
            duration_ms: info.duration_ms,
            has_audio: info.has_audio,
            has_video: info.has_video,
            size_bytes: info.size_bytes,
            fingerprint: fp,
        }),
        source_language: "fr".into(),
        target_language: "en".into(),
        options: Options::default(),
        ..Default::default()
    };

    let id = uuid::Uuid::new_v4().to_string();
    std::fs::create_dir_all(&opts.work_dir).map_err(AppError::io)?;
    cfg.project_path = opts.work_dir.join(format!("bench-{}.flick", &id[..8]));
    cfg.project_id = id.clone();

    let stats: Arc<Mutex<Vec<crate::pipeline::run::StageStat>>> = Arc::default();
    let timings: Arc<Mutex<std::collections::BTreeMap<Stage, Timing>>> = Arc::default();
    let (sink_stats, sink_timings) = (stats.clone(), timings.clone());
    let started = Instant::now();
    let outcome = run_dub(
        &cfg,
        dub,
        RunMode::Fresh,
        CancelToken::default(),
        &mut |e| match e {
            RunEvent::Stat(s) => sink_stats.lock().expect("stats").push(s),
            RunEvent::Stage { stage, status } => {
                let mut t = sink_timings.lock().expect("timings");
                let entry = t.entry(stage).or_default();
                match status {
                    StageStatus::Running if entry.started.is_none() => {
                        entry.started = Some(Instant::now())
                    }
                    StageStatus::Done | StageStatus::Failed => {
                        if let Some(s) = entry.started.take() {
                            entry.wall_ms += s.elapsed().as_millis() as u64;
                        }
                    }
                    _ => {}
                }
            }
            RunEvent::Progress {
                stage,
                stage_progress,
                ..
            } => on_progress(stage, stage_progress),
            _ => {}
        },
    )
    .await;
    let total_ms = started.elapsed().as_millis() as u64;

    let work = WorkFolder::folder_for(&cfg.project_path);
    let completed = outcome
        .as_ref()
        .ok()
        .and_then(|d| d.run.as_ref())
        .is_some_and(|r| r.state == RunState::Completed);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&cfg.project_path);
    outcome?;

    let stats = stats.lock().expect("stats").clone();
    let timings = timings.lock().expect("timings");
    let media_ms = info.duration_ms.max(1) as f64;
    let results: Vec<BenchResult> = [
        Stage::Extract,
        Stage::Separate,
        Stage::Transcribe,
        Stage::Translate,
        Stage::Synthesize,
        Stage::Mix,
    ]
    .into_iter()
    .filter_map(|stage| {
        let of: Vec<_> = stats.iter().filter(|s| s.stage == stage).collect();
        if of.is_empty() && !matches!(stage, Stage::Extract | Stage::Mix) {
            return None;
        }
        // Worker stages are timed by the worker; the two that run in the core by the stage events.
        let (wall_ms, peak, model) = if of.is_empty() {
            let w = timings.get(&stage).map_or(0, |t| t.wall_ms);
            (w, None, "ffmpeg".to_string())
        } else {
            (
                of.iter().map(|s| s.wall_ms).sum(),
                of.iter().map(|s| s.peak_ram_mb).max(),
                of[0].model_id.clone(),
            )
        };
        Some(BenchResult {
            stage,
            model_id: model,
            wall_ms,
            peak_ram_mb: peak,
            real_time_factor: ((wall_ms as f64 / media_ms) * 1000.0).round() / 1000.0,
        })
    })
    .collect();
    if results.is_empty() {
        return Err(AppError::new(STAGE_FAILED, "dub.error.stageFailed"));
    }

    let effective = limited(machine_input, opts.simulated);
    let total_rtf = ((total_ms as f64 / media_ms) * 100.0).round() / 100.0;
    Ok(BenchRecord {
        id,
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
        machine: machine_info(&effective, opts.simulated),
        fixture: BenchFixture {
            id: opts.fixture_id.clone(),
            duration_ms: info.duration_ms,
        },
        verdict: verdict(&results, completed, &effective, &opts.spec, total_rtf),
        results,
    })
}

/// Where records live: one JSON file per run.
pub fn records_dir(base: &Path) -> PathBuf {
    base.join("bench")
}

/// A file name a person can sort: `bench-macos-arm64-simulated-20261105-1a2b3c4d.json`.
pub fn file_name(rec: &BenchRecord) -> String {
    let day: String = rec
        .created_at
        .chars()
        .filter(char::is_ascii_digit)
        .take(8)
        .collect();
    format!(
        "bench-{}-{}-{}-{}-{}.json",
        rec.machine.os,
        rec.machine.arch,
        if rec.machine.simulated {
            "simulated"
        } else {
            "real"
        },
        day,
        &rec.id[..8.min(rec.id.len())]
    )
}

pub fn save_record(dir: &Path, rec: &BenchRecord) -> Result<PathBuf, AppError> {
    std::fs::create_dir_all(dir).map_err(AppError::io)?;
    let path = dir.join(file_name(rec));
    let text = serde_json::to_string_pretty(rec)
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?;
    crate::fsutil::write_atomic(&path, text.as_bytes())?;
    Ok(path)
}

/// Every record in `dir`, oldest first. Files that are not records are skipped.
pub fn load_records(dir: &Path) -> Vec<BenchRecord> {
    let mut out: Vec<BenchRecord> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    out.sort_by(|a: &BenchRecord, b| a.created_at.cmp(&b.created_at));
    out
}

/// Find the fixture video: next to the app, in the source tree, or in `FLICK_FIXTURES_DIR`.
pub fn find_fixture(id: &str) -> Option<PathBuf> {
    let file = format!("{id}.mp4");
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("FLICK_FIXTURES_DIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe() {
        for a in exe.ancestors().skip(1).take(6) {
            roots.push(a.join("fixtures"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for a in cwd.ancestors().take(6) {
            roots.push(a.join("fixtures"));
        }
    }
    roots
        .into_iter()
        .map(|r| r.join(&file))
        .find(|p| p.is_file())
}

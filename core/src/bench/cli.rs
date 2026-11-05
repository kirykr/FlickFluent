//! `flickfluent --bench [--json] [options]`: the same benchmark the app runs, from a terminal.
//! It exits 0 on success, 1 when the run fails, 2 on bad arguments and 3 when models are missing
//! (it lists them and their sizes; `--download` fetches them first).
//!
//! Options: `--fixture <id>`, `--models <dir>`, `--out <dir>` (where the record is saved, the
//! default is `<app data>/bench`), `--simulate-cores <n> --simulate-ram-gb <n>` (hold the run to a
//! smaller machine and mark the record `simulated`), `--sidecar <launcher.py>`, `--python <exe>`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use super::{
    find_fixture, limited, records_dir, run_bench, save_record, BenchOptions, Limits,
    DEFAULT_FIXTURE, FIXTURES,
};
use crate::cli::CliOutput;
use crate::hardware::{probe, SpecDefinition};

use crate::models::{bundled_catalogue, installed};
use crate::models::{install_entry, Control, Outcome, Target};
use crate::pipeline::ffmpeg::Ffmpeg;
use crate::pipeline::plan::{machine_class, plan_models_full};
use crate::pipeline::preflight::preflight;
use crate::pipeline::run::RunConfig;
use crate::project::dub::{FallbackChoices, Stage};
use crate::sidecar::SidecarConfig;

pub const APP_ID: &str = "app.flickfluent.desktop";

pub fn is_bench(args: &[String]) -> bool {
    args.iter().any(|a| a == "--bench")
}

fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
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

/// The folder the app keeps its data in (the same one the window uses).
pub fn app_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    base.map(|b| b.join(APP_ID))
}

fn launcher(args: &[String]) -> PathBuf {
    if let Some(p) = value(args, "--sidecar") {
        return PathBuf::from(p);
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).take(6).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().take(6).map(Path::to_path_buf));
    }
    roots
        .into_iter()
        .flat_map(|r| {
            [
                r.join("sidecar/launcher.py"),
                r.join("resources/sidecar/launcher.py"),
            ]
        })
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("sidecar/launcher.py"))
}

fn fail(code: i32, message: String, json_out: bool) -> CliOutput {
    let stdout = if json_out {
        json!({ "ok": false, "error": message }).to_string() + "\n"
    } else {
        format!("{message}\n")
    };
    CliOutput {
        exit_code: code,
        stdout,
    }
}

pub async fn run(args: &[String]) -> CliOutput {
    let show = !args.iter().any(|a| a == "--json");
    run_with(args, &mut |stage, f| {
        if show && (f - 1.0).abs() < f32::EPSILON {
            eprintln!("{stage:?} done");
        }
    })
    .await
}

/// Like `run`, with a callback for stage progress (the app sends it to the window).
pub async fn run_with(
    args: &[String],
    on_progress: &mut (dyn FnMut(Stage, f32) + Send),
) -> CliOutput {
    let json_out = args.iter().any(|a| a == "--json");
    let fixture_id = value(args, "--fixture").map_or(DEFAULT_FIXTURE, String::as_str);
    if !FIXTURES.contains(&fixture_id) {
        return fail(
            2,
            format!(
                "unknown fixture '{fixture_id}'; choose one of {}",
                FIXTURES.join(", ")
            ),
            json_out,
        );
    }
    let Some(fixture_path) = find_fixture(fixture_id) else {
        return fail(
            2,
            format!("fixture {fixture_id}.mp4 not found; set FLICK_FIXTURES_DIR"),
            json_out,
        );
    };
    let data = app_data_dir();
    let Some(models) = value(args, "--models")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("FLICK_MODELS_DIR").map(PathBuf::from))
        .or_else(|| data.as_ref().map(|d| d.join("models")))
    else {
        return fail(
            2,
            "cannot tell where the models are; pass --models".into(),
            json_out,
        );
    };
    let out_dir = value(args, "--out")
        .map(PathBuf::from)
        .or_else(|| data.as_ref().map(|d| records_dir(d)))
        .unwrap_or_else(|| PathBuf::from("bench"));

    let limits = match (
        value(args, "--simulate-cores"),
        value(args, "--simulate-ram-gb"),
    ) {
        (None, None) => None,
        (Some(c), Some(r)) => match (c.parse::<u32>(), r.parse::<f64>()) {
            (Ok(cpu_cores), Ok(ram_gb)) if cpu_cores > 0 && ram_gb > 0.0 => {
                Some(Limits { cpu_cores, ram_gb })
            }
            _ => {
                return fail(
                    2,
                    "--simulate-cores and --simulate-ram-gb need positive numbers".into(),
                    json_out,
                )
            }
        },
        _ => {
            return fail(
                2,
                "give both --simulate-cores and --simulate-ram-gb".into(),
                json_out,
            )
        }
    };

    let real = probe();
    let machine = limited(&real, limits);
    let class = machine_class(&machine);
    let spec = SpecDefinition::bundled();
    let catalogue = bundled_catalogue();
    let have: Vec<String> = installed(&models).into_iter().map(|m| m.id).collect();
    let plan = plan_models_full(
        &catalogue,
        class,
        os_name(),
        arch_name(),
        "fr",
        "en",
        false,
        &have,
        &FallbackChoices::default(),
    );
    let work_root = std::env::temp_dir().join("flickfluent-bench");
    let pre = preflight(
        &catalogue,
        &plan,
        &models,
        &work_root,
        class,
        os_name(),
        arch_name(),
        "fr",
        "en",
        false,
        0,
    );
    if !pre.missing_models.is_empty() {
        let list: Vec<String> = pre
            .missing_models
            .iter()
            .map(|m| format!("  {} ({} bytes)", m.id, m.size_bytes))
            .collect();
        if !args.iter().any(|a| a == "--download") {
            return fail(
                3,
                format!(
                    "Missing models for a {class:?} machine ({} bytes to download):\n{}\nRun again with --download to fetch them.",
                    pre.download_bytes,
                    list.join("\n")
                ),
                json_out,
            );
        }
        for m in &pre.missing_models {
            let Some(entry) = catalogue.iter().find(|e| e.id == m.id) else {
                continue;
            };
            eprintln!("downloading {} ...", m.id);
            let target = Target {
                models_dir: &models,
                os: os_name(),
                arch: arch_name(),
                sidecar_args: vec![launcher(args).into_os_string()],
            };
            match install_entry(entry, &target, &Control::new(), &mut |_| {}).await {
                Ok(Outcome::Completed(_)) => {}
                Ok(_) => return fail(1, format!("download of {} did not finish", m.id), json_out),
                Err(e) => {
                    return fail(
                        1,
                        format!("download of {} failed: {}", m.id, e.message_key),
                        json_out,
                    )
                }
            }
        }
    }

    let work = std::env::temp_dir().join("flickfluent-bench");
    let mut allow: Vec<OsString> = vec![
        launcher(args).into_os_string(),
        "--allow".into(),
        models.as_os_str().to_owned(),
        "--allow".into(),
        work.as_os_str().to_owned(),
    ];
    // The sidecar resolves links before checking its allow-list, so allow where they lead.
    for e in std::fs::read_dir(&models).into_iter().flatten().flatten() {
        if let Ok(real) = e.path().canonicalize() {
            allow.push("--allow".into());
            allow.push(real.into_os_string());
        }
    }
    let threads = limits.map_or_else(
        || real.cpu_cores.unwrap_or(4).saturating_sub(1).max(1),
        |l| l.cpu_cores.saturating_sub(1).max(1),
    );
    let cfg = RunConfig {
        cloud: None,
        project_path: work.join("bench.flick"),
        project_id: "bench".into(),
        models_dir: models.clone(),
        catalogue: catalogue.clone(),
        class,
        os: os_name().into(),
        arch: arch_name().into(),
        ffmpeg: Ffmpeg::locate(&models).unwrap_or_else(|| Ffmpeg::at(Path::new(""))),
        sidecar: SidecarConfig {
            program: value(args, "--python")
                .map(PathBuf::from)
                .unwrap_or_default(),
            args: allow,
            hang_timeout: Duration::from_secs(300),
            start_timeout: Duration::from_secs(120),
        },
        code_map: serde_json::from_str(include_str!("../../../config/language-codes.json"))
            .unwrap_or(json!({})),
        // A 1 MB ceiling makes the sidecar restart after every call, so each stage's peak
        // memory is its own and not the highest of everything that ran before it.
        params: json!({ "device": "cpu", "computeType": "int8", "threads": threads, "ramCeilingMb": 1 }),
    };
    let opts = BenchOptions {
        fixture_id: fixture_id.to_string(),
        fixture_path,
        work_dir: work,
        simulated: limits,
        spec,
    };
    let record = match run_bench(cfg, &real, &opts, on_progress).await {
        Ok(r) => r,
        Err(e) => {
            return fail(
                1,
                format!("benchmark failed: {} {:?}", e.code, e.details),
                json_out,
            )
        }
    };
    let saved = match save_record(&out_dir, &record) {
        Ok(p) => p,
        Err(e) => {
            return fail(
                1,
                format!("could not save the record: {}", e.message_key),
                json_out,
            )
        }
    };
    let stdout = if json_out {
        serde_json::to_string_pretty(&record).unwrap_or_default() + "\n"
    } else {
        let mut text = format!(
            "{} {} {} · {} cores · {} GB · {}\nfixture {} ({} s)\n",
            record.machine.os,
            record.machine.arch,
            record.machine.cpu_model.clone().unwrap_or_default(),
            record.machine.cores.map_or("?".into(), |c| c.to_string()),
            record.machine.ram_gb.map_or("?".into(), |c| c.to_string()),
            if record.machine.simulated {
                "SIMULATED"
            } else {
                "real"
            },
            record.fixture.id,
            record.fixture.duration_ms / 1000,
        );
        for r in &record.results {
            text += &format!(
                "  {:<11} {:<28} {:>8} ms  {:>7} MB  {:.3}x\n",
                format!("{:?}", r.stage).to_lowercase(),
                r.model_id,
                r.wall_ms,
                r.peak_ram_mb.map_or("-".into(), |m| m.to_string()),
                r.real_time_factor
            );
        }
        text += &format!(
            "can run minimum: {}  can run full-local: {}\n",
            record.verdict.can_run_minimum, record.verdict.can_run_full_local
        );
        for n in &record.verdict.notes {
            text += &format!("  - {n}\n");
        }
        text += &format!("saved {}\n", saved.display());
        text
    };
    CliOutput {
        exit_code: 0,
        stdout,
    }
}

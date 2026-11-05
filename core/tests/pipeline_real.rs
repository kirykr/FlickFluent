//! The whole dub with real models: French clip in, English dub out. Slow and needs downloads, so
//! it only runs when asked:
//!
//!   FLICK_TEST_MODELS=<folder> FLICK_TEST_PYTHON=<python with the worker packages> \
//!     cargo test -p flickfluent-core --test pipeline_real -- --ignored --nocapture
//!
//! `FLICK_TEST_MODELS` holds the catalogue's model folders by their catalogue ids (symlinks to
//! smaller models are fine): `whisper-small`, `m2m100-418m`, `htdemucs`, `speaker-diarization`,
//! `kokoro-multi-lang-v1_0`, `espeak-ng-data`. `FLICK_TEST_CLIP` picks another fixture.
#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use flickfluent_core::models::{bundled_catalogue, MachineClass};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunConfig};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{LineFlag, RunState};
use flickfluent_core::sidecar::SidecarConfig;
use serde_json::json;
use support::run_fixture::{dub_for, ffmpeg};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs real models; see the file header"]
async fn french_clip_becomes_an_english_dub_with_real_models() {
    let models = PathBuf::from(std::env::var("FLICK_TEST_MODELS").expect("FLICK_TEST_MODELS"));
    let python = std::env::var("FLICK_TEST_PYTHON").expect("FLICK_TEST_PYTHON");
    let root = tempfile::tempdir().unwrap();
    let launcher = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sidecar/launcher.py");
    // The sidecar resolves symlinks before checking its allow-list, so allow where they lead.
    let mut allow: Vec<std::ffi::OsString> = vec![
        root.path().as_os_str().to_owned(),
        models.as_os_str().to_owned(),
    ];
    for e in std::fs::read_dir(&models).unwrap().flatten() {
        if let Ok(real) = e.path().canonicalize() {
            allow.push(real.into_os_string());
        }
    }
    let mut args: Vec<std::ffi::OsString> = vec![launcher.into_os_string()];
    for a in allow {
        args.push("--allow".into());
        args.push(a);
    }
    let cfg = RunConfig {
        project_path: root.path().join("real.flick"),
        project_id: "real".into(),
        models_dir: models.clone(),
        catalogue: bundled_catalogue(),
        class: MachineClass::Minimum,
        os: "macos".into(),
        arch: "arm64".into(),
        ffmpeg: ffmpeg(),
        sidecar: SidecarConfig {
            program: PathBuf::from(python),
            args,
            hang_timeout: Duration::from_secs(300),
            start_timeout: Duration::from_secs(60),
        },
        code_map: serde_json::from_str(include_str!("../../config/language-codes.json")).unwrap(),
        params: json!({ "device": "cpu", "computeType": "int8", "threads": 4 }),
    };
    let clip = std::env::var("FLICK_TEST_CLIP").unwrap_or_else(|_| "clip-20s-fr.mp4".into());
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(clip);
    let dub = dub_for(&source, "en").await;
    let started = std::time::Instant::now();
    let out = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |e| {
        if let flickfluent_core::pipeline::run::RunEvent::Stage { stage, status } = e {
            println!(
                "{:>6.1}s {stage:?} {status:?}",
                started.elapsed().as_secs_f32()
            );
        }
    })
    .await
    .unwrap();

    let run = out.run.as_ref().unwrap();
    assert_eq!(run.state, RunState::Completed, "{:?}", run.error);
    for l in &out.lines {
        println!(
            "{:>6} {:>6} {} | {} -> {} ({} ms) {:?}",
            l.start_ms,
            l.end_ms,
            l.speaker_id,
            l.text,
            l.translated.as_deref().unwrap_or(""),
            l.audio_ms.unwrap_or(0),
            l.flags
        );
    }
    assert!(out.lines.len() >= 2);
    assert!(out.lines.iter().all(|l| l
        .translated
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty())));
    assert!(out
        .lines
        .iter()
        .all(|l| l.audio_ms.is_some_and(|ms| ms > 200)));
    assert!(
        out.speakers.len() >= 2,
        "two voices in the clip: {:?}",
        out.speakers
    );
    assert!(!out
        .lines
        .iter()
        .any(|l| l.flags.contains(&LineFlag::Overlap)));

    let work = WorkFolder::folder_for(&cfg.project_path);
    let mixed = work.join("dub-mix.wav");
    assert!(mixed.exists());
    let duration = |args: &[&str]| -> String {
        let out = std::process::Command::new("ffmpeg")
            .args(["-nostdin", "-hide_banner"])
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    let m = mixed.to_string_lossy().into_owned();
    let log = duration(&["-i", &m, "-af", "volumedetect", "-f", "null", "-"]);
    let mean: f64 = log
        .lines()
        .find_map(|l| l.split("mean_volume:").nth(1))
        .and_then(|v| v.trim().trim_end_matches(" dB").parse().ok())
        .expect("volumedetect");
    assert!(mean > -45.0, "the mix is not silent: {mean} dB");
    let first = &out.lines[0];
    let window = format!("{:.2}", first.start_ms as f64 / 1000.0);
    let log = duration(&[
        "-ss",
        &window,
        "-t",
        "2",
        "-i",
        &m,
        "-af",
        "volumedetect",
        "-f",
        "null",
        "-",
    ]);
    let speech: f64 = log
        .lines()
        .find_map(|l| l.split("mean_volume:").nth(1))
        .and_then(|v| v.trim().trim_end_matches(" dB").parse().ok())
        .expect("volumedetect");
    assert!(
        speech > -40.0,
        "dub is audible at the first line: {speech} dB"
    );
    println!("total {:.1}s", started.elapsed().as_secs_f32());
}

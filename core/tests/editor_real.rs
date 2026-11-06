//! The editor with real models: edit one line of the 2-minute clip, make it again, print the time
//! and how many clip files changed (exactly one), then run Smart Fit on every flagged line. Slow
//! and needs downloads, so it only runs when asked, like `pipeline_real.rs`:
//!
//!   FLICK_TEST_MODELS=<folder> FLICK_TEST_PYTHON=<python with the worker packages> \
//!     cargo test -p flickfluent-core --test editor_real -- --ignored --nocapture
//!
//! Listen to the join at the splice and to pitch +4 and -4 on one line (the files are kept in
//! the folder printed at the end), and look at the numbers against SC-001 (under 15 s on the
//! full-local machine) and SC-002 (at least 90% of the lines that need no more than 1.30
//! fitted). Record the results in `docs/verification/004.md`.
#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flickfluent_core::editor::fit::{fit, FitMode};
use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::editor::resynth::resynthesize;
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::session::EditSession;
use flickfluent_core::models::{bundled_catalogue, MachineClass};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunConfig};
use flickfluent_core::project::dub::LineFlag;
use flickfluent_core::sidecar::SidecarConfig;
use serde_json::json;
use support::run_fixture::{dub_for, ffmpeg};

fn config(root: &Path, models: &Path, python: String) -> RunConfig {
    let launcher = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sidecar/launcher.py");
    let mut args: Vec<std::ffi::OsString> = vec![launcher.into_os_string()];
    let mut allow = vec![root.as_os_str().to_owned(), models.as_os_str().to_owned()];
    for e in std::fs::read_dir(models).unwrap().flatten() {
        if let Ok(real) = e.path().canonicalize() {
            allow.push(real.into_os_string());
        }
    }
    for a in allow {
        args.push("--allow".into());
        args.push(a);
    }
    RunConfig {
        cloud: None,
        project_path: root.join("real.flick"),
        project_id: "real".into(),
        models_dir: models.to_path_buf(),
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
    }
}

fn clip_files(work: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    fn walk(dir: &Path, base: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else if p.to_string_lossy().contains("clips") {
                out.insert(
                    p.strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    walk(&work.join("sections"), work, &mut out);
    out
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs real models; see the file header"]
async fn one_edited_line_is_made_again_and_flagged_lines_are_fitted() {
    let models = PathBuf::from(std::env::var("FLICK_TEST_MODELS").expect("FLICK_TEST_MODELS"));
    let python = std::env::var("FLICK_TEST_PYTHON").expect("FLICK_TEST_PYTHON");
    let root = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-2min-fr.mp4");
    let dub = dub_for(&source, "en").await;
    let cfg = config(root.path(), &models, python.clone());
    let dub = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    let work = flickfluent_core::pipeline::state::WorkFolder::folder_for(&cfg.project_path);

    let rt = EditorRuntime::for_project(config(root.path(), &models, python), &dub);
    let session = EditSession::open(rt, dub, Box::new(|_| Ok(()))).unwrap();
    let shared = Arc::new(Mutex::new(session));

    // SC-001: one edited line, audible again.
    let id = shared.lock().unwrap().dub().lines[3].id.clone();
    let before = clip_files(&work);
    shared
        .lock()
        .unwrap()
        .apply(EditOp::UpdateLine {
            id,
            patch: LinePatch {
                translated: Some("This sentence was typed by hand.".into()),
                ..Default::default()
            },
        })
        .unwrap();
    let started = Instant::now();
    let out = resynthesize(&shared, None, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    let took = started.elapsed();
    let after = clip_files(&work);
    let changed = after
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .count();
    println!(
        "re-synthesis: {:.1} s, {changed} clip file(s) changed, state {:?}",
        took.as_secs_f32(),
        out.state
    );
    assert_eq!(changed, 1, "exactly one clip file changes");

    // SC-002: Smart Fit on every flagged line.
    let flagged = shared
        .lock()
        .unwrap()
        .dub()
        .lines
        .iter()
        .filter(|l| l.flags.contains(&LineFlag::TooLong))
        .count();
    let report = fit(
        &shared,
        FitMode::Smart,
        None,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    println!(
        "smart fit: {flagged} lines flagged tooLong, {} fitted, {} still flagged",
        report.fitted, report.still_flagged
    );
    println!("work folder kept for listening: {}", work.display());
    std::mem::forget(root);
}

//! An edit and a later dub run (spec FR-020, research R16): a resume keeps edited sections, a
//! fresh run asks before it replaces them.

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::session::EditSession;
use flickfluent_core::error::DUB_EDITS_WOULD_BE_LOST;
use flickfluent_core::models::MachineClass;
use flickfluent_core::pipeline::plan::plan_models;
use flickfluent_core::pipeline::preflight::preflight;
use flickfluent_core::pipeline::run::{
    check_fresh_start, edited_lines, run_dub, CancelToken, Mode,
};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{DubData, RunState};
use serde_json::json;
use support::run_fixture::{catalogue, config, dub_for, long_clip};

fn stamps(dir: &Path) -> BTreeMap<String, SystemTime> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, SystemTime>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.insert(
                    p.strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    e.metadata().unwrap().modified().unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// A 12-minute dub stopped while it translated section 1: section 0 is finished.
async fn stopped_run(root: &Path) -> (DubData, PathBuf) {
    let source = long_clip(root, 12);
    let cfg = config(
        root,
        "c.flick",
        json!({ "slowWorker": "translate", "slowSection": "0001", "slowSeconds": 60 }),
    );
    let dub = dub_for(&source, "en").await;
    let cancel = CancelToken::default();
    let trigger = cancel.clone();
    let work = WorkFolder::folder_for(&cfg.project_path);
    let watched = work.clone();
    let watcher = tokio::spawn(async move {
        while !watched.join("sections/0001/lines.json").exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(700)).await;
        trigger.cancel();
    });
    let out = run_dub(&cfg, dub, Mode::Fresh, cancel, &mut |_| {})
        .await
        .unwrap();
    watcher.await.unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Cancelled);
    (out, work)
}

fn edit_first_line(root: &Path, dub: DubData) -> EditSession {
    let rt = EditorRuntime::for_project(config(root, "c.flick", json!({})), &dub);
    let mut session = EditSession::open(rt, dub, Box::new(|_| Ok(()))).unwrap();
    let id = session.dub().lines[0].id.clone();
    session
        .apply(EditOp::UpdateLine {
            id,
            patch: LinePatch {
                translated: Some("Edited by hand".into()),
                ..Default::default()
            },
        })
        .unwrap();
    session
}

#[tokio::test]
async fn resuming_keeps_an_edited_section_and_finishes_the_rest() {
    let root = tempfile::tempdir().unwrap();
    let (stopped, work) = stopped_run(root.path()).await;
    let session = edit_first_line(root.path(), stopped);
    let sections = &session.dub().run.as_ref().unwrap().sections;
    assert!(sections[0].edited, "an accepted edit marks the section");
    assert!(sections.iter().skip(1).all(|s| !s.edited));

    let before = stamps(&work.join("sections/0000"));
    let fast = config(root.path(), "c.flick", json!({}));
    let out = run_dub(
        &fast,
        session.dub().clone(),
        Mode::Resume,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert_eq!(out.lines[0].translated.as_deref(), Some("Edited by hand"));
    assert!(out.lines[0].user_edited);
    assert_eq!(
        stamps(&work.join("sections/0000")),
        before,
        "no stage touched the edited section"
    );
    assert!(out.run.as_ref().unwrap().sections[0].edited);
    // The other sections were made by the run and are not marked.
    assert!(out
        .lines
        .iter()
        .any(|l| l.section_index > 0 && l.audio.is_some()));
    assert!(work.join("dub-mix.wav").exists());
}

#[tokio::test]
async fn a_fresh_start_asks_before_it_replaces_edited_lines() {
    let root = tempfile::tempdir().unwrap();
    let (stopped, work) = stopped_run(root.path()).await;
    let session = edit_first_line(root.path(), stopped);
    let dub = session.dub().clone();
    assert_eq!(edited_lines(&dub), 1);

    let before = stamps(&work);
    let err = check_fresh_start(&dub, false).unwrap_err();
    assert_eq!(err.code, DUB_EDITS_WOULD_BE_LOST);
    assert_eq!(err.details.unwrap()["editedLines"], 1);
    assert_eq!(stamps(&work), before, "nothing was deleted");

    check_fresh_start(&dub, true).unwrap();
    let fast = config(root.path(), "c.flick", json!({}));
    let out = run_dub(&fast, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert!(out.lines[0]
        .translated
        .as_deref()
        .unwrap()
        .starts_with("T:"));
    assert!(out.run.as_ref().unwrap().sections.iter().all(|s| !s.edited));
    assert_eq!(edited_lines(&out), 0);
}

#[tokio::test]
async fn nothing_is_asked_when_nothing_was_edited() {
    let root = tempfile::tempdir().unwrap();
    let dub = dub_for(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-20s-fr.mp4"),
        "en",
    )
    .await;
    assert_eq!(edited_lines(&dub), 0);
    check_fresh_start(&dub, false).unwrap();
    let _ = root;
}

#[test]
fn the_preflight_carries_the_count() {
    let dir = tempfile::tempdir().unwrap();
    let cat = catalogue();
    let plan = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
    );
    let mut pre = preflight(
        &cat,
        &plan,
        dir.path(),
        dir.path(),
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
        1000,
    );
    let mut dub = DubData {
        lines: vec![Default::default(), Default::default(), Default::default()],
        ..Default::default()
    };
    dub.lines[0].user_edited = true;
    dub.lines[2].user_edited = true;
    pre.edits_will_be_lost = edited_lines(&dub);
    assert_eq!(serde_json::to_value(&pre).unwrap()["editsWillBeLost"], 2);
}

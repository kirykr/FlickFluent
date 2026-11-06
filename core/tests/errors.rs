//! Every dub error has a message and a next step the user can read, and the two failures that
//! need care (a source that disappears, a full disk) stop cleanly and keep finished work.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use flickfluent_core::error::*;
use flickfluent_core::pipeline::errors::{next_step_key, DUB_ERRORS};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunEvent};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{RunState, Stage};
use serde_json::{json, Value};
use support::run_fixture::{config, dub_for, long_clip};

fn en() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/en.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn has_key(tree: &Value, key: &str) -> bool {
    key.split('.')
        .try_fold(tree, |node, part| node.get(part))
        .is_some_and(Value::is_string)
}

/// The codes `contracts/ipc-commands.md` lists for the dub commands.
const CONTRACT_CODES: &[&str] = &[
    UNREADABLE_FILE,
    NO_AUDIO_TRACK,
    NO_SPEECH_FOUND,
    SAME_LANGUAGE,
    NO_FALLBACK_CHOSEN,
    MODELS_MISSING,
    NO_SPACE,
    MACHINE_TOO_WEAK,
    SIDECAR_START_FAILED,
    SIDECAR_CRASHED,
    STAGE_FAILED,
    SOURCE_CHANGED,
    NOTHING_TO_RESUME,
    FILE_EXISTS,
    RUN_NOT_COMPLETE,
    // feature 003
    CLOUD_CONSENT_REQUIRED,
    CLOUD_NO_KEY,
    CLOUD_KEYS_EXHAUSTED,
    CLOUD_AUTH_FAILED,
    CLOUD_OFFLINE,
    CLOUD_BAD_REPLY,
    KEY_INVALID,
    KEY_DUPLICATE,
    KEY_STORE_UNAVAILABLE,
];

#[test]
fn every_contract_code_has_a_message_and_a_next_step_in_english() {
    let tree = en();
    for code in CONTRACT_CODES {
        let (_, message, next) = DUB_ERRORS
            .iter()
            .find(|(c, _, _)| c == code)
            .unwrap_or_else(|| panic!("{code} missing from DUB_ERRORS"));
        assert!(has_key(&tree, message), "{code}: no en.json key {message}");
        assert!(has_key(&tree, next), "{code}: no en.json key {next}");
        assert_eq!(next_step_key(code), *next);
    }
}

#[test]
fn every_listed_error_has_real_keys_and_no_code_is_listed_twice() {
    let tree = en();
    let mut seen = std::collections::HashSet::new();
    for (code, message, next) in DUB_ERRORS {
        assert!(seen.insert(*code), "{code} listed twice");
        assert!(has_key(&tree, message), "{code}: no en.json key {message}");
        assert!(has_key(&tree, next), "{code}: no en.json key {next}");
    }
}

#[test]
fn every_message_with_a_placeholder_names_one_the_run_can_fill() {
    // The UI fills {{stage}}, {{needed}}, {{free}}, {{missing}} and, for cloud steps, {{service}}, {{labels}} and {{detail}}; nothing else is promised.
    let tree = en();
    let known = [
        "stage",
        "needed",
        "free",
        "missing",
        "shortfall",
        "service",
        "labels",
        "detail",
        "seconds",
        "count",
    ];
    for (code, message, _) in DUB_ERRORS {
        let text = message
            .split('.')
            .try_fold(&tree, |n, p| n.get(p))
            .and_then(Value::as_str)
            .unwrap();
        for part in text.split("{{").skip(1) {
            let name = part.split("}}").next().unwrap();
            assert!(known.contains(&name), "{code}: unknown placeholder {name}");
        }
    }
}

type Events = Arc<Mutex<Vec<RunEvent>>>;

fn collect() -> (Events, Events) {
    let log = Arc::new(Mutex::new(Vec::new()));
    (log.clone(), log)
}

#[tokio::test]
async fn a_source_removed_mid_run_fails_the_stage_that_needed_it_and_names_it() {
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 12);
    let cfg = config(root.path(), "gone.flick", json!({}));
    let dub = dub_for(&source, "en").await;
    let (log, sink) = collect();
    let victim = source.clone();
    let err = run_dub(
        &cfg,
        dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| {
            // Pull the file as soon as the first section has finished.
            if matches!(&e, RunEvent::Line(l) if l.section_index == 0) {
                let _ = std::fs::remove_file(&victim);
            }
            sink.lock().unwrap().push(e);
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, STAGE_FAILED);
    let details = err.details.clone().unwrap();
    assert!(
        details["stage"].is_string(),
        "the error names the stage: {details}"
    );
    let events = log.lock().unwrap();
    let reported = events
        .iter()
        .find_map(|e| match e {
            RunEvent::Error(r) => Some(r.clone()),
            _ => None,
        })
        .expect("dub:error was sent");
    assert_eq!(reported.code, STAGE_FAILED);
    assert!(reported.stage.is_some());
    assert_eq!(reported.message_key, "dub.error.sourceMissing");
    assert_eq!(reported.next_step_key, "dub.next.chooseAnotherFile");
    let tree = en();
    assert!(has_key(&tree, &reported.message_key) && has_key(&tree, &reported.next_step_key));
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::Done(RunState::Failed))));
}

#[tokio::test]
async fn a_full_disk_stops_cleanly_with_the_numbers_and_keeps_finished_work() {
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 12);
    let full = json!({ "diskFullWorker": "transcribe", "diskFullSection": "0001" });
    let cfg = config(root.path(), "full.flick", full);
    let dub = dub_for(&source, "en").await;
    let (log, sink) = collect();
    let err = run_dub(
        &cfg,
        dub.clone(),
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| sink.lock().unwrap().push(e),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, NO_SPACE);
    let d = err.details.clone().unwrap();
    let (needed, free, short) = (
        d["neededBytes"].as_u64().unwrap(),
        d["freeBytes"].as_u64().unwrap(),
        d["shortfallBytes"].as_u64().unwrap(),
    );
    assert!(needed > 0);
    assert_eq!(short, needed.saturating_sub(free), "the shortfall is exact");

    // Section 0 is finished and kept; the run is failed, not lost.
    let work = WorkFolder::folder_for(&cfg.project_path);
    assert!(work.join("sections/0000/synth.json").exists());
    let run = WorkFolder::open(&cfg.project_path)
        .unwrap()
        .load_run()
        .unwrap()
        .unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(run.error.as_ref().unwrap().code, NO_SPACE);
    assert_eq!(run.error.as_ref().unwrap().stage, Some(Stage::Transcribe));
    assert_eq!(run.error.unwrap().next_step_key, "dub.next.freeDiskSpace");
    assert!(log
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, RunEvent::Done(RunState::Failed))));

    // Space freed: resuming finishes without redoing section 0.
    let before = std::fs::metadata(work.join("sections/0000/synth.json"))
        .unwrap()
        .modified()
        .unwrap();
    let ok = config(root.path(), "full.flick", json!({}));
    let out = run_dub(&ok, dub, Mode::Resume, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(out.run.unwrap().state, RunState::Completed);
    let after = std::fs::metadata(work.join("sections/0000/synth.json"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before, after);
}

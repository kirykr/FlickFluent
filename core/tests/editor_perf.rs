//! A long film: 6,000 lines open, search, replace and undo fast (`plan.md` budgets).

mod support;

use std::time::Instant;

use flickfluent_core::editor::ops::EditOp;
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::search::{find, Field, SearchOptions};
use flickfluent_core::editor::session::EditSession;
use flickfluent_core::project::dub::{DubData, Speaker, SpeakerGender};
use serde_json::json;
use support::editor_fixture::line;
use support::run_fixture::config;

fn long() -> DubData {
    let mut d = DubData {
        target_language: "en".into(),
        source_language: "fr".into(),
        speakers: vec![Speaker {
            id: "spk1".into(),
            gender: SpeakerGender::Male,
            ..Default::default()
        }],
        ..Default::default()
    };
    d.lines = (0..6000u64)
        .map(|i| {
            line(
                &format!("l{i}"),
                "spk1",
                i * 1000,
                i * 1000 + 800,
                &format!("Line number {i} says hello to everyone"),
            )
        })
        .collect();
    d
}

fn timed<T>(label: &str, budget_ms: u128, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = f();
    let took = started.elapsed().as_millis();
    assert!(
        took < budget_ms,
        "{label} took {took} ms (budget {budget_ms})"
    );
    out
}

#[test]
fn six_thousand_lines_stay_within_the_budgets() {
    let root = tempfile::tempdir().unwrap();
    let dub = long();
    let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);
    let mut s = timed("open", 1500, || {
        EditSession::open(rt, dub, Box::new(|_| Ok(()))).unwrap()
    });

    let opts = SearchOptions {
        query: "hello".into(),
        field: Field::Translated,
        match_case: false,
        whole_word: true,
    };
    let matches = timed("search", 100, || find(&s.dub().lines, &opts));
    assert_eq!(matches.len(), 6000);

    timed("replace all", 1500, || {
        s.apply(EditOp::ReplaceMatches {
            matches,
            replacement: "hi".into(),
        })
        .unwrap()
    });
    assert_eq!(s.changed_ids().len(), 6000);
    timed("undo", 1500, || s.undo().unwrap());
    assert!(s.changed_ids().is_empty());

    let texts: Vec<(String, String)> = (0..6000)
        .map(|i| (format!("l{i}"), format!("New text {i}")))
        .collect();
    timed("bulk text", 1500, || {
        s.apply(EditOp::BulkText { texts }).unwrap()
    });
    timed("undo", 1500, || s.undo().unwrap());
    timed("redo", 1500, || s.redo().unwrap());
}

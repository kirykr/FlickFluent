//! Closing and reopening keeps every edit and every changed mark (SC-007). The marks are
//! derived from the saved hashes; the undo stacks end with the session.

mod support;

use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::session::EditSession;
use flickfluent_core::project::dub::read_from;
use flickfluent_core::project::Project;
use serde_json::json;
use support::editor_fixture::Live;
use support::run_fixture::config;

#[tokio::test]
async fn edits_and_changed_marks_survive_a_restart_and_the_undo_stack_does_not() {
    let live = Live::finished(json!({})).await;
    let ids: Vec<String> = live.lines().iter().take(4).map(|l| l.id.clone()).collect();
    for (n, id) in ids.iter().enumerate() {
        live.edit_text(id, &format!("Edited {n}"));
    }
    // Two are made again; two are left for later.
    flickfluent_core::editor::resynth::resynthesize(
        &live.shared,
        Some(ids[..2].to_vec()),
        flickfluent_core::pipeline::run::CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    let end = live.lines()[2].end_ms + 50;
    live.with(|s| {
        s.apply(EditOp::UpdateLine {
            id: ids[2].clone(),
            patch: LinePatch {
                end_ms: Some(end),
                ..Default::default()
            },
        })
        .unwrap()
    });
    let marks = live.with(|s| s.changed_ids());
    let lines = live.lines();
    assert_eq!(marks, ids[2..4].to_vec());
    assert!(live.with(|s| s.state().can_undo));

    // "Restart": read the project file with nothing else and open a new session over it.
    let dub = read_from(&Project::read(&live.path).unwrap().data);
    assert_eq!(dub.lines, lines, "the file holds every edit");
    let cfg = config(live.root.path(), "e.flick", json!({}));
    let reopened = EditSession::open(
        EditorRuntime::for_project(cfg, &dub),
        dub,
        Box::new(|_| Ok(())),
    )
    .unwrap();
    assert_eq!(reopened.changed_ids(), marks, "the marks come back");
    let state = reopened.state();
    assert!(
        !state.can_undo && !state.can_redo,
        "the undo stacks are empty"
    );
}

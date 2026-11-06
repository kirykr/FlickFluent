//! Edit rules: every op, its inverse, undo and redo. A rejected edit changes nothing, and after
//! every accepted edit the project file equals the session (SC-007).

mod support;

use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::error::{EDIT_END_BEFORE_START, EDIT_NOTHING_TO_UNDO, EDIT_OUT_OF_RANGE};
use flickfluent_core::project::dub::LineFlag;
use support::editor_fixture::{dub, Editing};

fn patch() -> LinePatch {
    LinePatch::default()
}

fn update(id: &str, p: LinePatch) -> EditOp {
    EditOp::UpdateLine {
        id: id.into(),
        patch: p,
    }
}

#[test]
fn a_text_edit_is_saved_marks_the_line_and_the_section() {
    let mut e = Editing::new(dub());
    let r = e
        .session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some("Hello there".into()),
                ..patch()
            },
        ))
        .unwrap();
    assert_eq!(r.changed_ids, vec!["l1"]);
    assert_eq!(r.lines.len(), 1);
    assert!(r.can_undo && !r.can_redo);
    let l = &e.session.dub().lines[0];
    assert_eq!(l.translated.as_deref(), Some("Hello there"));
    assert!(l.user_edited);
    assert!(e.session.dub().run.as_ref().unwrap().sections[0].edited);
    e.assert_saved();
}

#[test]
fn end_before_start_is_refused_and_changes_nothing() {
    let mut e = Editing::new(dub());
    let before = e.session.dub().clone();
    let err = e
        .session
        .apply(update(
            "l1",
            LinePatch {
                end_ms: Some(900),
                ..patch()
            },
        ))
        .unwrap_err();
    assert_eq!(err.code, EDIT_END_BEFORE_START);
    assert_eq!(e.session.dub(), &before);
    assert!(
        !e.session.state().can_undo,
        "a rejected op leaves the undo stack alone"
    );
    e.assert_saved();
}

#[test]
fn values_outside_the_safe_range_are_refused() {
    let mut e = Editing::new(dub());
    for p in [
        LinePatch {
            speed: Some(1.5),
            ..patch()
        },
        LinePatch {
            speed: Some(0.5),
            ..patch()
        },
        LinePatch {
            volume_db: Some(9.0),
            ..patch()
        },
        LinePatch {
            pitch_semitones: Some(-5.0),
            ..patch()
        },
    ] {
        let err = e.session.apply(update("l2", p)).unwrap_err();
        assert_eq!(err.code, EDIT_OUT_OF_RANGE);
    }
    assert!(!e.session.state().can_undo);
    // The edge values are fine.
    e.session
        .apply(update(
            "l2",
            LinePatch {
                speed: Some(1.3),
                ..patch()
            },
        ))
        .unwrap();
    assert_eq!(e.session.dub().lines[1].speed, 1.3);
}

#[test]
fn undo_then_redo_gives_the_same_lines_and_both_are_saved() {
    let mut e = Editing::new(dub());
    let original = e.session.dub().clone();
    e.session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some("Changed".into()),
                ..patch()
            },
        ))
        .unwrap();
    e.session
        .apply(EditOp::DeleteLines {
            ids: vec!["l2".into()],
        })
        .unwrap();
    let edited = e.session.dub().clone();
    assert_eq!(edited.lines.len(), 3);

    let u = e.session.undo().unwrap();
    assert!(u.can_undo && u.can_redo);
    assert_eq!(u.removed_ids, Vec::<String>::new());
    assert_eq!(
        u.lines.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        vec!["l2"]
    );
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, original.lines);
    assert_eq!(
        e.session.dub().run,
        original.run,
        "the edited mark is undone too"
    );
    e.assert_saved();

    e.session.redo().unwrap();
    e.session.redo().unwrap();
    assert_eq!(e.session.dub().lines, edited.lines);
    assert_eq!(e.session.dub().run, edited.run);
    e.assert_saved();
}

#[test]
fn undo_and_redo_with_nothing_to_do_say_so() {
    let mut e = Editing::new(dub());
    assert_eq!(e.session.undo().unwrap_err().code, EDIT_NOTHING_TO_UNDO);
    assert_eq!(e.session.redo().unwrap_err().code, EDIT_NOTHING_TO_UNDO);
}

#[test]
fn a_new_edit_clears_the_redo_stack() {
    let mut e = Editing::new(dub());
    e.session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some("A".into()),
                ..patch()
            },
        ))
        .unwrap();
    e.session.undo().unwrap();
    assert!(e.session.state().can_redo);
    e.session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some("B".into()),
                ..patch()
            },
        ))
        .unwrap();
    assert!(!e.session.state().can_redo);
}

#[test]
fn insert_puts_the_line_in_time_order_with_a_new_id_and_the_right_section() {
    let mut e = Editing::new(dub());
    let r = e
        .session
        .apply(EditOp::InsertLine {
            after_id: None,
            start_ms: 13_000,
            end_ms: 15_000,
            speaker_id: "spk1".into(),
            translated: "The end".into(),
        })
        .unwrap();
    let new = &r.lines[0];
    assert_eq!(e.session.dub().lines.last().unwrap().id, new.id);
    assert!(!["l1", "l2", "l3", "l4"].contains(&new.id.as_str()));
    assert_eq!(new.section_index, 0);
    assert_eq!(
        r.changed_ids,
        vec![new.id.clone()],
        "no clip yet, so it is changed"
    );
    e.assert_saved();

    // After a given line.
    let r = e
        .session
        .apply(EditOp::InsertLine {
            after_id: Some("l1".into()),
            start_ms: 3_200,
            end_ms: 3_800,
            speaker_id: "spk2".into(),
            translated: "Between".into(),
        })
        .unwrap();
    assert_eq!(e.session.dub().lines[1].id, r.lines[0].id);
    e.session.undo().unwrap();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines.len(), 4);
    e.assert_saved();
}

#[test]
fn insert_with_bad_times_or_an_unknown_speaker_is_refused() {
    let mut e = Editing::new(dub());
    let bad = |start, end, speaker: &str| EditOp::InsertLine {
        after_id: None,
        start_ms: start,
        end_ms: end,
        speaker_id: speaker.into(),
        translated: "x".into(),
    };
    assert_eq!(
        e.session.apply(bad(5_000, 5_000, "spk1")).unwrap_err().code,
        EDIT_END_BEFORE_START
    );
    assert_eq!(
        e.session
            .apply(bad(5_000, 6_000, "nobody"))
            .unwrap_err()
            .code,
        flickfluent_core::error::EDIT_NOT_FOUND
    );
    assert_eq!(e.session.dub().lines.len(), 4);
    assert!(!e.session.state().can_undo);
}

#[test]
fn delete_removes_lines_and_undo_puts_them_back_in_place() {
    let mut e = Editing::new(dub());
    let original = e.session.dub().lines.clone();
    let r = e
        .session
        .apply(EditOp::DeleteLines {
            ids: vec!["l2".into(), "l3".into()],
        })
        .unwrap();
    assert_eq!(r.removed_ids, vec!["l2", "l3"]);
    assert_eq!(e.session.dub().lines.len(), 2);
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, original);
}

#[test]
fn times_that_overlap_a_neighbour_set_the_flag_and_move_nothing() {
    let mut e = Editing::new(dub());
    e.session
        .apply(update(
            "l1",
            LinePatch {
                end_ms: Some(4_500),
                ..patch()
            },
        ))
        .unwrap();
    let lines = &e.session.dub().lines;
    assert!(
        lines[1].flags.contains(&LineFlag::Overlap) || lines[0].flags.contains(&LineFlag::Overlap)
    );
    assert_eq!((lines[0].start_ms, lines[1].start_ms), (1_000, 4_000));
    e.session.undo().unwrap();
    assert!(e
        .session
        .dub()
        .lines
        .iter()
        .all(|l| !l.flags.contains(&LineFlag::Overlap)));
}

#[test]
fn emptying_the_text_keeps_the_line_and_flags_it() {
    let mut e = Editing::new(dub());
    e.session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some(String::new()),
                ..patch()
            },
        ))
        .unwrap();
    let l = &e.session.dub().lines[0];
    assert!(l.flags.contains(&LineFlag::EmptyText));
    assert!(
        e.session.state().changed_ids.is_empty(),
        "no text, no speech to make"
    );
    e.session
        .apply(update(
            "l1",
            LinePatch {
                translated: Some("Back".into()),
                ..patch()
            },
        ))
        .unwrap();
    assert!(!e.session.dub().lines[0]
        .flags
        .contains(&LineFlag::EmptyText));
}

// --- part 2: merge, split, duplicate, bulk text --------------------------------------------

use flickfluent_core::error::EDIT_CANNOT_MERGE;

fn texts(e: &Editing) -> Vec<String> {
    e.session
        .dub()
        .lines
        .iter()
        .map(|l| l.translated.clone().unwrap_or_default())
        .collect()
}

#[test]
fn split_divides_the_text_and_the_time_in_proportion_and_both_halves_are_changed() {
    let mut e = Editing::new(dub());
    let before = e.session.dub().lines.clone();
    // "Good morning" (12 characters) split after "Good " (5): 5/12 of 2,000 ms.
    let r = e
        .session
        .apply(EditOp::SplitLine {
            id: "l2".into(),
            text_offset: 5,
        })
        .unwrap();
    let lines = &e.session.dub().lines;
    assert_eq!(lines.len(), 5);
    let (a, b) = (&lines[1], &lines[2]);
    assert_eq!(
        (a.id.as_str(), a.translated.as_deref()),
        ("l2", Some("Good"))
    );
    assert_eq!(b.translated.as_deref(), Some("morning"));
    assert_eq!(a.start_ms, 4_000);
    assert_eq!(a.end_ms, 4_000 + 2_000 * 5 / 12);
    assert_eq!((b.start_ms, b.end_ms), (a.end_ms, 6_000));
    assert_eq!(r.changed_ids.len(), 2, "both halves need speech");
    assert!(b.audio.is_none());
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, before);
    assert!(!e.session.state().can_undo, "one step");
    e.session.redo().unwrap();
    assert_eq!(e.session.dub().lines.len(), 5);
}

#[test]
fn a_split_at_either_end_or_past_it_is_refused() {
    let mut e = Editing::new(dub());
    for offset in [0, 12, 40] {
        let err = e
            .session
            .apply(EditOp::SplitLine {
                id: "l2".into(),
                text_offset: offset,
            })
            .unwrap_err();
        assert_eq!(err.code, EDIT_OUT_OF_RANGE, "{offset}");
    }
    assert_eq!(e.session.dub().lines.len(), 4);
}

#[test]
fn merge_joins_same_speaker_neighbours_and_refuses_the_rest() {
    let mut e = Editing::with(dub(), |d| d.lines[1].speaker_id = "spk1".into());
    let before = e.session.dub().lines.clone();
    let r = e
        .session
        .apply(EditOp::MergeLines {
            ids: vec!["l1".into(), "l2".into()],
        })
        .unwrap();
    let lines = &e.session.dub().lines;
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].id, "l1");
    assert_eq!(lines[0].translated.as_deref(), Some("Hello Good morning"));
    assert_eq!((lines[0].start_ms, lines[0].end_ms), (1_000, 6_000));
    assert_eq!(r.removed_ids, vec!["l2"]);
    assert!(r.changed_ids.contains(&"l1".to_string()));
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, before);

    // Not neighbours (l1 and l3), different speakers (l3 and l4), fewer than two.
    for ids in [vec!["l1", "l3"], vec!["l3", "l4"], vec!["l1"]] {
        let err = e
            .session
            .apply(EditOp::MergeLines {
                ids: ids.iter().map(|s| s.to_string()).collect(),
            })
            .unwrap_err();
        assert_eq!(err.code, EDIT_CANNOT_MERGE, "{ids:?}");
    }
}

#[test]
fn duplicate_puts_a_copy_after_the_line_without_overlapping_it() {
    let mut e = Editing::new(dub());
    let r = e
        .session
        .apply(EditOp::DuplicateLine { id: "l3".into() })
        .unwrap();
    let lines = &e.session.dub().lines;
    assert_eq!(lines.len(), 5);
    let copy = &lines[3];
    assert_ne!(copy.id, "l3");
    assert_eq!(copy.translated, lines[2].translated);
    assert!(copy.start_ms >= lines[2].end_ms);
    assert_eq!(
        copy.end_ms - copy.start_ms,
        lines[2].end_ms - lines[2].start_ms
    );
    assert!(copy.audio.is_none());
    assert_eq!(r.changed_ids, vec![copy.id.clone()]);
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines.len(), 4);
}

#[test]
fn bulk_text_changes_only_lines_whose_text_differs_in_one_step() {
    let mut e = Editing::new(dub());
    let r = e
        .session
        .apply(EditOp::BulkText {
            texts: vec![
                ("l1".into(), "Hello".into()),        // same
                ("l2".into(), "Good evening".into()), // new
                ("l4".into(), "Farewell".into()),     // new
            ],
        })
        .unwrap();
    assert_eq!(
        texts(&e),
        vec!["Hello", "Good evening", "How are you", "Farewell"]
    );
    assert_eq!(r.lines.len(), 2);
    assert_eq!(r.changed_ids, vec!["l2", "l4"]);
    e.session.undo().unwrap();
    assert_eq!(
        texts(&e),
        vec!["Hello", "Good morning", "How are you", "Goodbye"]
    );
    assert!(!e.session.state().can_undo);

    // Nothing differs: nothing is saved and nothing is undoable.
    let r = e
        .session
        .apply(EditOp::BulkText {
            texts: vec![("l1".into(), "Hello".into())],
        })
        .unwrap();
    assert!(r.lines.is_empty() && !r.can_undo);
    // An unknown id refuses the whole action.
    let err = e
        .session
        .apply(EditOp::BulkText {
            texts: vec![("l1".into(), "X".into()), ("ghost".into(), "Y".into())],
        })
        .unwrap_err();
    assert_eq!(err.code, flickfluent_core::error::EDIT_NOT_FOUND);
    assert_eq!(texts(&e)[0], "Hello");
}

#[test]
fn a_failed_save_leaves_the_session_exactly_as_it_was() {
    use flickfluent_core::editor::runtime::EditorRuntime;
    use flickfluent_core::editor::session::EditSession;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let fail = Arc::new(AtomicBool::new(false));
    let flag = fail.clone();
    let rt = EditorRuntime::for_project(
        support::run_fixture::config(root.path(), "a.flick", serde_json::json!({})),
        &dub(),
    );
    let mut s = EditSession::open(
        rt,
        dub(),
        Box::new(move |_| {
            if flag.load(Ordering::SeqCst) {
                Err(flickfluent_core::error::AppError::new(
                    "IO_ERROR", "error.io",
                ))
            } else {
                Ok(())
            }
        }),
    )
    .unwrap();
    let before = s.dub().clone();
    fail.store(true, Ordering::SeqCst);
    assert!(s
        .apply(EditOp::BulkText {
            texts: vec![("l1".into(), "Changed".into())]
        })
        .is_err());
    assert_eq!(s.dub(), &before);
    assert!(!s.state().can_undo);
}

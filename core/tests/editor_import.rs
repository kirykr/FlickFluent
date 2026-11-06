//! Subtitle import: a preview first, three choices, nothing changes before the user confirms,
//! and each apply is one undo step (spec US3, FR-018).

mod support;

use std::time::Duration;

use flickfluent_core::editor::ops::EditOp;
use flickfluent_core::error::SUBTITLE_PREVIEW_EXPIRED;
use flickfluent_core::subtitles::import::ImportChoice;
use flickfluent_core::subtitles::read::{SubtitleCue, SubtitleFile, SubtitleFormat};
use flickfluent_core::subtitles::write::{export_file, Format, Source};
use support::editor_fixture::{dub, Editing};

fn cue(start: u64, end: u64, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_ms: start,
        end_ms: end,
        text: text.into(),
    }
}

fn file(cues: Vec<SubtitleCue>) -> SubtitleFile {
    SubtitleFile {
        format: SubtitleFormat::Srt,
        encoding: "UTF-8".into(),
        cues,
    }
}

/// Four cues at the same times as the fixture's four lines.
fn same_times() -> SubtitleFile {
    file(vec![
        cue(1_000, 3_000, "Imported one"),
        cue(4_000, 6_000, "Imported two"),
        cue(7_000, 9_000, "Imported three"),
        cue(10_000, 12_000, "Imported four"),
    ])
}

fn texts(e: &Editing) -> Vec<String> {
    e.session
        .dub()
        .lines
        .iter()
        .map(|l| l.translated.clone().unwrap_or_default())
        .collect()
}

#[test]
fn the_preview_counts_and_compares_and_changes_nothing() {
    let mut e = Editing::new(dub());
    let before = e.session.dub().clone();
    let p = e.session.import_preview(same_times());
    assert_eq!((p.cue_count, p.line_count), (4, 4));
    assert!(p.all_match && p.differences.is_empty());
    assert_eq!(p.encoding, "UTF-8");
    assert_eq!(
        p.choices,
        vec![
            ImportChoice::ReplaceText,
            ImportChoice::ReplaceLines,
            ImportChoice::FillByOverlap
        ]
    );
    assert_eq!(e.session.dub(), &before);
    assert!(!e.session.state().can_undo);

    // Times off by more than a millisecond, and a different number of cues.
    let many = file(
        (0..30)
            .map(|i| cue(i * 500 + 7, i * 500 + 400, "x"))
            .collect(),
    );
    let p = e.session.import_preview(many);
    assert!(!p.all_match);
    assert_eq!(p.differences.len(), 20, "only the first twenty are listed");
    assert!(!p.choices.contains(&ImportChoice::ReplaceText));
    assert!(p.choices.contains(&ImportChoice::ReplaceLines));
}

#[test]
fn replace_text_changes_the_words_and_keeps_the_times() {
    let mut e = Editing::new(dub());
    let before = e.session.dub().lines.clone();
    let p = e.session.import_preview(same_times());
    let r = e
        .session
        .import_apply(&p.preview_id, ImportChoice::ReplaceText)
        .unwrap();
    assert_eq!(
        texts(&e),
        vec![
            "Imported one",
            "Imported two",
            "Imported three",
            "Imported four"
        ]
    );
    for (a, b) in e.session.dub().lines.iter().zip(&before) {
        assert_eq!((a.start_ms, a.end_ms, &a.id), (b.start_ms, b.end_ms, &b.id));
    }
    assert_eq!(r.changed_ids.len(), 4);
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, before);
    assert!(!e.session.state().can_undo, "one step");
}

#[test]
fn replace_text_needs_the_same_number_of_cues() {
    let mut e = Editing::new(dub());
    let p = e
        .session
        .import_preview(file(vec![cue(0, 500, "only one")]));
    assert!(e
        .session
        .import_apply(&p.preview_id, ImportChoice::ReplaceText)
        .is_err());
    assert!(!e.session.state().can_undo);
}

#[test]
fn replace_lines_builds_lines_from_the_cues_with_speakers_by_overlap() {
    let mut e = Editing::new(dub());
    let before = e.session.dub().lines.clone();
    let p = e.session.import_preview(file(vec![
        cue(4_200, 5_800, "Inside the second"), // overlaps l2 (spk2)
        cue(7_100, 8_900, "Inside the third"),  // overlaps l3 (spk1)
        cue(40_000, 41_000, "Far away"),        // overlaps nothing
    ]));
    e.session
        .import_apply(&p.preview_id, ImportChoice::ReplaceLines)
        .unwrap();
    let lines = &e.session.dub().lines;
    assert_eq!(lines.len(), 3);
    assert_eq!(
        texts(&e),
        vec!["Inside the second", "Inside the third", "Far away"]
    );
    assert_eq!(lines[0].speaker_id, "spk2");
    assert_eq!(lines[1].speaker_id, "spk1");
    assert_eq!(
        lines[2].speaker_id, "spk1",
        "the first speaker when nothing overlaps"
    );
    assert!(lines.iter().all(|l| l.audio.is_none() && l.user_edited));
    assert_eq!(
        e.session.state().changed_ids.len(),
        3,
        "new lines need speech"
    );
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(e.session.dub().lines, before);
}

#[test]
fn fill_by_overlap_takes_the_cue_that_overlaps_most_and_leaves_the_rest() {
    let mut e = Editing::new(dub());
    let p = e.session.import_preview(file(vec![
        cue(500, 1_200, "Brief"),         // l1 overlaps 200 ms
        cue(1_200, 3_000, "Most of one"), // l1 overlaps 1800 ms
        cue(4_000, 6_000, "Two exactly"), // l2
                                          // nothing near l3 or l4
    ]));
    e.session
        .import_apply(&p.preview_id, ImportChoice::FillByOverlap)
        .unwrap();
    assert_eq!(
        texts(&e),
        vec!["Most of one", "Two exactly", "How are you", "Goodbye"]
    );
    e.assert_saved();
    e.session.undo().unwrap();
    assert_eq!(texts(&e)[0], "Hello");
    assert!(!e.session.state().can_undo);
}

#[test]
fn an_expired_or_unknown_preview_cannot_be_applied() {
    let mut e = Editing::new(dub());
    let err = e
        .session
        .import_apply("nope", ImportChoice::FillByOverlap)
        .unwrap_err();
    assert_eq!(err.code, SUBTITLE_PREVIEW_EXPIRED);

    e.session.set_preview_ttl(Duration::from_millis(1));
    let p = e.session.import_preview(same_times());
    std::thread::sleep(Duration::from_millis(20));
    let err = e
        .session
        .import_apply(&p.preview_id, ImportChoice::ReplaceText)
        .unwrap_err();
    assert_eq!(err.code, SUBTITLE_PREVIEW_EXPIRED);
    assert_eq!(texts(&e)[0], "Hello", "nothing changed");
}

#[test]
fn a_project_that_only_makes_subtitles_can_import_edit_and_export() {
    let mut e = Editing::with(dub(), |d| {
        d.options.fallbacks.subtitles_only = true;
        for l in &mut d.lines {
            l.audio = None;
            l.audio_ms = None;
        }
    });
    assert!(
        e.session.state().changed_ids.is_empty(),
        "no speech is wanted"
    );
    let p = e.session.import_preview(same_times());
    e.session
        .import_apply(&p.preview_id, ImportChoice::ReplaceText)
        .unwrap();
    e.session
        .apply(EditOp::DeleteLines {
            ids: vec!["l4".into()],
        })
        .unwrap();
    assert!(e.session.state().changed_ids.is_empty());

    let path = e.work().join("out.srt");
    let n = export_file(
        &e.session.dub().lines,
        Source::Translated,
        Format::Srt,
        &path,
        None,
    )
    .unwrap();
    assert_eq!(n, 3);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("1\n00:00:01,000 --> 00:00:03,000\nImported one\n"));
    assert!(!e.work().join("out.srt.part").exists());
}

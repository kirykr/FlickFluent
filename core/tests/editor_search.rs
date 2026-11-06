//! Search and replace over the lines (research R10): plain text, case and whole-word options,
//! a chosen subset replaced as one undo step, 6,000 lines scanned fast.

mod support;

use std::time::Instant;

use flickfluent_core::editor::ops::EditOp;
use flickfluent_core::editor::search::{find, Field, Match, MatchField, SearchOptions};
use flickfluent_core::project::dub::TranscriptLine;
use support::editor_fixture::{dub, line, Editing};

fn opts(query: &str, field: Field) -> SearchOptions {
    SearchOptions {
        query: query.into(),
        field,
        match_case: false,
        whole_word: false,
    }
}

fn lines() -> Vec<TranscriptLine> {
    let mut l = vec![
        line("a", "spk1", 0, 1000, "Hello world, hello again"),
        line("b", "spk1", 1000, 2000, "Say HELLO to the world"),
        line("c", "spk1", 2000, 3000, "Shell, not the word"),
        line("d", "spk1", 3000, 4000, "café au lait"),
    ];
    l[0].text = "Bonjour le monde".into();
    l[1].text = "Dis bonjour".into();
    l
}

#[test]
fn it_finds_in_the_translation_the_original_or_both() {
    let l = lines();
    let t = find(&l, &opts("hello", Field::Translated));
    assert_eq!(
        t.iter()
            .map(|m| (m.line_id.as_str(), m.start, m.end))
            .collect::<Vec<_>>(),
        vec![("a", 0, 5), ("a", 13, 18), ("b", 4, 9)]
    );
    assert!(t.iter().all(|m| m.field == MatchField::Translated));

    let o = find(&l, &opts("bonjour", Field::Original));
    assert_eq!(o.len(), 2);
    assert!(o.iter().all(|m| m.field == MatchField::Original));

    let both = find(&l, &opts("o", Field::Both));
    assert!(both.iter().any(|m| m.field == MatchField::Original));
    assert!(both.iter().any(|m| m.field == MatchField::Translated));
}

#[test]
fn match_case_and_whole_word_narrow_the_result() {
    let l = lines();
    let mut o = opts("HELLO", Field::Translated);
    o.match_case = true;
    assert_eq!(find(&l, &o).len(), 1);

    let mut w = opts("hell", Field::Translated);
    assert_eq!(find(&l, &w).len(), 4, "hello x3 and shell");
    w.whole_word = true;
    assert_eq!(find(&l, &w).len(), 0);
    let mut w = opts("the", Field::Translated);
    w.whole_word = true;
    assert_eq!(find(&l, &w).len(), 2);
}

#[test]
fn offsets_count_utf16_units_like_the_window_does() {
    let l = lines();
    let m = find(&l, &opts("au", Field::Translated));
    assert_eq!(m.len(), 1);
    assert_eq!(
        (m[0].start, m[0].end),
        (5, 7),
        "é is one unit, so au starts at 5"
    );
    // An emoji is two UTF-16 units.
    let mut e = vec![line("e", "spk1", 0, 1000, "go 😀 now")];
    e[0].translated = Some("go 😀 now".into());
    let m = find(&e, &opts("now", Field::Translated));
    assert_eq!((m[0].start, m[0].end), (6, 9));
}

#[test]
fn an_empty_query_finds_nothing() {
    assert!(find(&lines(), &opts("", Field::Both)).is_empty());
}

fn session_with_lines() -> Editing {
    Editing::with(dub(), |d| {
        d.lines[0].translated = Some("Hello there".into());
        d.lines[1].translated = Some("Hello again".into());
        d.lines[2].translated = Some("Say hello".into());
    })
}

#[test]
fn replace_changes_only_the_chosen_matches_and_is_one_undo_step() {
    let mut e = session_with_lines();
    let all = find(&e.session.dub().lines, &opts("hello", Field::Translated));
    assert_eq!(all.len(), 3);
    let chosen: Vec<Match> = all.into_iter().filter(|m| m.line_id != "l2").collect();
    let r = e
        .session
        .apply(EditOp::ReplaceMatches {
            matches: chosen,
            replacement: "Hi".into(),
        })
        .unwrap();
    let texts: Vec<_> = e
        .session
        .dub()
        .lines
        .iter()
        .map(|l| l.translated.clone().unwrap())
        .collect();
    assert_eq!(texts, vec!["Hi there", "Hello again", "Say Hi", "Goodbye"]);
    assert_eq!(r.lines.len(), 2);
    assert_eq!(r.changed_ids, vec!["l1", "l3"]);
    e.assert_saved();

    e.session.undo().unwrap();
    assert_eq!(
        e.session.dub().lines[0].translated.as_deref(),
        Some("Hello there")
    );
    assert_eq!(
        e.session.dub().lines[2].translated.as_deref(),
        Some("Say hello")
    );
    assert!(!e.session.state().can_undo, "one step undid both lines");
}

#[test]
fn two_matches_in_one_line_are_both_replaced() {
    let mut e = Editing::with(dub(), |d| {
        d.lines[0].translated = Some("la la land".into());
    });
    let m = find(&e.session.dub().lines, &opts("la", Field::Translated));
    assert_eq!(m.len(), 3, "la, la, and the one in land");
    e.session
        .apply(EditOp::ReplaceMatches {
            matches: m.into_iter().take(2).collect(),
            replacement: "tra".into(),
        })
        .unwrap();
    assert_eq!(
        e.session.dub().lines[0].translated.as_deref(),
        Some("tra tra land")
    );
}

#[test]
fn matches_in_the_original_text_are_never_replaced() {
    let mut e = Editing::new(dub());
    let m = find(&e.session.dub().lines, &opts("orig", Field::Original));
    assert!(!m.is_empty());
    let r = e
        .session
        .apply(EditOp::ReplaceMatches {
            matches: m,
            replacement: "x".into(),
        })
        .unwrap();
    assert!(r.lines.is_empty());
    assert!(!e.session.state().can_undo);
}

#[test]
fn six_thousand_lines_are_scanned_in_under_100_ms() {
    let l: Vec<TranscriptLine> = (0..6000)
        .map(|i| {
            let mut x = line(
                &format!("l{i}"),
                "spk1",
                i * 1000,
                i * 1000 + 800,
                &format!("This is line number {i} of the film, said hello to the world"),
            );
            x.text = format!("Ceci est la ligne {i} du film");
            x
        })
        .collect();
    let started = Instant::now();
    let m = find(&l, &opts("hello", Field::Both));
    let took = started.elapsed();
    assert_eq!(m.len(), 6000);
    assert!(took.as_millis() < 100, "took {took:?}");
}

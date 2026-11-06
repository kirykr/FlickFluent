//! SRT and WebVTT in and out (`contracts/subtitle-formats.md`): the fixtures parse as the
//! contract says, the writers match it byte for byte, and a round trip loses nothing.

use std::path::{Path, PathBuf};
use std::time::Instant;

use flickfluent_core::error::SUBTITLE_PARSE_FAILED;
use flickfluent_core::project::dub::TranscriptLine;
use flickfluent_core::subtitles::read::{parse_bytes, SubtitleFormat};
use flickfluent_core::subtitles::templates::bundled;
use flickfluent_core::subtitles::write::{srt_text, vtt_text, Source};

fn fixture(name: &str) -> Vec<u8> {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/subtitles")
        .join(name);
    std::fs::read(p).unwrap()
}

fn cues(name: &str) -> Vec<(u64, u64, String)> {
    parse_bytes(&fixture(name))
        .unwrap()
        .cues
        .into_iter()
        .map(|c| (c.start_ms, c.end_ms, c.text))
        .collect()
}

#[test]
fn srt_with_tags_and_two_row_cues() {
    let file = parse_bytes(&fixture("plain.srt")).unwrap();
    assert_eq!(file.format, SubtitleFormat::Srt);
    assert_eq!(file.encoding, "UTF-8");
    assert_eq!(
        cues("plain.srt"),
        vec![
            (1000, 3500, "Hello there.".to_string()),
            (4000, 6250, "Second line across two rows.".to_string()),
            (7000, 8000, "Tagged text".to_string()),
        ]
    );
}

#[test]
fn a_bom_and_windows_line_ends_are_handled() {
    let c = cues("crlf-bom.srt");
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].2, "With BOM and CRLF.");
    assert!(!c[0].2.contains('\r'));
}

#[test]
fn a_dot_before_the_milliseconds_is_read() {
    assert_eq!(
        cues("dot-millis.srt"),
        vec![
            (1250, 2500, "Dot milliseconds.".to_string()),
            (3000, 4125, "Still fine.".to_string()),
        ]
    );
}

#[test]
fn webvtt_skips_style_ids_and_settings_and_reads_entities() {
    let file = parse_bytes(&fixture("styled.vtt")).unwrap();
    assert_eq!(file.format, SubtitleFormat::Vtt);
    assert_eq!(
        cues("styled.vtt"),
        vec![
            (1000, 3000, "Styled & positioned".to_string()),
            (3500, 5000, "Plain cue".to_string()),
        ]
    );
}

#[test]
fn webvtt_notes_and_regions_are_not_cues() {
    assert_eq!(
        cues("notes-and-regions.vtt"),
        vec![
            (1000, 2000, "First after notes".to_string()),
            (2500, 4000, "Second one".to_string()),
        ]
    );
}

#[test]
fn a_latin1_file_is_detected_named_and_decoded() {
    let file = parse_bytes(&fixture("latin1.srt")).unwrap();
    assert_ne!(file.encoding, "UTF-8");
    assert!(
        file.cues[0].text.starts_with("Café crème"),
        "{}",
        file.cues[0].text
    );
    assert!(file.cues[1].text.contains("über"));
}

#[test]
fn cues_out_of_order_are_sorted_by_start() {
    let c = cues("out-of-order.srt");
    assert_eq!(c[0].2, "Earlier cue.");
    assert_eq!(c[1].2, "Later cue.");
}

#[test]
fn bad_files_are_refused_and_the_first_problem_is_named() {
    let e = parse_bytes(&fixture("bad-time.srt")).unwrap_err();
    assert_eq!(e.code, SUBTITLE_PARSE_FAILED);
    let d = e.details.unwrap();
    assert_eq!(d["cue"], 2);
    assert!(d["detail"].as_str().unwrap().contains("cue 2"));

    for name in ["empty.srt", "not-a-subtitle.txt"] {
        let e = parse_bytes(&fixture(name)).unwrap_err();
        assert_eq!(e.code, SUBTITLE_PARSE_FAILED, "{name}");
    }
}

#[test]
fn six_thousand_cues_parse_in_under_a_second() {
    let mut text = String::new();
    for i in 0..6000u64 {
        let (s, e) = (i * 1000, i * 1000 + 800);
        text.push_str(&format!(
            "{}\n00:{:02}:{:02},{:03} --> 00:{:02}:{:02},{:03}\nCue number {}\n\n",
            i + 1,
            s / 60000 % 60,
            s / 1000 % 60,
            s % 1000,
            e / 60000 % 60,
            e / 1000 % 60,
            e % 1000,
            i
        ));
    }
    let started = Instant::now();
    let file = parse_bytes(text.as_bytes()).unwrap();
    assert_eq!(file.cues.len(), 6000);
    assert!(
        started.elapsed().as_millis() < 1000,
        "{:?}",
        started.elapsed()
    );
}

// --- writers -------------------------------------------------------------------------------

fn line(
    id: &str,
    start: u64,
    end: u64,
    original: &str,
    translated: Option<&str>,
) -> TranscriptLine {
    TranscriptLine {
        id: id.into(),
        start_ms: start,
        end_ms: end,
        text: original.into(),
        translated: translated.map(String::from),
        dub_start_ms: Some(start.saturating_sub(300)),
        ..Default::default()
    }
}

fn transcript() -> Vec<TranscriptLine> {
    vec![
        line("a", 500, 2_500, "Bonjour.", Some("Hello <there> & you.")),
        line("b", 3_725_005, 3_726_000, "Merci.", Some("Thanks.")),
        line("c", 4_000_000, 4_001_000, "", Some("")),
        line("d", 4_002_000, 4_003_000, "Salut.", Some("Hi.")),
    ]
}

#[test]
fn srt_matches_the_contract_byte_for_byte() {
    let srt = srt_text(&transcript(), Source::Translated);
    assert_eq!(
        srt,
        "1\n00:00:00,500 --> 00:00:02,500\nHello <there> & you.\n\n2\n01:02:05,005 --> 01:02:06,000\nThanks.\n\n3\n01:06:42,000 --> 01:06:43,000\nHi.\n\n"
    );
    assert!(!srt.starts_with('\u{feff}'), "no BOM");
}

#[test]
fn both_gives_the_original_a_line_break_and_the_translation() {
    let srt = srt_text(&transcript()[..2], Source::Both);
    assert_eq!(
        srt,
        "1\n00:00:00,500 --> 00:00:02,500\nBonjour.\nHello <there> & you.\n\n2\n01:02:05,005 --> 01:02:06,000\nMerci.\nThanks.\n\n"
    );
    let orig = srt_text(&transcript()[..1], Source::Original);
    assert!(orig.contains("\nBonjour.\n") && !orig.contains("Hello"));
}

#[test]
fn webvtt_carries_the_template_and_escapes_text_and_uses_the_transcript_times() {
    let classic = bundled()
        .unwrap()
        .into_iter()
        .find(|t| t.id == "classic")
        .unwrap();
    let vtt = vtt_text(&transcript()[..2], Source::Translated, &classic);
    let expect_head = "WEBVTT\n\nSTYLE\n::cue {\n  font-family: Inter, \"Noto Sans Khmer\", \"Khmer UI\", \"Khmer Sangam MN\", sans-serif;\n";
    assert!(vtt.starts_with(expect_head), "{vtt}");
    assert!(vtt.contains("  color: #ffffff;\n"));
    assert!(vtt.contains("font-size: 5vh;"));
    assert!(vtt.contains("\n}\n\n00:00:00.500 --> 00:00:02.500 line:94% position:50% align:center\nHello &lt;there&gt; &amp; you.\n\n"));
    assert!(
        vtt.contains("01:02:05.005 --> 01:02:06.000"),
        "transcript times, not dub starts"
    );

    let top = bundled()
        .unwrap()
        .into_iter()
        .find(|t| t.id == "top-note")
        .unwrap();
    assert!(vtt_text(&transcript()[..1], Source::Translated, &top).contains("line:6% position:50%"));
}

#[test]
fn export_then_import_gives_equal_text_and_times_to_the_millisecond() {
    // Angle brackets in SRT text read back as tags, so the round trip uses text without them.
    let mut lines = transcript();
    lines[0].translated = Some("Hello & you.".into());
    for text in [
        srt_text(&lines, Source::Translated),
        vtt_text(&lines, Source::Translated, &bundled().unwrap()[0]),
    ] {
        let file = parse_bytes(text.as_bytes()).unwrap();
        let kept: Vec<&TranscriptLine> = lines.iter().filter(|l| l.id != "c").collect();
        assert_eq!(file.cues.len(), kept.len());
        for (cue, line) in file.cues.iter().zip(kept) {
            assert_eq!((cue.start_ms, cue.end_ms), (line.start_ms, line.end_ms));
            assert_eq!(cue.text, line.translated.clone().unwrap());
        }
    }
}

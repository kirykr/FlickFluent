//! The translated lines as an `.srt` file, for a run that does not speak.

use flickfluent_core::error::{FILE_EXISTS, INVALID_PROJECT};
use flickfluent_core::pipeline::stages::{export_subtitles, subtitles_text};
use flickfluent_core::project::dub::TranscriptLine;

fn line(id: &str, start: u64, end: u64, text: &str, translated: Option<&str>) -> TranscriptLine {
    TranscriptLine {
        id: id.into(),
        start_ms: start,
        end_ms: end,
        text: text.into(),
        translated: translated.map(String::from),
        ..Default::default()
    }
}

#[test]
fn srt_uses_the_translation_and_falls_back_to_the_original() {
    let lines = [
        line("a", 500, 2_500, "Bonjour.", Some("Hello.")),
        line("b", 3_725_005, 3_726_000, "Merci.", None),
        line("c", 4_000, 5_000, "Salut.", Some("  ")),
    ];
    let srt = subtitles_text(&lines);
    assert_eq!(
        srt,
        "1\n00:00:00,500 --> 00:00:02,500\nHello.\n\n2\n01:02:05,005 --> 01:02:06,000\nMerci.\n\n3\n00:00:04,000 --> 00:00:05,000\nSalut.\n\n"
    );
}

#[test]
fn export_writes_the_file_and_guards_names_and_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let lines = [line("a", 0, 1_000, "Hi", Some("Hello"))];
    let out = export_subtitles(&lines, dir.path(), "my movie", false).unwrap();
    assert_eq!(out.path.file_name().unwrap(), "my movie.srt");
    assert!(std::fs::read_to_string(&out.path)
        .unwrap()
        .contains("Hello"));

    assert_eq!(
        export_subtitles(&lines, dir.path(), "my movie", false)
            .unwrap_err()
            .code,
        FILE_EXISTS
    );
    export_subtitles(&lines, dir.path(), "my movie", true).unwrap();
    for bad in ["../x", "a/b", "", " "] {
        assert_eq!(
            export_subtitles(&lines, dir.path(), bad, false)
                .unwrap_err()
                .code,
            INVALID_PROJECT,
            "{bad:?}"
        );
    }
}

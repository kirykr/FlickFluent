//! Project format 2: dubbing data round-trips, format 1 files migrate with a backup.

use std::io::Write;
use std::path::Path;

use flickfluent_core::error::FORMAT_TOO_NEW;
use flickfluent_core::project::dub::{DubData, LineFlag, Source, TranscriptLine};
use flickfluent_core::project::{Project, ProjectSession};

fn write_zip(path: &Path, version: u32, data: &str) {
    let manifest = format!(
        r#"{{"formatVersion":{version},"appVersion":"0.1.0","id":"3f1b8f0e-0000-4000-8000-000000000001","name":"Old","createdAt":"2026-11-04T10:00:00Z","savedAt":"2026-11-04T10:00:00Z"}}"#
    );
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", opts).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.start_file("project.json", opts).unwrap();
    zip.write_all(data.as_bytes()).unwrap();
    zip.finish().unwrap();
}

fn sample() -> DubData {
    let mut line = TranscriptLine {
        id: "l1".into(),
        section_index: 0,
        start_ms: 1000,
        end_ms: 2500,
        speaker_id: "spk1".into(),
        language: "fr".into(),
        text: "Bonjour".into(),
        translated: Some("Hello".into()),
        ..Default::default()
    };
    line.flags.insert(LineFlag::TooLong);
    DubData {
        source: Some(Source {
            path: "/v/a.mp4".into(),
            duration_ms: 20_000,
            has_audio: true,
            has_video: true,
            size_bytes: 10,
            fingerprint: "abc".into(),
        }),
        source_language: "auto".into(),
        target_language: "en".into(),
        lines: vec![line],
        ..Default::default()
    }
}

#[test]
fn format_1_file_migrates_with_backup_and_empty_v2_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.flick");
    write_zip(&path, 1, r#"{"notes":"keep me"}"#);
    let before = std::fs::read(&path).unwrap();

    let opened = ProjectSession::open(&path).unwrap().session;
    assert_eq!(opened.project.manifest.format_version, 2);
    assert_eq!(opened.project.data["notes"], "keep me");
    assert!(opened.project.data["lines"].as_array().unwrap().is_empty());
    assert_eq!(
        std::fs::read(dir.path().join("old.flick.v1.bak")).unwrap(),
        before
    );
}

#[test]
fn dub_data_round_trips_through_save_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.flick");
    let mut s = ProjectSession::create("P", &path).unwrap();
    s.set_data(serde_json::to_value(sample()).unwrap());
    s.save().unwrap();
    s.close();

    let back = Project::read(&path).unwrap();
    assert_eq!(back.manifest.format_version, 2);
    let dub: DubData = serde_json::from_value(back.data).unwrap();
    assert_eq!(dub, sample());
}

#[test]
fn line_must_end_after_it_starts() {
    let mut d = sample();
    assert!(d.validate().is_ok());
    d.lines[0].end_ms = d.lines[0].start_ms;
    assert!(d.validate().is_err());
}

#[test]
fn format_3_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.flick");
    write_zip(&path, 3, "{}");
    let err = ProjectSession::open(&path).err().expect("must refuse");
    assert_eq!(err.code, FORMAT_TOO_NEW);
}

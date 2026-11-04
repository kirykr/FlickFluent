use std::io::{Read, Write};
use std::path::Path;

use flickfluent_core::error::{DAMAGED, FORMAT_TOO_NEW};
use flickfluent_core::project::ProjectSession;
use zip::write::SimpleFileOptions;

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn manifest(version: u32, extra: &str) -> String {
    format!(
        r#"{{"formatVersion":{version},"appVersion":"9.9.9","id":"3f1b8f0e-0000-4000-8000-000000000000","name":"Future","createdAt":"2026-11-04T10:00:00Z","savedAt":"2026-11-04T10:00:00Z"{extra}}}"#
    )
}

#[test]
fn newer_format_is_refused_and_the_file_is_not_touched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.flick");
    write_zip(
        &path,
        &[
            ("manifest.json", manifest(99, "").as_bytes()),
            ("project.json", b"{}"),
        ],
    );
    let before = std::fs::read(&path).unwrap();

    let err = ProjectSession::open(&path).err().expect("must refuse");
    assert_eq!(err.code, FORMAT_TOO_NEW);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // No sidecar files appear either.
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1);
}

#[test]
fn unknown_keys_and_assets_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keep.flick");
    write_zip(
        &path,
        &[
            (
                "manifest.json",
                manifest(1, r#","futureKey":{"a":1}"#).as_bytes(),
            ),
            ("project.json", b"{}"),
            ("assets/clip.bin", &[1, 2, 3, 4]),
        ],
    );

    let mut s = ProjectSession::open(&path).unwrap().session;
    s.set_data(serde_json::json!({ "x": 1 }));
    s.save().unwrap();
    s.close();

    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut asset = Vec::new();
    zip.by_name("assets/clip.bin")
        .unwrap()
        .read_to_end(&mut asset)
        .unwrap();
    assert_eq!(asset, vec![1, 2, 3, 4]);
    let mut m = String::new();
    zip.by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut m)
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&m).unwrap();
    assert_eq!(v["futureKey"], serde_json::json!({ "a": 1 }));
}

#[test]
fn a_file_that_is_not_a_project_is_damaged_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("junk.flick");
    std::fs::write(&path, b"this is not a zip").unwrap();
    let err = ProjectSession::open(&path).err().unwrap();
    assert_eq!(err.code, DAMAGED);
}

#[test]
fn a_zip_without_a_manifest_is_damaged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nomanifest.flick");
    write_zip(&path, &[("project.json", b"{}")]);
    assert_eq!(ProjectSession::open(&path).err().unwrap().code, DAMAGED);
}

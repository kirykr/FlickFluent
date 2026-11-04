//! Installing catalogue entries of every shape: several files, ffmpeg archives, and removal.

mod support;

use std::io::Write;
use std::path::Path;

use flickfluent_core::models::{
    delete_model, install_entry, installed, unpack_zip, CatalogueEntry, Control, FileSpec, Outcome,
    Step, Target,
};
use support::{serve, Behavior};

fn sha(body: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn target(dir: &Path) -> Target<'_> {
    Target {
        models_dir: dir,
        os: "macos",
        arch: "arm64",
        sidecar_args: vec![],
    }
}

fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        for (name, data) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[tokio::test]
async fn a_model_with_several_files_lands_in_its_own_folder_and_is_recorded_once() {
    let dir = tempfile::tempdir().unwrap();
    let body = b"0123456789".repeat(1000);
    let server = serve(Behavior::new(body.clone()));
    let file = |name: &str| FileSpec {
        name: name.into(),
        url: server.url.clone(),
        sha256: sha(&body),
        size_bytes: body.len() as u64,
    };
    let entry = CatalogueEntry {
        id: "multi".into(),
        step: Step::Transcribe,
        size_bytes: 2 * body.len() as u64,
        files: vec![file("model.bin"), file("sub/vocab.txt")],
        ..Default::default()
    };
    let mut last = 0;
    let out = install_entry(&entry, &target(dir.path()), &Control::new(), &mut |p| {
        assert_eq!(p.size_bytes, 2 * body.len() as u64);
        assert!(
            p.bytes_done >= last || p.bytes_done == 0 || p.phase as u8 == 1,
            "progress goes forward"
        );
        last = p.bytes_done;
    })
    .await
    .unwrap();
    assert_eq!(out, Outcome::Completed(dir.path().join("multi")));
    assert!(
        dir.path().join("multi/model.bin").is_file()
            && dir.path().join("multi/sub/vocab.txt").is_file()
    );
    let list = installed(dir.path());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].size_bytes, 2 * body.len() as u64);

    delete_model(dir.path(), &entry).unwrap();
    assert!(!dir.path().join("multi").exists());
    assert!(installed(dir.path()).is_empty());
}

#[tokio::test]
async fn a_wrong_hash_in_the_second_file_installs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let body = b"abc".repeat(500);
    let server = serve(Behavior::new(body.clone()));
    let good = FileSpec {
        name: "a".into(),
        url: server.url.clone(),
        sha256: sha(&body),
        size_bytes: body.len() as u64,
    };
    let bad = FileSpec {
        name: "b".into(),
        sha256: "0".repeat(64),
        ..good.clone()
    };
    let entry = CatalogueEntry {
        id: "bad".into(),
        size_bytes: 2 * body.len() as u64,
        files: vec![good, bad],
        ..Default::default()
    };
    let err = install_entry(&entry, &target(dir.path()), &Control::new(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, flickfluent_core::error::HASH_MISMATCH);
    assert!(installed(dir.path()).is_empty());
}

#[tokio::test]
async fn ffmpeg_in_a_zip_is_placed_under_models_ffmpeg_bin_with_its_licence() {
    let dir = tempfile::tempdir().unwrap();
    let body = zip_with(&[
        ("ffmpeg-n8-lgpl/bin/ffmpeg", b"#!/bin/sh\n"),
        ("ffmpeg-n8-lgpl/bin/ffprobe", b"#!/bin/sh\n"),
        ("ffmpeg-n8-lgpl/LICENSE.txt", b"LGPL text"),
    ]);
    let server = serve(Behavior::new(body.clone()));
    let entry = CatalogueEntry {
        id: "ffmpeg-test".into(),
        step: Step::Ffmpeg,
        size_bytes: body.len() as u64,
        url: format!("{}.zip", server.url),
        sha256: sha(&body),
        file_name: Some("ffmpeg.zip".into()),
        ..Default::default()
    };
    install_entry(&entry, &target(dir.path()), &Control::new(), &mut |_| {})
        .await
        .unwrap();
    assert!(
        dir.path().join("ffmpeg/bin/ffmpeg").is_file()
            && dir.path().join("ffmpeg/bin/ffprobe").is_file()
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("ffmpeg/LICENSE.txt")).unwrap(),
        "LGPL text"
    );
    assert!(
        !dir.path().join("ffmpeg-test/ffmpeg.zip").exists(),
        "archive removed"
    );
    assert!(!dir.path().join("ffmpeg.staging").exists());
    assert!(flickfluent_core::pipeline::ffmpeg::Ffmpeg::locate(dir.path()).is_some());

    delete_model(dir.path(), &entry).unwrap();
    assert!(!dir.path().join("ffmpeg").exists());
}

#[test]
fn zip_entries_cannot_escape_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    std::fs::write(&archive, zip_with(&[("../escaped.txt", b"x")])).unwrap();
    assert!(unpack_zip(&archive, &dir.path().join("out")).is_err());
    assert!(!dir.path().join("escaped.txt").exists());
}

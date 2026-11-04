//! Runtime unpacking is safe; ffmpeg probing maps files to the right errors.

use std::io::Write;

use flickfluent_core::error::{NO_AUDIO_TRACK, UNREADABLE_FILE};
use flickfluent_core::models::runtime::unpack_tar_gz;
use flickfluent_core::pipeline::ffmpeg::Ffmpeg;
use flickfluent_core::pipeline::sections::parse_silencedetect;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut raw = Vec::new();
    {
        let mut b = tar::Builder::new(&mut raw);
        for (name, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            // Bypass the builder's own path check so the test can make a hostile archive.
            h.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name.as_bytes());
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        b.finish().unwrap();
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&raw).unwrap();
    gz.finish().unwrap()
}

#[test]
fn unpack_puts_files_under_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("pack.tar.gz");
    std::fs::write(&archive, tar_gz(&[("python/bin/python3", b"x")])).unwrap();
    let dest = dir.path().join("runtime");
    unpack_tar_gz(&archive, &dest).unwrap();
    assert!(dest.join("python/bin/python3").exists());
}

#[test]
fn unpack_refuses_entries_that_escape_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.tar.gz");
    std::fs::write(&archive, tar_gz(&[("../escaped.txt", b"x")])).unwrap();
    let dest = dir.path().join("runtime");
    assert!(unpack_tar_gz(&archive, &dest).is_err());
    assert!(!dir.path().join("escaped.txt").exists());
}

#[test]
fn silencedetect_log_is_parsed() {
    let log = "[silencedetect @ 0x1] silence_start: 12.5\n[silencedetect @ 0x1] silence_end: 13.25 | silence_duration: 0.75\n";
    let s = parse_silencedetect(log);
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].start_ms, s[0].end_ms), (12_500, 13_250));
}

fn ffmpeg() -> Option<Ffmpeg> {
    Ffmpeg::locate(std::path::Path::new("/nonexistent"))
}

#[tokio::test]
async fn probe_reads_the_sample_clip() {
    let Some(f) = ffmpeg() else {
        return eprintln!("ffmpeg not found; skipped");
    };
    let info = f.probe(&fixture("clip-20s-fr.mp4")).await.unwrap();
    assert!(info.has_audio && info.has_video);
    assert!(
        (info.duration_ms as i64 - 20_000).abs() < 150,
        "{}",
        info.duration_ms
    );
}

#[tokio::test]
async fn silent_video_has_no_audio_track() {
    let Some(f) = ffmpeg() else {
        return eprintln!("ffmpeg not found; skipped");
    };
    let info = f.probe(&fixture("clip-no-audio.mp4")).await.unwrap();
    assert!(!info.has_audio);
    assert_eq!(
        Ffmpeg::require_audio(&info).unwrap_err().code,
        NO_AUDIO_TRACK
    );
}

#[tokio::test]
async fn corrupt_file_is_unreadable() {
    let Some(f) = ffmpeg() else {
        return eprintln!("ffmpeg not found; skipped");
    };
    let err = f.probe(&fixture("clip-corrupt.mp4")).await.unwrap_err();
    assert_eq!(err.code, UNREADABLE_FILE);
}

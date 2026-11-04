//! Stage 1, extract: both audio copies, exact duration, and the two source errors.

use std::path::{Path, PathBuf};

use flickfluent_core::error::{NO_AUDIO_TRACK, UNREADABLE_FILE};
use flickfluent_core::pipeline::ffmpeg::Ffmpeg;
use flickfluent_core::pipeline::stages::{extract, fingerprint, probe_source};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

fn ffmpeg() -> Ffmpeg {
    Ffmpeg::locate(Path::new("/nonexistent")).expect("ffmpeg on PATH or FLICK_FFMPEG_DIR")
}

/// `(channels, sample rate, duration ms)` of a wav from ffprobe.
fn wav_facts(path: &Path) -> (u32, u32, u64) {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=channels,sample_rate:format=duration",
        ])
        .args(["-of", "json"])
        .arg(path)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let s = &v["streams"][0];
    (
        s["channels"].as_u64().unwrap() as u32,
        s["sample_rate"].as_str().unwrap().parse().unwrap(),
        (v["format"]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            * 1000.0)
            .round() as u64,
    )
}

#[tokio::test]
async fn extract_writes_both_copies_with_the_right_format_and_length() {
    let work = tempfile::tempdir().unwrap();
    let source = fixture("clip-20s-fr.mp4");
    let info = probe_source(&ffmpeg(), &source).await.unwrap();
    let out = extract(
        &ffmpeg(),
        &source,
        work.path(),
        info.duration_ms,
        &mut |_| {},
    )
    .await
    .unwrap();

    let (ch, rate, ms) = wav_facts(&out.asr);
    assert_eq!((ch, rate), (1, 16_000));
    assert!(
        ms.abs_diff(info.duration_ms) <= 50,
        "asr {ms} vs source {}",
        info.duration_ms
    );

    let (ch, rate, ms) = wav_facts(&out.full);
    assert_eq!((ch, rate), (2, 44_100));
    assert!(
        ms.abs_diff(info.duration_ms) <= 50,
        "full {ms} vs source {}",
        info.duration_ms
    );

    assert!(work.path().join("source-info.json").exists());
    assert!(
        !work.path().join("audio.wav.part").exists(),
        "no leftover .part file"
    );
}

#[tokio::test]
async fn a_video_without_audio_gives_no_audio_track() {
    let err = probe_source(&ffmpeg(), &fixture("clip-no-audio.mp4"))
        .await
        .unwrap_err();
    assert_eq!(err.code, NO_AUDIO_TRACK);
}

#[tokio::test]
async fn a_corrupt_file_is_unreadable() {
    let err = probe_source(&ffmpeg(), &fixture("clip-corrupt.mp4"))
        .await
        .unwrap_err();
    assert_eq!(err.code, UNREADABLE_FILE);
}

#[test]
fn fingerprint_changes_with_content_and_size_only() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.bin");
    let b = dir.path().join("b.bin");
    let data = vec![7u8; 3 * 1024 * 1024];
    std::fs::write(&a, &data).unwrap();
    std::fs::write(&b, &data).unwrap();
    assert_eq!(
        fingerprint(&a).unwrap(),
        fingerprint(&b).unwrap(),
        "same bytes, different name"
    );

    let mut changed = data.clone();
    changed[10] = 9; // inside the first MB
    std::fs::write(&b, &changed).unwrap();
    assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());

    let mut longer = data;
    longer.push(0);
    std::fs::write(&b, &longer).unwrap();
    assert_ne!(
        fingerprint(&a).unwrap(),
        fingerprint(&b).unwrap(),
        "size is part of it"
    );
}

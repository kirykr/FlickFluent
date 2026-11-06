//! The post chain turns a raw engine clip into the line clip with ffmpeg: speed, volume and
//! pitch cost milliseconds and never reach an engine (research R4).

mod support;

use std::ffi::OsStr;
use std::path::Path;

use flickfluent_core::cloud::audio::wav_duration_ms;
use flickfluent_core::editor::post::make_line_clip;
use support::run_fixture::ffmpeg;

const HZ: f64 = 440.0;
const RATE: f64 = 24_000.0;

async fn raw_tone(path: &Path, ms: u64) {
    let spec = format!(
        "sine=frequency={HZ}:sample_rate={RATE}:duration={:.3}",
        ms as f64 / 1000.0
    );
    ffmpeg()
        .run(
            &[
                OsStr::new("-f"),
                OsStr::new("lavfi"),
                OsStr::new("-i"),
                OsStr::new(&spec),
                OsStr::new("-ac"),
                OsStr::new("1"),
                path.as_os_str(),
            ],
            ms,
            &mut |_| {},
        )
        .await
        .unwrap();
}

fn pcm(path: &Path) -> Vec<f64> {
    let b = std::fs::read(path).unwrap();
    let mut at = 12;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        if &b[at..at + 4] == b"data" {
            let end = (at + 8 + size).min(b.len());
            return b[at + 8..end]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64)
                .collect();
        }
        at += 8 + size + (size & 1);
    }
    panic!("no data chunk");
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
}

/// Frequency of a clean tone from its zero crossings.
fn frequency(x: &[f64], rate: f64) -> f64 {
    let crossings = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    crossings as f64 / (x.len() as f64 / rate)
}

#[tokio::test]
async fn speed_sets_the_length_within_20_ms() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw.wav");
    raw_tone(&raw, 3_000).await;
    let raw_ms = wav_duration_ms(&raw).unwrap();
    for speed in [0.85, 1.0, 1.15, 1.30] {
        let out = dir.path().join(format!("s{speed}.wav"));
        make_line_clip(&ffmpeg(), &raw, &out, speed, 0.0, 0.0)
            .await
            .unwrap();
        let ms = wav_duration_ms(&out).unwrap() as f64;
        let want = raw_ms as f64 / speed;
        assert!(
            (ms - want).abs() <= 20.0,
            "speed {speed}: {ms} ms, wanted {want:.0}"
        );
    }
}

#[tokio::test]
async fn volume_changes_the_level_by_the_set_decibels() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw.wav");
    raw_tone(&raw, 2_000).await;
    let base = rms(&pcm(&raw));
    for db in [-12.0, -6.0, 3.0, 6.0] {
        let out = dir.path().join(format!("v{db}.wav"));
        make_line_clip(&ffmpeg(), &raw, &out, 1.0, db, 0.0)
            .await
            .unwrap();
        let got = 20.0 * (rms(&pcm(&out)) / base).log10();
        assert!((got - db).abs() <= 0.5, "{db} dB gave {got:.2} dB");
    }
}

#[tokio::test]
async fn pitch_moves_the_tone_and_leaves_the_length() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw.wav");
    raw_tone(&raw, 3_000).await;
    let raw_ms = wav_duration_ms(&raw).unwrap() as f64;
    for semitones in [-4.0, -2.0, 2.0, 4.0] {
        let out = dir.path().join(format!("p{semitones}.wav"));
        make_line_clip(&ffmpeg(), &raw, &out, 1.0, 0.0, semitones)
            .await
            .unwrap();
        let ms = wav_duration_ms(&out).unwrap() as f64;
        assert!(
            (ms - raw_ms).abs() <= 20.0,
            "{semitones} st: {ms} ms against {raw_ms}"
        );
        let want = HZ * 2f64.powf(semitones / 12.0);
        let got = frequency(&pcm(&out), RATE);
        assert!(
            (got / want - 1.0).abs() < 0.03,
            "{semitones} st: {got:.0} Hz, wanted {want:.0}"
        );
    }
}

#[tokio::test]
async fn all_three_together_and_nothing_is_left_half_written() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw.wav");
    raw_tone(&raw, 2_000).await;
    let out = dir.path().join("both.wav");
    make_line_clip(&ffmpeg(), &raw, &out, 1.2, -3.0, 3.0)
        .await
        .unwrap();
    assert!((wav_duration_ms(&out).unwrap() as f64 - 2_000.0 / 1.2).abs() <= 20.0);
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .collect();
    assert!(left.is_empty());
    // A raw clip that is not there is an error, and no output appears.
    let missing = dir.path().join("missing.wav");
    let bad = dir.path().join("bad.wav");
    assert!(make_line_clip(&ffmpeg(), &missing, &bad, 1.0, 0.0, 0.0)
        .await
        .is_err());
    assert!(!bad.exists());
}

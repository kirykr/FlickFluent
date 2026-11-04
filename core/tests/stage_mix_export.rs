//! Stages 6 and 7, mix and export, on tones made with ffmpeg so the energy checks are exact.

use std::path::{Path, PathBuf};
use std::process::Command;

use flickfluent_core::error::{FILE_EXISTS, INVALID_PROJECT};
use flickfluent_core::pipeline::ffmpeg::Ffmpeg;
use flickfluent_core::pipeline::mix::{mix, MixLine, MixSection};
use flickfluent_core::pipeline::stages::{export, ExportRequest};
use flickfluent_core::project::dub::ExportKind;

const TOTAL_MS: u64 = 20_000;

fn ffmpeg() -> Ffmpeg {
    Ffmpeg::locate(Path::new("/nonexistent")).expect("ffmpeg on PATH or FLICK_FFMPEG_DIR")
}

/// An ffmpeg `sine` source peaks at 0.125 (rms 0.088); `gain` scales that.
fn tone(path: &Path, hz: u32, secs: f32, rate: u32, channels: u32, gain: f32) {
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency={hz}:duration={secs}:sample_rate={rate}"
        ))
        .args([
            "-af",
            &format!("volume={gain}"),
            "-ac",
            &channels.to_string(),
            "-c:a",
            "pcm_s16le",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

/// Mono 44.1 kHz samples of any audio file.
fn samples(path: &Path) -> Vec<i16> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-ac", "1", "-ar", "44100", "-f", "s16le", "-"])
        .output()
        .unwrap();
    out.stdout
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}

/// Root mean square between two times, as a fraction of full scale.
fn rms(s: &[i16], from_ms: u64, to_ms: u64) -> f64 {
    let (a, b) = (
        (from_ms * 44_100 / 1000) as usize,
        (to_ms * 44_100 / 1000) as usize,
    );
    let part = &s[a..b.min(s.len())];
    (part
        .iter()
        .map(|v| (*v as f64 / 32768.0).powi(2))
        .sum::<f64>()
        / part.len() as f64)
        .sqrt()
}

struct Setup {
    dir: tempfile::TempDir,
    original: PathBuf,
    music: PathBuf,
    clips: Vec<(u64, PathBuf)>,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("audio-full.wav");
    let music = dir.path().join("music.wav");
    tone(&original, 440, 20.0, 44_100, 2, 1.0);
    tone(&music, 220, 20.0, 44_100, 2, 0.2);
    let mut clips = Vec::new();
    for (i, start) in [5_000u64, 12_000].into_iter().enumerate() {
        let clip = dir.path().join(format!("line{i}.wav"));
        tone(&clip, 1000, 1.0, 24_000, 1, 1.0); // native voice format: mono 24 kHz
        clips.push((start, clip));
    }
    Setup {
        dir,
        original,
        music,
        clips,
    }
}

fn sections(s: &Setup, with_music: bool) -> Vec<MixSection> {
    vec![MixSection {
        start_ms: 0,
        end_ms: TOTAL_MS,
        music: with_music.then(|| s.music.clone()),
        lines: s
            .clips
            .iter()
            .map(|(start_ms, clip)| MixLine {
                start_ms: *start_ms,
                clip: clip.clone(),
            })
            .collect(),
    }]
}

#[tokio::test]
async fn mix_is_source_length_with_dub_at_each_line_and_music_elsewhere() {
    let s = setup();
    let out = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &sections(&s, true),
        true,
        &mut |_| {},
    )
    .await
    .unwrap();
    let m = samples(&out);
    assert!(
        (m.len() as i64 - (TOTAL_MS * 44_100 / 1000) as i64).abs() <= 44_100 / 20,
        "length within 50 ms"
    );
    for (start, _) in &s.clips {
        assert!(
            rms(&m, *start + 100, *start + 300) > 0.05,
            "dub energy at {start}"
        );
    }
    let quiet = rms(&m, 1_000, 3_000);
    assert!(quiet > 0.01, "music is there where nobody speaks: {quiet}");
    assert!(
        rms(&m, 5_100, 5_300) > quiet * 3.0,
        "dub is louder than the bed"
    );
}

#[tokio::test]
async fn mute_original_removes_the_original_and_fast_mode_leaves_only_the_dub() {
    let s = setup();
    // Fast mode: no music stem, original muted. Nothing but the dub remains.
    let out = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &sections(&s, false),
        true,
        &mut |_| {},
    )
    .await
    .unwrap();
    let m = samples(&out);
    assert!(rms(&m, 1_000, 3_000) < 0.001, "silence where nobody speaks");
    assert!(rms(&m, 5_100, 5_300) > 0.05);

    // Original kept: its level is there under the dub.
    let out = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &sections(&s, false),
        false,
        &mut |_| {},
    )
    .await
    .unwrap();
    let m = samples(&out);
    assert!(rms(&m, 1_000, 3_000) > 0.05, "original still audible");
}

#[tokio::test]
async fn a_second_section_lines_up_with_the_first() {
    let s = setup();
    let secs = vec![
        MixSection {
            start_ms: 0,
            end_ms: 10_000,
            music: None,
            lines: vec![MixLine {
                start_ms: 5_000,
                clip: s.clips[0].1.clone(),
            }],
        },
        MixSection {
            start_ms: 10_000,
            end_ms: TOTAL_MS,
            music: None,
            lines: vec![MixLine {
                start_ms: 12_000,
                clip: s.clips[1].1.clone(),
            }],
        },
    ];
    let out = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &secs,
        true,
        &mut |_| {},
    )
    .await
    .unwrap();
    let m = samples(&out);
    assert!((m.len() as i64 - (TOTAL_MS * 44_100 / 1000) as i64).abs() <= 44_100 / 20);
    assert!(rms(&m, 5_100, 5_300) > 0.05 && rms(&m, 12_100, 12_300) > 0.05);
    assert!(rms(&m, 8_000, 9_000) < 0.001);
}

fn video_md5(path: &Path) -> String {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-map", "0:v:0", "-c", "copy", "-f", "md5", "-"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn stream_kinds(path: &Path) -> String {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn request<'a>(
    source: &'a Path,
    mix: &'a Path,
    folder: &'a Path,
    kind: ExportKind,
    name: &'a str,
) -> ExportRequest<'a> {
    ExportRequest {
        source,
        mix,
        folder,
        kind,
        file_name: name,
        overwrite: false,
        duration_ms: 20_000,
    }
}

#[tokio::test]
async fn export_copies_the_video_and_audio_only_has_no_video() {
    let s = setup();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-20s-fr.mp4");
    let mixed = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &sections(&s, true),
        true,
        &mut |_| {},
    )
    .await
    .unwrap();
    let folder = tempfile::tempdir().unwrap();

    let video = export(
        &ffmpeg(),
        &request(&source, &mixed, folder.path(), ExportKind::Video, "My dub"),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(video.path.file_name().unwrap(), "My dub.mp4");
    assert_eq!(
        video_md5(&video.path),
        video_md5(&source),
        "video stream is bit-identical"
    );
    assert!(stream_kinds(&video.path).contains("audio"));

    let audio = export(
        &ffmpeg(),
        &request(&source, &mixed, folder.path(), ExportKind::Audio, "My dub"),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(audio.path.file_name().unwrap(), "My dub.m4a");
    assert!(!stream_kinds(&audio.path).contains("video"));
    assert!(audio.size_bytes > 0);
}

#[tokio::test]
async fn export_needs_overwrite_for_an_existing_file_and_refuses_unsafe_names() {
    let s = setup();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/clip-20s-fr.mp4");
    let mixed = mix(
        &ffmpeg(),
        s.dir.path(),
        &s.original,
        &sections(&s, true),
        true,
        &mut |_| {},
    )
    .await
    .unwrap();
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("out.m4a"), b"keep me").unwrap();

    let err = export(
        &ffmpeg(),
        &request(&source, &mixed, folder.path(), ExportKind::Audio, "out"),
        &mut |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, FILE_EXISTS);
    assert_eq!(
        std::fs::read(folder.path().join("out.m4a")).unwrap(),
        b"keep me"
    );

    let mut req = request(&source, &mixed, folder.path(), ExportKind::Audio, "out");
    req.overwrite = true;
    export(&ffmpeg(), &req, &mut |_| {}).await.unwrap();
    assert_ne!(
        std::fs::read(folder.path().join("out.m4a")).unwrap(),
        b"keep me"
    );

    for bad in ["../escape", "a/b", "", "  ", "con\0x"] {
        let err = export(
            &ffmpeg(),
            &request(&source, &mixed, folder.path(), ExportKind::Audio, bad),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, INVALID_PROJECT, "{bad:?}");
    }
}

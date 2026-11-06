//! After an edit the mix is rebuilt for the sections that changed and spliced into the old mix
//! file (research R3): the stretches before and after stay sample-equal, the length stays.

mod support;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use flickfluent_core::pipeline::mix::{mix, render_sections, splice, MixLine, MixSection};
use support::run_fixture::ffmpeg;

const RATE: usize = 44_100;

async fn tone(path: &Path, hz: u32, ms: u64) {
    let spec = format!(
        "sine=frequency={hz}:sample_rate={RATE}:duration={:.3}",
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

/// The samples of a PCM wav (16-bit), walking the chunks to the data chunk.
fn pcm(path: &Path) -> Vec<i16> {
    let b = std::fs::read(path).unwrap();
    let mut at = 12;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        if &b[at..at + 4] == b"data" {
            let end = (at + 8 + size).min(b.len());
            return b[at + 8..end]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
        }
        at += 8 + size + (size & 1);
    }
    panic!("no data chunk in {path:?}");
}

struct Film {
    dir: tempfile::TempDir,
    original: PathBuf,
}

async fn film() -> Film {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("audio-full.wav");
    tone(&original, 220, 30_000).await;
    Film { dir, original }
}

fn sections(clips: &[(u64, &Path)]) -> Vec<MixSection> {
    (0..3)
        .map(|i| MixSection {
            start_ms: i * 10_000,
            end_ms: (i + 1) * 10_000,
            music: None,
            lines: clips
                .iter()
                .filter(|(at, _)| *at / 10_000 == i)
                .map(|(at, clip)| MixLine {
                    start_ms: *at,
                    clip: clip.to_path_buf(),
                })
                .collect(),
        })
        .collect()
}

fn work_in(film: &Film, name: &str) -> PathBuf {
    let w = film.dir.path().join(name);
    std::fs::create_dir_all(&w).unwrap();
    w
}

#[tokio::test]
async fn only_the_touched_section_changes_and_the_rest_is_sample_equal() {
    let film = film().await;
    let (a, b, c) = (
        film.dir.path().join("a.wav"),
        film.dir.path().join("b.wav"),
        film.dir.path().join("c.wav"),
    );
    tone(&a, 440, 2_000).await;
    tone(&b, 660, 2_000).await;
    tone(&c, 880, 2_000).await;
    let work = work_in(&film, "work");
    let first = sections(&[(1_000, &a), (12_000, &b), (23_000, &c)]);
    let old = mix(&ffmpeg(), &work, &film.original, &first, false, &mut |_| {})
        .await
        .unwrap();
    let before = pcm(&old);

    // The line in the middle section is replaced by another tone.
    let b2 = film.dir.path().join("b2.wav");
    tone(&b2, 990, 2_000).await;
    let second = sections(&[(1_000, &a), (12_000, &b2), (23_000, &c)]);
    let pieces = render_sections(
        &ffmpeg(),
        &work,
        &film.original,
        &second,
        &[1],
        false,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(pieces.len(), 1);
    let out = splice(&ffmpeg(), &work, &old, &second, &pieces)
        .await
        .unwrap();
    let after = pcm(&out);

    assert_eq!(after.len(), before.len(), "same length");
    let stereo = |ms: usize| ms * RATE / 1000 * 2;
    assert_eq!(
        after[..stereo(10_000)],
        before[..stereo(10_000)],
        "before the section"
    );
    assert_eq!(
        after[stereo(20_000)..],
        before[stereo(20_000)..],
        "after the section"
    );
    assert_ne!(
        after[stereo(10_000)..stereo(20_000)],
        before[stereo(10_000)..stereo(20_000)],
        "the section itself changed"
    );

    // The splice equals a full remix of the new state.
    let full_work = work_in(&film, "full");
    let full = mix(
        &ffmpeg(),
        &full_work,
        &film.original,
        &second,
        false,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(pcm(&full), after);

    assert!(
        std::fs::read_dir(work.join("mix"))
            .unwrap()
            .next()
            .is_none(),
        "no piece is left"
    );
}

#[tokio::test]
async fn two_sections_can_be_rebuilt_together() {
    let film = film().await;
    let (a, b) = (film.dir.path().join("a.wav"), film.dir.path().join("b.wav"));
    tone(&a, 440, 3_000).await;
    tone(&b, 660, 3_000).await;
    let work = work_in(&film, "work");
    // The line sits at the end of section 0, running over into section 1's start window.
    let first = sections(&[(8_500, &a)]);
    let old = mix(&ffmpeg(), &work, &film.original, &first, false, &mut |_| {})
        .await
        .unwrap();
    // It moves into section 1: both sections are rebuilt.
    let second = sections(&[(10_500, &a), (25_000, &b)]);
    let pieces = render_sections(
        &ffmpeg(),
        &work,
        &film.original,
        &second,
        &[0, 1, 2],
        false,
        &mut |_| {},
    )
    .await
    .unwrap();
    let out = splice(&ffmpeg(), &work, &old, &second, &pieces)
        .await
        .unwrap();

    let full = mix(
        &ffmpeg(),
        &work_in(&film, "full"),
        &film.original,
        &second,
        false,
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(pcm(&out), pcm(&full));
}

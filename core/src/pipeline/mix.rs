//! Stage 6, mix: place every dubbed line clip at its start time over a bed, one section at a time
//! so the ffmpeg graph stays small and memory stays flat on a long movie.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::ffmpeg::Ffmpeg;
use crate::error::AppError;

const RATE: u32 = 44_100;

/// One synthesized line placed on the timeline.
#[derive(Debug, Clone)]
pub struct MixLine {
    /// From the start of the source, not of the section.
    pub start_ms: u64,
    pub clip: PathBuf,
}

#[derive(Debug, Clone)]
pub struct MixSection {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Music-only stem for this section, as long as the section. `None` in fast mode.
    pub music: Option<PathBuf>,
    pub lines: Vec<MixLine>,
}

fn secs(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// Render one section's piece, `mix/sec<start>.wav`: the bed with every line placed over it.
async fn render_piece(
    ffmpeg: &Ffmpeg,
    dir: &Path,
    original: &Path,
    sec: &MixSection,
    mute_original: bool,
    on_progress: &mut (dyn FnMut(f32) + Send),
) -> Result<PathBuf, AppError> {
    let len_ms = sec.end_ms - sec.start_ms;
    let piece = dir.join(format!("sec{:04}.wav", sec.start_ms));
    let part = dir.join(format!("sec{:04}.wav.part", sec.start_ms));
    let mut args: Vec<OsString> = Vec::new();

    // Input 0: the bed.
    let bed_original = !mute_original;
    match (&sec.music, bed_original) {
        (_, true) => {
            args.extend([
                "-ss".into(),
                secs(sec.start_ms).into(),
                "-t".into(),
                secs(len_ms).into(),
            ]);
            args.extend(["-i".into(), original.into()]);
        }
        (Some(music), false) => {
            args.extend(["-t".into(), secs(len_ms).into(), "-i".into(), music.into()])
        }
        (None, false) => args.extend([
            "-f".into(),
            "lavfi".into(),
            "-t".into(),
            secs(len_ms).into(),
            "-i".into(),
            format!("anullsrc=r={RATE}:cl=stereo").into(),
        ]),
    }
    let mut graph =
        format!("[0:a]aresample={RATE},aformat=sample_fmts=fltp:channel_layouts=stereo[bed];");
    let mut labels = String::from("[bed]");
    for (i, line) in sec.lines.iter().enumerate() {
        args.extend(["-i".into(), line.clip.clone().into()]);
        let delay = line.start_ms.saturating_sub(sec.start_ms);
        graph.push_str(&format!(
            "[{n}:a]aresample={RATE},aformat=sample_fmts=fltp:channel_layouts=stereo,adelay={delay}:all=1[l{i}];",
            n = i + 1
        ));
        labels.push_str(&format!("[l{i}]"));
    }
    graph.push_str(&format!(
        "{labels}amix=inputs={count}:normalize=0:duration=longest,apad,atrim=end={end},alimiter=limit=0.95[out]",
        count = sec.lines.len() + 1,
        end = secs(len_ms),
    ));
    let script = dir.join(format!("sec{:04}.graph", sec.start_ms));
    std::fs::write(&script, graph).map_err(AppError::io)?;
    args.extend([
        "-filter_complex_script".into(),
        script.clone().into(),
        "-map".into(),
        "[out]".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        part.clone().into(),
    ]);

    let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
    let result = ffmpeg.run(&refs, len_ms, on_progress).await;
    let _ = std::fs::remove_file(&script);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &piece).map_err(AppError::io)?;
    Ok(piece)
}

/// Build `dub-mix.wav` (stereo 44.1 kHz) in `work`, as long as the last section ends.
///
/// The bed under the dub is: the music stem when the original is muted and there is one; the
/// original audio from `original` when it is not muted; silence otherwise.
pub async fn mix(
    ffmpeg: &Ffmpeg,
    work: &Path,
    original: &Path,
    sections: &[MixSection],
    mute_original: bool,
    on_progress: &mut (dyn FnMut(f32) + Send),
) -> Result<PathBuf, AppError> {
    let dir = work.join("mix");
    std::fs::create_dir_all(&dir).map_err(AppError::io)?;
    let total_ms = sections.last().map(|s| s.end_ms).unwrap_or(0);
    let mut pieces = Vec::new();

    for (n, sec) in sections.iter().enumerate() {
        let base = n as f32 / sections.len() as f32;
        let width = 1.0 / sections.len() as f32;
        let piece = render_piece(ffmpeg, &dir, original, sec, mute_original, &mut |p| {
            on_progress(base + p * width)
        })
        .await?;
        pieces.push(piece);
    }

    // Join the pieces without re-encoding.
    let list = dir.join("concat.txt");
    let body: String = pieces
        .iter()
        .map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''")))
        .collect();
    std::fs::write(&list, body).map_err(AppError::io)?;
    let out = work.join("dub-mix.wav");
    let part = work.join("dub-mix.wav.part");
    let args: Vec<OsString> = vec![
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list.clone().into(),
        "-c".into(),
        "copy".into(),
        "-f".into(),
        "wav".into(),
        part.clone().into(),
    ];
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
    let joined = ffmpeg.run(&refs, total_ms, &mut |_| {}).await;
    let _ = std::fs::remove_file(&list);
    if let Err(e) = joined {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &out).map_err(AppError::io)?;
    for p in pieces {
        let _ = std::fs::remove_file(p);
    }
    on_progress(1.0);
    Ok(out)
}

/// Render only the sections at these indexes. Returns each piece with its index; the caller
/// splices them into the old mix with [`splice`].
pub async fn render_sections(
    ffmpeg: &Ffmpeg,
    work: &Path,
    original: &Path,
    sections: &[MixSection],
    only: &[usize],
    mute_original: bool,
    on_progress: &mut (dyn FnMut(f32) + Send),
) -> Result<Vec<(usize, PathBuf)>, AppError> {
    let dir = work.join("mix");
    std::fs::create_dir_all(&dir).map_err(AppError::io)?;
    let mut out = Vec::new();
    for (n, index) in only.iter().enumerate() {
        let Some(sec) = sections.get(*index) else {
            continue;
        };
        let base = n as f32 / only.len() as f32;
        let width = 1.0 / only.len() as f32;
        let piece = render_piece(ffmpeg, &dir, original, sec, mute_original, &mut |p| {
            on_progress(base + p * width)
        })
        .await?;
        out.push((*index, piece));
    }
    Ok(out)
}

fn sample_at(ms: u64) -> u64 {
    (ms * RATE as u64 + 500) / 1000
}

/// One stretch of the spliced mix.
enum Part<'a> {
    /// Samples `from` to `to` (or the end) of the old mix.
    Old {
        from: u64,
        to: Option<u64>,
    },
    Piece(&'a Path),
}

/// Put `pieces` (rendered with [`render_sections`]) into `old_mix` in place of their sections,
/// and write the result as `dub-mix.wav`. The stretches between come from the old file by
/// sample number, so they stay sample-equal and the length does not change. The pieces are
/// removed afterwards.
pub async fn splice(
    ffmpeg: &Ffmpeg,
    work: &Path,
    old_mix: &Path,
    sections: &[MixSection],
    pieces: &[(usize, PathBuf)],
) -> Result<PathBuf, AppError> {
    let dir = work.join("mix");
    std::fs::create_dir_all(&dir).map_err(AppError::io)?;
    let mut ordered: Vec<&(usize, PathBuf)> = pieces.iter().collect();
    ordered.sort_by_key(|(i, _)| *i);
    let total_ms = sections.last().map(|s| s.end_ms).unwrap_or(0);

    let mut parts: Vec<Part> = Vec::new();
    let mut at_ms = 0u64;
    for (index, piece) in &ordered {
        let sec = &sections[*index];
        if sec.start_ms > at_ms {
            parts.push(Part::Old {
                from: sample_at(at_ms),
                to: Some(sample_at(sec.start_ms)),
            });
        }
        parts.push(Part::Piece(piece));
        at_ms = sec.end_ms;
    }
    if at_ms < total_ms {
        parts.push(Part::Old {
            from: sample_at(at_ms),
            to: None,
        });
    }

    // One input per part, in playing order; the old mix is opened once per stretch.
    let mut args: Vec<OsString> = Vec::new();
    let mut graph = String::new();
    let mut labels = String::new();
    for (n, part) in parts.iter().enumerate() {
        match part {
            Part::Old { from, to } => {
                args.extend(["-i".into(), old_mix.as_os_str().to_owned()]);
                let range = match to {
                    Some(to) => format!("start_sample={from}:end_sample={to}"),
                    None => format!("start_sample={from}"),
                };
                graph.push_str(&format!("[{n}:a]atrim={range},asetpts=PTS-STARTPTS[p{n}];"));
                labels.push_str(&format!("[p{n}]"));
            }
            Part::Piece(piece) => {
                args.extend(["-i".into(), piece.as_os_str().to_owned()]);
                labels.push_str(&format!("[{n}:a]"));
            }
        }
    }
    graph.push_str(&format!("{labels}concat=n={}:v=0:a=1[out]", parts.len()));
    let script = dir.join("splice.graph");
    std::fs::write(&script, graph).map_err(AppError::io)?;
    let out = work.join("dub-mix.wav");
    let part = work.join("dub-mix.wav.part");
    args.extend([
        "-filter_complex_script".into(),
        script.clone().into(),
        "-map".into(),
        "[out]".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        part.clone().into(),
    ]);
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
    let joined = ffmpeg.run(&refs, total_ms, &mut |_| {}).await;
    let _ = std::fs::remove_file(&script);
    if let Err(e) = joined {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &out).map_err(AppError::io)?;
    for (_, piece) in pieces {
        let _ = std::fs::remove_file(piece);
    }
    Ok(out)
}

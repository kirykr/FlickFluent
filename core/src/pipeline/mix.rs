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
        let base = n as f32 / sections.len() as f32;
        let width = 1.0 / sections.len() as f32;
        let result = ffmpeg
            .run(&refs, len_ms, &mut |p| on_progress(base + p * width))
            .await;
        let _ = std::fs::remove_file(&script);
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        std::fs::rename(&part, &piece).map_err(AppError::io)?;
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

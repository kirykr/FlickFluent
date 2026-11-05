//! Stages that run in the core (no model): extract and export.

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::ffmpeg::{Ffmpeg, SourceInfo};
use crate::error::{AppError, FILE_EXISTS, INVALID_PROJECT};
use crate::fsutil::write_atomic;
use crate::models::preflight_space;
use crate::project::dub::ExportKind;

const FINGERPRINT_EDGE: u64 = 1024 * 1024;

/// `dub_probe_source`: facts about the file, or `UNREADABLE_FILE` / `NO_AUDIO_TRACK`.
pub async fn probe_source(ffmpeg: &Ffmpeg, path: &Path) -> Result<SourceInfo, AppError> {
    let info = ffmpeg.probe(path).await?;
    Ffmpeg::require_audio(&info)?;
    Ok(info)
}

/// Hash of the file size plus the first and last megabyte. Cheap on a large movie, and enough to
/// notice a replaced or moved file.
pub fn fingerprint(path: &Path) -> Result<String, AppError> {
    let mut file = std::fs::File::open(path).map_err(AppError::io)?;
    let size = file.metadata().map_err(AppError::io)?.len();
    let mut hash = Sha256::new();
    hash.update(size.to_le_bytes());
    let mut buf = vec![0u8; FINGERPRINT_EDGE.min(size) as usize];
    file.read_exact(&mut buf).map_err(AppError::io)?;
    hash.update(&buf);
    if size > FINGERPRINT_EDGE {
        let tail = FINGERPRINT_EDGE.min(size - FINGERPRINT_EDGE) as usize;
        file.seek(SeekFrom::End(-(tail as i64)))
            .map_err(AppError::io)?;
        let mut buf = vec![0u8; tail];
        file.read_exact(&mut buf).map_err(AppError::io)?;
        hash.update(&buf);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Debug, Clone)]
pub struct Extracted {
    /// Mono 16 kHz, for speech recognition.
    pub asr: PathBuf,
    /// Stereo 44.1 kHz, for separation and mixing.
    pub full: PathBuf,
}

/// Write `audio.wav` and `audio-full.wav` in `work`, each as `.part` then renamed, plus
/// `source-info.json`. `on_progress` goes 0 to 1 across both files.
pub async fn extract(
    ffmpeg: &Ffmpeg,
    source: &Path,
    work: &Path,
    duration_ms: u64,
    fingerprint: &str,
    on_progress: &mut (dyn FnMut(f32) + Send),
) -> Result<Extracted, AppError> {
    std::fs::create_dir_all(work).map_err(AppError::io)?;
    let asr = work.join("audio.wav");
    let full = work.join("audio-full.wav");
    let jobs: [(&Path, &[&str]); 2] = [
        (&asr, &["-ac", "1", "-ar", "16000"]),
        (&full, &["-ac", "2", "-ar", "44100"]),
    ];
    for (i, (target, format)) in jobs.iter().enumerate() {
        let part = target.with_extension("wav.part");
        let mut args: Vec<OsString> = vec!["-i".into(), source.into(), "-vn".into()];
        args.extend(format.iter().map(OsString::from));
        args.extend([
            "-c:a".into(),
            "pcm_s16le".into(),
            "-f".into(),
            "wav".into(),
            part.clone().into(),
        ]);
        let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
        let base = i as f32 / 2.0;
        let result = ffmpeg
            .run(&refs, duration_ms, &mut |p| on_progress(base + p / 2.0))
            .await;
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        std::fs::rename(&part, target).map_err(AppError::io)?;
    }
    let info = serde_json::json!({ "durationMs": duration_ms, "fingerprint": fingerprint });
    write_atomic(&work.join("source-info.json"), info.to_string().as_bytes())?;
    Ok(Extracted { asr, full })
}

pub struct ExportRequest<'a> {
    pub source: &'a Path,
    /// `dub-mix.wav` from the mix stage.
    pub mix: &'a Path,
    pub folder: &'a Path,
    pub kind: ExportKind,
    /// Without extension. The extension follows `kind`.
    pub file_name: &'a str,
    pub overwrite: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// A name that is one plain file name: no folders, no control characters, not empty. The same
/// characters `project::file_name_for` replaces are refused here instead of silently changed,
/// because the user typed this name for a file they will look for.
fn check_file_name(name: &str) -> Result<(), AppError> {
    let bad = name.trim().is_empty()
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        || name.trim_end().ends_with('.');
    if bad {
        return Err(AppError::new(INVALID_PROJECT, "error.invalidProject")
            .with_details(serde_json::json!({ "detail": "unsafe export file name" })));
    }
    Ok(())
}

/// Stage 7. Video: the source video stream is copied untouched and the dub replaces the audio.
/// Audio: the dub alone, as M4A. Written as `.part`, then renamed.
pub async fn export(
    ffmpeg: &Ffmpeg,
    req: &ExportRequest<'_>,
    on_progress: &mut (dyn FnMut(f32) + Send),
) -> Result<ExportResult, AppError> {
    check_file_name(req.file_name)?;
    let (ext, format) = match req.kind {
        ExportKind::Video => ("mp4", "mp4"),
        ExportKind::Audio => ("m4a", "ipod"),
    };
    let target = req.folder.join(format!("{}.{ext}", req.file_name.trim()));
    if target.exists() && !req.overwrite {
        return Err(AppError::new(FILE_EXISTS, "dub.error.fileExists"));
    }
    let needed = match req.kind {
        ExportKind::Video => std::fs::metadata(req.source).map_err(AppError::io)?.len(),
        ExportKind::Audio => std::fs::metadata(req.mix).map_err(AppError::io)?.len(),
    };
    preflight_space(needed, req.folder)?;

    let part = target.with_extension(format!("{ext}.part"));
    let mut args: Vec<OsString> = Vec::new();
    match req.kind {
        ExportKind::Video => args.extend([
            "-i".into(),
            req.source.into(),
            "-i".into(),
            req.mix.into(),
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "1:a:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-movflags".into(),
            "+faststart".into(),
        ]),
        ExportKind::Audio => args.extend(["-i".into(), req.mix.into(), "-vn".into()]),
    }
    args.extend([
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-f".into(),
        format.into(),
        part.clone().into(),
    ]);
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
    if let Err(e) = ffmpeg.run(&refs, req.duration_ms, on_progress).await {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &target).map_err(AppError::io)?;
    let size_bytes = std::fs::metadata(&target).map_err(AppError::io)?.len();
    Ok(ExportResult {
        path: target,
        size_bytes,
    })
}

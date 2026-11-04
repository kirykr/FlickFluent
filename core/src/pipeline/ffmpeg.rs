//! ffmpeg and ffprobe as subprocesses: probe, run with parsed progress, find silences.
//! Streaming through ffmpeg keeps memory flat for long movies.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::sections::{parse_silencedetect, Silence};
use crate::error::{AppError, NO_AUDIO_TRACK, STAGE_FAILED, UNREADABLE_FILE};

/// Facts about a media file, as `dub_probe_source` returns them.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub duration_ms: u64,
    pub has_audio: bool,
    pub has_video: bool,
    pub container: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Ffmpeg {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

impl Ffmpeg {
    pub fn at(dir: &Path) -> Self {
        Self {
            ffmpeg: dir.join(exe("ffmpeg")),
            ffprobe: dir.join(exe("ffprobe")),
        }
    }

    /// Order: `FLICK_FFMPEG_DIR` (tests and development), the installed LGPL build under
    /// `<models>/ffmpeg/`, then ffmpeg already on the PATH.
    pub fn locate(models_dir: &Path) -> Option<Self> {
        if let Ok(dir) = std::env::var("FLICK_FFMPEG_DIR") {
            let f = Self::at(Path::new(&dir));
            if f.ffmpeg.exists() {
                return Some(f);
            }
        }
        let installed = Self::at(&models_dir.join("ffmpeg").join("bin"));
        if installed.ffmpeg.exists() && installed.ffprobe.exists() {
            return Some(installed);
        }
        let ok = |name: &str| {
            std::process::Command::new(exe(name))
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };
        (ok("ffmpeg") && ok("ffprobe")).then(|| Self {
            ffmpeg: PathBuf::from(exe("ffmpeg")),
            ffprobe: PathBuf::from(exe("ffprobe")),
        })
    }

    pub async fn probe(&self, path: &Path) -> Result<SourceInfo, AppError> {
        let unreadable = |why: &str| {
            AppError::new(UNREADABLE_FILE, "dub.error.unreadableFile")
                .with_details(serde_json::json!({ "path": path.to_string_lossy(), "reason": why }))
        };
        let size_bytes = std::fs::metadata(path)
            .map_err(|e| unreadable(&e.to_string()))?
            .len();
        let out = Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(AppError::io)?;
        if !out.status.success() {
            return Err(unreadable(String::from_utf8_lossy(&out.stderr).trim()));
        }
        let json: Value =
            serde_json::from_slice(&out.stdout).map_err(|e| unreadable(&e.to_string()))?;
        let streams = json["streams"].as_array().cloned().unwrap_or_default();
        let kind = |k: &str| streams.iter().any(|s| s["codec_type"] == k);
        let seconds = json["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .ok_or_else(|| unreadable("no duration"))?;
        Ok(SourceInfo {
            duration_ms: (seconds * 1000.0).round() as u64,
            has_audio: kind("audio"),
            has_video: kind("video"),
            container: json["format"]["format_name"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            size_bytes,
        })
    }

    /// Error for a source with no audio stream.
    pub fn require_audio(info: &SourceInfo) -> Result<(), AppError> {
        if info.has_audio {
            Ok(())
        } else {
            Err(AppError::new(NO_AUDIO_TRACK, "dub.error.noAudioTrack"))
        }
    }

    /// Run ffmpeg with `args`. `total_ms` turns `out_time_us` into a 0 to 1 fraction for
    /// `on_progress`. On failure the last stderr lines come back in the error details.
    pub async fn run(
        &self,
        args: &[&std::ffi::OsStr],
        total_ms: u64,
        on_progress: &mut (dyn FnMut(f32) + Send),
    ) -> Result<(), AppError> {
        let mut child = Command::new(&self.ffmpeg)
            .args([
                "-v",
                "error",
                "-nostdin",
                "-y",
                "-progress",
                "pipe:1",
                "-nostats",
            ])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(AppError::io)?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let err_task = tokio::spawn(async move {
            let mut text = String::new();
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                text.push_str(&l);
                text.push('\n');
            }
            text
        });
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(us) = line
                .strip_prefix("out_time_us=")
                .and_then(|v| v.trim().parse::<i64>().ok())
            {
                if total_ms > 0 && us >= 0 {
                    on_progress(((us as f64 / 1000.0) / total_ms as f64).clamp(0.0, 1.0) as f32);
                }
            }
        }
        let status = child.wait().await.map_err(AppError::io)?;
        let err_text = err_task.await.unwrap_or_default();
        if status.success() {
            on_progress(1.0);
            Ok(())
        } else {
            let tail: Vec<&str> = err_text
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            Err(AppError::new(STAGE_FAILED, "dub.error.ffmpeg")
                .with_details(serde_json::json!({ "stderr": tail })))
        }
    }

    /// Quiet stretches of at least `min_secs` below `noise_db`, used to choose section cuts.
    pub async fn silences(
        &self,
        audio: &Path,
        noise_db: i32,
        min_secs: f32,
    ) -> Result<Vec<Silence>, AppError> {
        let filter = format!("silencedetect=noise={noise_db}dB:d={min_secs}");
        let out = Command::new(&self.ffmpeg)
            .args(["-v", "info", "-nostdin", "-i"])
            .arg(audio)
            .args(["-af", &filter, "-f", "null", "-"])
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(AppError::io)?;
        if !out.status.success() {
            return Err(AppError::new(STAGE_FAILED, "dub.error.ffmpeg"));
        }
        Ok(parse_silencedetect(&String::from_utf8_lossy(&out.stderr)))
    }
}

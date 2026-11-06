//! The post chain (research R4): ffmpeg turns a raw engine clip into the line clip with speed,
//! volume and pitch. It never calls an engine, so it costs milliseconds and no cloud characters.

use std::ffi::OsString;
use std::path::Path;

use crate::cloud::audio::{wav_duration_ms, wav_sample_rate};
use crate::error::AppError;
use crate::pipeline::ffmpeg::Ffmpeg;

/// Filter graph for the three settings. Pitch is a change of play rate (`asetrate`) put back to
/// the original rate; the tempo filter then brings the length back, so pitch does not change
/// how long the line takes. All of it is one `atempo`, which takes 0.5 to 2.0.
fn filters(rate: u32, speed: f64, volume_db: f64, pitch: f64) -> String {
    let factor = 2f64.powf(pitch / 12.0);
    let mut chain = Vec::new();
    if pitch.abs() > 1e-6 {
        chain.push(format!("asetrate={:.3}", rate as f64 * factor));
        chain.push(format!("aresample={rate}"));
    }
    let tempo = speed / factor;
    if (tempo - 1.0).abs() > 1e-6 {
        chain.push(format!("atempo={tempo:.6}"));
    }
    if volume_db.abs() > 1e-6 {
        chain.push(format!("volume={volume_db:.2}dB"));
    }
    if chain.is_empty() {
        chain.push("anull".into());
    }
    chain.join(",")
}

/// Make `out` from `raw`. The result is written beside `out` and renamed into place, so a
/// failed or stopped run leaves nothing half written.
pub async fn make_line_clip(
    ffmpeg: &Ffmpeg,
    raw: &Path,
    out: &Path,
    speed: f64,
    volume_db: f64,
    pitch_semitones: f64,
) -> Result<u64, AppError> {
    let rate = wav_sample_rate(raw).ok_or_else(|| {
        AppError::io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cannot read the clip {}", raw.display()),
        ))
    })?;
    let mut part = out.as_os_str().to_owned();
    part.push(".part");
    let part = std::path::PathBuf::from(part);
    let graph = filters(rate, speed, volume_db, pitch_semitones);
    let args: Vec<OsString> = vec![
        "-i".into(),
        raw.as_os_str().to_owned(),
        "-filter:a".into(),
        graph.into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        part.as_os_str().to_owned(),
    ];
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(OsString::as_os_str).collect();
    if let Err(e) = ffmpeg.run(&refs, 0, &mut |_| {}).await {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, out).map_err(AppError::io)?;
    Ok(wav_duration_ms(out).unwrap_or(0))
}

use std::path::{Path, PathBuf};

use super::services::Step;

/// Characters of text a cloud step sends per second of video. The transcript does not exist
/// before the run starts, so the amount shown beforehand is an estimate (research R6); the real
/// count is recorded after the run. Checked against the French fixtures in task T066.
pub const CHARS_PER_SECOND: u64 = 10;

/// About how many characters one cloud step will send for a video of this length.
pub fn estimate_chars(duration_ms: u64) -> u64 {
    duration_ms * CHARS_PER_SECOND / 1000
}

/// The amount as shown to the user: "about N" with N to the nearest hundred.
pub fn round_for_display(chars: u64) -> u64 {
    (chars + 50) / 100 * 100
}

/// Where a run that outgrew its agreement keeps the amount it projected, inside the work folder.
fn floor_path(work: &Path) -> PathBuf {
    work.join("cloud-needed.json")
}

/// The larger amount an earlier try of this dub projected for the step, or 0.
pub fn read_floor(work: &Path, step: Step, service_id: &str) -> u64 {
    std::fs::read(floor_path(work))
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<Floor>>(&b).ok())
        .and_then(|v| {
            v.into_iter()
                .find(|f| f.step == step && f.service_id == service_id)
        })
        .map(|f| f.chars)
        .unwrap_or(0)
}

/// Remember the projection, so the next preflight asks for the real amount and not the estimate.
pub fn write_floor(work: &Path, step: Step, service_id: &str, chars: u64) {
    let mut all: Vec<Floor> = std::fs::read(floor_path(work))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    all.retain(|f| !(f.step == step && f.service_id == service_id));
    all.push(Floor {
        step,
        service_id: service_id.to_string(),
        chars,
    });
    if let Ok(bytes) = serde_json::to_vec(&all) {
        let _ = crate::fsutil::write_atomic(&floor_path(work), &bytes);
    }
}

/// A fresh run starts from the estimate again.
pub fn clear_floor(work: &Path) {
    let _ = std::fs::remove_file(floor_path(work));
}

/// What a run has sent so far, scaled to the whole video: `sent` characters for the first
/// `done_ms` of `total_ms`. Never below `sent`.
pub fn project_chars(sent: u64, done_ms: u64, total_ms: u64) -> u64 {
    if done_ms == 0 {
        return sent;
    }
    (sent.saturating_mul(total_ms) / done_ms).max(sent)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Floor {
    step: Step,
    service_id: String,
    chars: u64,
}

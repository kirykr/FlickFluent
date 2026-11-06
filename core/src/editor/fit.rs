//! Smart Fit and End Sync (spec US2, research R4, R5). Smart Fit changes a line's speed, within
//! the safe range, so its speech fits its window; End Sync starts a clip earlier, into free time,
//! so it ends when its slot ends. Neither calls an engine: the new speed goes through the ffmpeg
//! post chain on the raw clip, and a new start only moves the clip in the mix.

use serde::{Deserialize, Serialize};

use super::limits::{NEIGHBOUR_MARGIN_MS, SPEED_MAX, SPEED_MIN};
use super::ops::EditOp;
use super::resynth::{resynthesize, ResynthEvent, Shared};
use crate::error::AppError;
use crate::pipeline::run::{window_ms, CancelToken, TOO_SHORT};
use crate::project::dub::{LineFlag, TranscriptLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FitMode {
    Smart,
    EndSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FitReason {
    /// The line needs a faster speed than the safe range allows: shorten the text or widen the slot.
    NeedsMoreThanLimit,
    /// Slowing down to the lowest safe speed still leaves the clip far shorter than its slot.
    SlowestReached,
    /// There is no free time before the line to start earlier.
    NoRoomBefore,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FitLine {
    pub id: String,
    pub speed_before: f64,
    pub speed_after: f64,
    pub dub_start_before: Option<u64>,
    pub dub_start_after: Option<u64>,
    pub fits: bool,
    pub reason: Option<FitReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FitReport {
    pub mode: FitMode,
    pub lines: Vec<FitLine>,
    pub fitted: usize,
    pub still_flagged: usize,
}

/// What a fit would do: the report, and the values to write.
pub struct FitPlan {
    pub report: FitReport,
    pub speeds: Vec<(String, f64)>,
    pub starts: Vec<(String, Option<u64>)>,
}

/// Aim a little past the line where a clip counts as too short, so rounding cannot leave it there.
const SHORT_MARGIN: f64 = 1.1;

fn ceil3(x: f64) -> f64 {
    (x * 1000.0 - 1e-6).ceil() / 1000.0
}

fn floor3(x: f64) -> f64 {
    (x * 1000.0 + 1e-6).floor() / 1000.0
}

fn wanted(lines: &[TranscriptLine], mode: FitMode, ids: Option<&[String]>) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| match ids {
            Some(ids) => ids.contains(&l.id),
            None => match mode {
                FitMode::Smart => {
                    l.flags.contains(&LineFlag::TooLong) || l.flags.contains(&LineFlag::TooShort)
                }
                FitMode::EndSync => true,
            },
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn plan(lines: &[TranscriptLine], mode: FitMode, ids: Option<&[String]>) -> FitPlan {
    match mode {
        FitMode::Smart => smart(lines, ids),
        FitMode::EndSync => end_sync(lines, ids),
    }
}

fn finish(
    mode: FitMode,
    rows: Vec<FitLine>,
    speeds: Vec<(String, f64)>,
    starts: Vec<(String, Option<u64>)>,
) -> FitPlan {
    let fitted = rows.iter().filter(|r| r.fits).count();
    FitPlan {
        report: FitReport {
            mode,
            still_flagged: rows.len() - fitted,
            fitted,
            lines: rows,
        },
        speeds,
        starts,
    }
}

fn smart(lines: &[TranscriptLine], ids: Option<&[String]>) -> FitPlan {
    let (mut rows, mut speeds) = (Vec::new(), Vec::new());
    for i in wanted(lines, FitMode::Smart, ids) {
        let l = &lines[i];
        let Some(raw) = l.raw_ms.or(l.audio_ms).filter(|ms| *ms > 0) else {
            continue;
        };
        let (raw, slot, window) = (
            raw as f64,
            l.end_ms.saturating_sub(l.start_ms).max(1) as f64,
            window_ms(lines, i).max(1) as f64,
        );
        let now = raw / l.speed;
        let (mut after, mut fits, mut reason) = (l.speed, true, None);
        if now > window {
            let need = ceil3(raw / window);
            if need > SPEED_MAX {
                (after, fits, reason) = (SPEED_MAX, false, Some(FitReason::NeedsMoreThanLimit));
            } else {
                after = need.max(SPEED_MIN);
            }
        } else if now < slot * TOO_SHORT {
            let slow = floor3(raw / (slot * TOO_SHORT * SHORT_MARGIN));
            if slow < SPEED_MIN {
                (after, fits, reason) = (
                    SPEED_MIN,
                    raw / SPEED_MIN >= slot * TOO_SHORT * 1.02,
                    Some(FitReason::SlowestReached),
                );
                if fits {
                    reason = None;
                }
            } else {
                after = slow.min(1.0);
            }
        }
        if (after - l.speed).abs() > 1e-9 {
            speeds.push((l.id.clone(), after));
        }
        rows.push(FitLine {
            id: l.id.clone(),
            speed_before: l.speed,
            speed_after: after,
            dub_start_before: l.dub_start_ms,
            dub_start_after: l.dub_start_ms,
            fits,
            reason,
        });
    }
    finish(FitMode::Smart, rows, speeds, Vec::new())
}

fn end_sync(lines: &[TranscriptLine], ids: Option<&[String]>) -> FitPlan {
    let (mut rows, mut starts) = (Vec::new(), Vec::new());
    // Starts chosen so far, so a later line sees where the one before it now begins.
    let mut moved: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for i in wanted(lines, FitMode::EndSync, ids) {
        let l = &lines[i];
        let Some(clip) = l.audio_ms else {
            continue;
        };
        let clip_end = |p: &TranscriptLine| {
            let from = moved
                .get(p.id.as_str())
                .copied()
                .or(p.dub_start_ms)
                .unwrap_or(p.start_ms);
            from + p.audio_ms.unwrap_or(p.end_ms.saturating_sub(p.start_ms))
        };
        let earliest = i
            .checked_sub(1)
            .map(|p| clip_end(&lines[p]) + NEIGHBOUR_MARGIN_MS)
            .unwrap_or(0);
        let ideal = l.end_ms.saturating_sub(clip);
        let (mut after, mut fits, mut reason) = (l.dub_start_ms, true, None);
        if ideal < l.start_ms {
            let target = ideal.max(earliest);
            if target >= l.start_ms {
                (fits, reason) = (false, Some(FitReason::NoRoomBefore));
            } else {
                after = Some(target);
                if target != ideal {
                    (fits, reason) = (false, Some(FitReason::NoRoomBefore));
                }
                moved.insert(l.id.as_str(), target);
                starts.push((l.id.clone(), after));
            }
        }
        rows.push(FitLine {
            id: l.id.clone(),
            speed_before: l.speed,
            speed_after: l.speed,
            dub_start_before: l.dub_start_ms,
            dub_start_after: after,
            fits,
            reason,
        });
    }
    finish(FitMode::EndSync, rows, Vec::new(), starts)
}

/// Run a fit: write the new speeds or starts as one undo step, make the clips with the post
/// chain, and rebuild the mix. Returns what was done, judged on the lines as they are afterwards.
pub async fn fit(
    shared: &Shared,
    mode: FitMode,
    ids: Option<Vec<String>>,
    cancel: CancelToken,
    on_event: &mut (dyn FnMut(ResynthEvent) + Send),
) -> Result<FitReport, AppError> {
    let plan = {
        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
        let plan = self::plan(&s.dub().lines, mode, ids.as_deref());
        if !plan.speeds.is_empty() || !plan.starts.is_empty() {
            s.apply(EditOp::ApplyFit {
                speeds: plan.speeds.clone(),
                starts: plan.starts.clone(),
            })?;
        }
        plan
    };
    let changed: Vec<String> = plan
        .speeds
        .iter()
        .map(|(id, _)| id.clone())
        .chain(plan.starts.iter().map(|(id, _)| id.clone()))
        .collect();
    // The post chain for new speeds; for new starts only the mix is rebuilt.
    resynthesize(shared, Some(changed), cancel, on_event).await?;

    let mut report = plan.report;
    let s = shared.lock().unwrap_or_else(|e| e.into_inner());
    for row in &mut report.lines {
        if let Some(l) = s.dub().lines.iter().find(|l| l.id == row.id) {
            row.fits = row.fits
                && !l.flags.contains(&LineFlag::TooLong)
                && !l.flags.contains(&LineFlag::TooShort);
            row.speed_after = l.speed;
        }
    }
    report.fitted = report.lines.iter().filter(|r| r.fits).count();
    report.still_flagged = report.lines.len() - report.fitted;
    Ok(report)
}

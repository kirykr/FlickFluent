//! Smart Fit and End Sync (spec US2): speed within the safe range so speech fits its window, and
//! starts moved earlier into free time. No engine and no cloud request is ever made.

mod support;

use flickfluent_core::editor::fit::{fit, FitMode, FitReason};
use flickfluent_core::editor::limits::{SPEED_MAX, SPEED_MIN};
use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::pipeline::run::{window_ms, CancelToken};
use flickfluent_core::project::dub::LineFlag;
use serde_json::json;
use support::editor_fixture::Live;

async fn run(
    live: &Live,
    mode: FitMode,
    ids: Option<Vec<String>>,
) -> flickfluent_core::editor::fit::FitReport {
    fit(&live.shared, mode, ids, CancelToken::default(), &mut |_| {})
        .await
        .unwrap()
}

fn set_end(live: &Live, id: &str, end_ms: u64) {
    live.with(|s| {
        s.apply(EditOp::UpdateLine {
            id: id.into(),
            patch: LinePatch {
                end_ms: Some(end_ms),
                ..Default::default()
            },
        })
        .unwrap()
    });
}

/// Shorten line `i` until speeding it up by about `want` would make it fit its window.
fn squeeze(live: &Live, i: usize, want: f64) -> String {
    let line = live.lines()[i].clone();
    let raw = line.raw_ms.unwrap() as f64;
    for slot in (60..4000u64).rev().step_by(10) {
        set_end(live, &line.id, line.start_ms + slot);
        let lines = live.lines();
        let w = window_ms(&lines, i) as f64;
        if raw / w >= want {
            return line.id;
        }
    }
    panic!("could not squeeze line {i}");
}

#[tokio::test]
async fn smart_fit_raises_the_speed_until_the_line_fits_and_clears_the_flag() {
    let live = Live::finished(json!({})).await;
    let id = squeeze(&live, 2, 1.27);
    let flagged = live.lines()[2].clone();
    assert!(
        flagged.flags.contains(&LineFlag::TooLong),
        "{:?}",
        flagged.flags
    );

    let report = run(&live, FitMode::Smart, Some(vec![id.clone()])).await;
    assert_eq!((report.fitted, report.still_flagged), (1, 0));
    let r = &report.lines[0];
    assert!(r.fits && r.reason.is_none());
    assert!(
        r.speed_after > 1.0 && r.speed_after <= SPEED_MAX,
        "{}",
        r.speed_after
    );

    let line = live.lines()[2].clone();
    assert_eq!(line.speed, r.speed_after);
    assert!(!line.flags.contains(&LineFlag::TooLong));
    let window = window_ms(&live.lines(), 2) as f64;
    assert!(
        line.audio_ms.unwrap() as f64 <= window * 1.02,
        "{} against {window}",
        line.audio_ms.unwrap()
    );
    assert!(
        line.raw_ms.unwrap().abs_diff(flagged.raw_ms.unwrap()) <= 2,
        "the raw clip is kept"
    );
    assert!(live.requests().is_empty(), "no engine call");
    assert!(live.with(|s| s.changed_ids()).is_empty());
}

#[tokio::test]
async fn a_line_that_needs_more_than_the_limit_stops_at_it_and_says_why() {
    let live = Live::finished(json!({})).await;
    let id = squeeze(&live, 3, 2.0);
    let report = run(&live, FitMode::Smart, Some(vec![id])).await;
    assert_eq!((report.fitted, report.still_flagged), (0, 1));
    let r = &report.lines[0];
    assert_eq!(r.speed_after, SPEED_MAX);
    assert!(!r.fits);
    assert_eq!(r.reason, Some(FitReason::NeedsMoreThanLimit));
    assert!(
        live.lines()[3].flags.contains(&LineFlag::TooLong),
        "still flagged"
    );
}

#[tokio::test]
async fn a_clip_far_shorter_than_its_slot_is_slowed_but_never_below_the_limit() {
    let live = Live::finished(json!({})).await;
    let line = live.lines()[1].clone();
    // A slot twice the clip: tooShort, and 0.9 is enough to fix it.
    set_end(
        &live,
        &line.id,
        line.start_ms + line.audio_ms.unwrap() * 2 + 100,
    );
    assert!(live.lines()[1].flags.contains(&LineFlag::TooShort));
    let report = run(&live, FitMode::Smart, Some(vec![line.id.clone()])).await;
    let r = &report.lines[0];
    assert!(
        r.speed_after < 1.0 && r.speed_after >= SPEED_MIN,
        "{}",
        r.speed_after
    );
    assert!(r.fits);
    assert!(!live.lines()[1].flags.contains(&LineFlag::TooShort));

    // A slot ten times the clip cannot be fixed by slowing down within the limit.
    let other = live.lines()[4].clone();
    set_end(
        &live,
        &other.id,
        other.start_ms + other.audio_ms.unwrap() * 10,
    );
    let report = run(&live, FitMode::Smart, Some(vec![other.id.clone()])).await;
    assert_eq!(report.lines[0].speed_after, SPEED_MIN);
    assert!(!report.lines[0].fits);
    assert!(report.lines[0].reason.is_some());
}

#[tokio::test]
async fn a_clip_already_inside_its_slot_is_left_alone() {
    let live = Live::finished(json!({})).await;
    let before = live.clip_files();
    let id = live.lines()[0].id.clone();
    let report = run(&live, FitMode::Smart, Some(vec![id])).await;
    assert_eq!(report.lines[0].speed_after, 1.0);
    assert!(report.lines[0].fits);
    assert_eq!(live.clip_files(), before, "no clip file changed");
}

#[tokio::test]
async fn without_ids_smart_fit_takes_every_flagged_line_and_counts_them() {
    let live = Live::finished(json!({})).await;
    squeeze(&live, 2, 1.27);
    squeeze(&live, 3, 1.28);
    squeeze(&live, 5, 2.0);
    let report = run(&live, FitMode::Smart, None).await;
    assert_eq!(report.lines.len(), 3);
    assert_eq!((report.fitted, report.still_flagged), (2, 1));
    // One undo step puts all the speeds back.
    live.with(|s| s.undo().unwrap());
    assert!(live.lines().iter().all(|l| l.speed == 1.0));
}

// --- End Sync ------------------------------------------------------------------------------

#[tokio::test]
async fn end_sync_moves_the_start_earlier_so_the_clip_ends_with_its_slot() {
    let live = Live::finished(json!({})).await;
    // Make line 4's clip longer than its slot: shorten the slot only.
    let id = live.lines()[4].id.clone();
    let line = live.lines()[4].clone();
    set_end(&live, &id, line.start_ms + 500);
    let mix_before = std::fs::read(live.work.join("dub-mix.wav")).unwrap();
    let report = run(&live, FitMode::EndSync, Some(vec![id.clone()])).await;
    assert_ne!(
        std::fs::read(live.work.join("dub-mix.wav")).unwrap(),
        mix_before,
        "the mix places the clip at its new start"
    );
    assert!(live.with(|s| s
        .dub()
        .run
        .clone()
        .unwrap()
        .sections
        .iter()
        .all(|x| !x.mix_stale)));
    let after = live.lines()[4].clone();
    let prev = live.lines()[3].clone();
    let prev_end = prev.dub_start_ms.unwrap_or(prev.start_ms) + prev.audio_ms.unwrap();
    let clip = after.audio_ms.unwrap();
    let ideal = (line.start_ms + 500).saturating_sub(clip);
    let want = ideal.max(prev_end + 80);
    assert_eq!(after.dub_start_ms, Some(want.min(line.start_ms)));
    assert_eq!(
        (after.start_ms, after.end_ms),
        (line.start_ms, line.start_ms + 500),
        "times untouched"
    );
    assert!(after.dub_start_ms.unwrap() < line.start_ms);
    assert!(
        after.dub_start_ms.unwrap() >= prev_end,
        "never over the line before"
    );
    let r = &report.lines[0];
    assert_eq!(r.dub_start_after, after.dub_start_ms);
    assert!(live.requests().is_empty());
}

#[tokio::test]
async fn end_sync_reports_a_line_with_no_free_time_and_does_not_move_it() {
    let live = Live::finished(json!({})).await;
    // Line 1 starts right after line 0's clip: no room to go earlier.
    let l0 = live.lines()[0].clone();
    let l1 = live.lines()[1].clone();
    let l0_end = l0.start_ms + l0.audio_ms.unwrap();
    live.with(|s| {
        s.apply(EditOp::UpdateLine {
            id: l1.id.clone(),
            patch: LinePatch {
                start_ms: Some(l0_end + 10),
                end_ms: Some(l0_end + 410),
                ..Default::default()
            },
        })
        .unwrap()
    });
    let report = run(&live, FitMode::EndSync, Some(vec![l1.id.clone()])).await;
    let r = &report.lines[0];
    assert_eq!(r.reason, Some(FitReason::NoRoomBefore));
    assert!(!r.fits);
    assert_eq!(live.lines()[1].dub_start_ms, None, "not moved");
}

#[tokio::test]
async fn end_sync_leaves_a_clip_that_already_ends_in_its_slot_where_it_is() {
    let live = Live::finished(json!({})).await;
    let line = live.lines()[2].clone();
    set_end(
        &live,
        &line.id,
        line.start_ms + line.audio_ms.unwrap() + 100,
    );
    let report = run(&live, FitMode::EndSync, Some(vec![line.id])).await;
    assert!(report.lines[0].fits);
    assert_eq!(live.lines()[2].dub_start_ms, None);
}

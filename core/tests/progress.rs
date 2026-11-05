//! `dub:progress` pacing and the ETA. Pacer and ETA are checked with made-up clocks; the run is
//! checked with the fake sidecar and the real clock.

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flickfluent_core::pipeline::progress::{Eta, Pacer};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunEvent};
use flickfluent_core::project::dub::{RunState, Stage, StageStatus};
use serde_json::json;
use support::run_fixture::{config, dub_for, long_clip};

#[test]
fn pacer_lets_progress_through_once_a_second() {
    let t0 = Instant::now();
    let mut p = Pacer::default();
    assert!(p.progress_due(t0));
    assert!(!p.progress_due(t0 + Duration::from_millis(400)));
    assert!(!p.progress_due(t0 + Duration::from_millis(999)));
    assert!(p.progress_due(t0 + Duration::from_millis(1000)));
    assert!(!p.progress_due(t0 + Duration::from_millis(1500)));
}

#[test]
fn pacer_sends_a_heartbeat_inside_five_seconds_of_silence() {
    let t0 = Instant::now();
    let mut p = Pacer::default();
    assert!(p.progress_due(t0));
    assert!(!p.heartbeat_due(t0 + Duration::from_secs(2)));
    assert!(!p.heartbeat_due(t0 + Duration::from_millis(3900)));
    assert!(p.heartbeat_due(t0 + Duration::from_secs(4)));
    // The heartbeat counts as sent, so real progress right after it waits its turn.
    assert!(!p.progress_due(t0 + Duration::from_millis(4300)));
}

#[test]
fn eta_is_remaining_sections_times_the_mean_of_finished_ones() {
    let mut eta = Eta::default();
    assert_eq!(
        eta.seconds(10),
        None,
        "no estimate before a section is done"
    );
    eta.section_done(Duration::from_secs(20));
    assert_eq!(eta.seconds(10), Some(9 * 20));
    eta.section_done(Duration::from_secs(40));
    assert_eq!(eta.seconds(10), Some(8 * 30));
    assert_eq!(
        eta.seconds(2),
        Some(0),
        "nothing left is zero, not negative"
    );
}

#[test]
fn eta_follows_only_the_latest_sections() {
    let mut eta = Eta::default();
    for _ in 0..5 {
        eta.section_done(Duration::from_secs(100));
    }
    for _ in 0..5 {
        eta.section_done(Duration::from_secs(10));
    }
    // The window holds the last five, all 10 s.
    assert_eq!(eta.seconds(20), Some(10 * 10));
}

#[test]
fn eta_counts_sections_finished_before_a_resume() {
    let mut eta = Eta::default();
    eta.already_done(3);
    eta.section_done(Duration::from_secs(10));
    assert_eq!(eta.seconds(6), Some(2 * 10));
}

type Log = Arc<Mutex<Vec<(Instant, RunEvent)>>>;

async fn run_logged(slow: serde_json::Value, minutes: u32) -> Log {
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), minutes);
    let cfg = config(root.path(), "p.flick", slow);
    let dub = dub_for(&source, "en").await;
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let sink = log.clone();
    let out = run_dub(
        &cfg,
        dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| sink.lock().unwrap().push((Instant::now(), e)),
    )
    .await
    .unwrap();
    assert_eq!(out.run.unwrap().state, RunState::Completed);
    log
}

fn progress_times(log: &Log) -> Vec<Instant> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, e)| matches!(e, RunEvent::Progress { .. }))
        .map(|(t, _)| *t)
        .collect()
}

#[tokio::test]
async fn progress_never_comes_faster_than_once_a_second() {
    let log = run_logged(json!({}), 6).await;
    let times = progress_times(&log);
    assert!(times.len() >= 2, "a run reports progress");
    for w in times.windows(2) {
        // A few milliseconds of slack for the callback's own scheduling.
        assert!(
            w[1] - w[0] >= Duration::from_millis(980),
            "two progress events {:?} apart",
            w[1] - w[0]
        );
    }
}

#[tokio::test]
async fn a_silent_stage_still_reports_every_five_seconds() {
    // Separate in section 0001 lingers 11 s without a word.
    let slow = json!({ "slowWorker": "separate", "slowSection": "0001", "slowSeconds": 11 });
    let log = run_logged(slow, 12).await;
    let times = progress_times(&log);
    let gaps: Vec<Duration> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let longest = gaps.iter().max().copied().unwrap();
    assert!(
        longest <= Duration::from_millis(5200),
        "longest silence {longest:?}"
    );
    assert!(
        gaps.iter().any(|g| *g >= Duration::from_secs(3)),
        "the stall was covered by heartbeats, not by real progress: {gaps:?}"
    );
}

#[tokio::test]
async fn events_arrive_while_a_worker_runs_not_after_it() {
    let slow = json!({ "slowWorker": "separate", "slowSection": "0000", "slowSeconds": 6 });
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 6);
    let cfg = config(root.path(), "live.flick", slow);
    let dub = dub_for(&source, "en").await;
    let started = Instant::now();
    let first_during = Arc::new(Mutex::new(None::<Duration>));
    let seen = first_during.clone();
    run_dub(
        &cfg,
        dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| {
            if matches!(
                e,
                RunEvent::Progress {
                    stage: Stage::Separate,
                    ..
                }
            ) {
                seen.lock().unwrap().get_or_insert(started.elapsed());
            }
        },
    )
    .await
    .unwrap();
    let first = first_during.lock().unwrap().unwrap();
    // The worker holds section 0 for 6 s; a heartbeat must land well before it ends.
    assert!(
        first < Duration::from_secs(8),
        "first separate event after {first:?}"
    );
}

#[tokio::test]
async fn stage_events_fire_on_each_change_and_never_repeat() {
    let log = run_logged(json!({}), 6).await;
    let events = log.lock().unwrap();
    let mut last: std::collections::BTreeMap<Stage, StageStatus> = Default::default();
    let mut count = 0;
    for (_, e) in events.iter() {
        if let RunEvent::Stage { stage, status } = e {
            assert_ne!(
                last.get(stage),
                Some(status),
                "{stage:?} {status:?} sent twice in a row"
            );
            last.insert(*stage, *status);
            count += 1;
        }
    }
    assert!(count >= 6);
    assert_eq!(last.get(&Stage::Mix), Some(&StageStatus::Done));
}

#[tokio::test]
async fn an_eta_shows_once_a_section_is_done_and_does_not_grow() {
    // Section 1 lingers 3 s, so an event leaves while section 0 is finished and 1 is running.
    let slow = json!({ "slowWorker": "separate", "slowSection": "0001", "slowSeconds": 3 });
    let log = run_logged(slow, 8).await;
    let etas: Vec<Option<u64>> = log
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(_, e)| match e {
            RunEvent::Progress {
                eta_seconds,
                section_count,
                ..
            } if *section_count > 0 => Some(*eta_seconds),
            _ => None,
        })
        .collect();
    let known: Vec<u64> = etas.iter().flatten().copied().collect();
    assert!(!known.is_empty(), "an ETA shows once a section is done");
    assert!(
        known.last().unwrap() <= known.first().unwrap(),
        "the estimate does not grow: {known:?}"
    );
    let _ = Path::new("");
}

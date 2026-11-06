//! Re-synthesis of just the changed lines (spec FR-005, SC-001, SC-006): only those lines reach
//! the engine, each clip is swapped in when complete, and the mix is rebuilt for the touched
//! sections only. Runs against the fake sidecar and the fake cloud server.

mod support;

use std::sync::{Arc, Mutex};

use flickfluent_core::cloud::audio::wav_duration_ms;
use flickfluent_core::cloud::services::Step;
use flickfluent_core::editor::ops::EditOp;
use flickfluent_core::editor::resynth::{resynthesize, ResynthEvent, ResynthState};
use flickfluent_core::error::{CLOUD_CONSENT_REQUIRED, EDIT_WORK_FOLDER_MISSING};
use flickfluent_core::pipeline::run::CancelToken;
use serde_json::json;
use support::cloud_run::cloud;
use support::editor_fixture::Live;
use support::fake_cloud::FakeCloud;

const KEY: &str = "resynth-key-MARKER-0001";

async fn go(live: &Live, ids: Option<Vec<&str>>) -> flickfluent_core::editor::resynth::Resynth {
    resynthesize(
        &live.shared,
        ids.map(|v| v.into_iter().map(String::from).collect()),
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn only_the_changed_line_reaches_the_engine_and_every_other_clip_keeps_its_bytes() {
    let live = Live::finished(json!({})).await;
    let lines = live.lines();
    assert!(
        lines.len() >= 5,
        "the fixture has lines enough: {}",
        lines.len()
    );
    let target = lines[3].id.clone();
    let before = live.clip_files();
    let mix_before = std::fs::read(live.work.join("dub-mix.wav")).unwrap();
    let old_audio = lines[3].audio.clone().unwrap();

    live.edit_text(&target, "A brand new sentence");
    assert_eq!(live.with(|s| s.changed_ids()), vec![target.clone()]);
    let out = go(&live, None).await;

    assert_eq!(out.state, ResynthState::Completed);
    assert!(out.remaining_changed_ids.is_empty());
    assert_eq!(
        live.requests(),
        vec![vec![target.clone()]],
        "one request, one line"
    );

    let after = live.clip_files();
    for (path, bytes) in &before {
        if *path == old_audio {
            assert!(
                !after.contains_key(path),
                "the old clip of the edited line is removed"
            );
        } else {
            assert_eq!(after.get(path), Some(bytes), "{path} keeps its bytes");
        }
    }
    let new: Vec<_> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    assert_eq!(new.len(), 1, "exactly one new clip file: {new:?}");

    let line = live.lines().into_iter().find(|l| l.id == target).unwrap();
    assert_eq!(line.audio.as_deref(), Some(new[0].as_str()));
    assert_eq!(
        line.raw_audio, line.audio,
        "neutral speed: the raw clip is the line clip"
    );
    assert!(!line.speech_hash.is_empty());
    assert!(live.with(|s| s.changed_ids()).is_empty());
    assert_eq!(
        live.with(|s| s
            .dub()
            .run
            .clone()
            .unwrap()
            .sections
            .iter()
            .filter(|s| s.mix_stale)
            .count()),
        0
    );

    let mix_after = std::fs::read(live.work.join("dub-mix.wav")).unwrap();
    assert_ne!(mix_after, mix_before, "the mix has the new speech");
    assert_eq!(
        wav_duration_ms(&live.work.join("dub-mix.wav")),
        {
            std::fs::write(live.work.join("probe.wav"), &mix_before).unwrap();
            wav_duration_ms(&live.work.join("probe.wav"))
        },
        "same length"
    );
}

#[tokio::test]
async fn a_deleted_line_loses_its_speech_without_any_engine_call() {
    let live = Live::finished(json!({})).await;
    let victim = live.lines()[2].id.clone();
    let mix_before = std::fs::read(live.work.join("dub-mix.wav")).unwrap();
    live.with(|s| {
        s.apply(EditOp::DeleteLines {
            ids: vec![victim.clone()],
        })
        .unwrap()
    });
    assert!(live.with(|s| s
        .dub()
        .run
        .clone()
        .unwrap()
        .sections
        .iter()
        .any(|s| s.mix_stale)));

    let out = go(&live, None).await;
    assert_eq!(out.state, ResynthState::Completed);
    assert!(live.requests().is_empty(), "nothing is regenerated");
    assert_ne!(
        std::fs::read(live.work.join("dub-mix.wav")).unwrap(),
        mix_before
    );
    assert!(live.with(|s| s
        .dub()
        .run
        .clone()
        .unwrap()
        .sections
        .iter()
        .all(|s| !s.mix_stale)));
}

#[tokio::test]
async fn an_inserted_line_gets_speech_at_its_times() {
    let live = Live::finished(json!({})).await;
    let last = live.lines().last().cloned().unwrap();
    let start = last.end_ms + 600;
    let r = live.with(|s| {
        s.apply(EditOp::InsertLine {
            after_id: None,
            start_ms: start,
            end_ms: start + 2_000,
            speaker_id: last.speaker_id.clone(),
            translated: "Something new to say".into(),
        })
        .unwrap()
    });
    let id = r.lines[0].id.clone();
    assert_eq!(live.with(|s| s.changed_ids()), vec![id.clone()]);

    let out = go(&live, None).await;
    assert_eq!(out.state, ResynthState::Completed);
    assert_eq!(live.requests(), vec![vec![id.clone()]]);
    let line = live.lines().into_iter().find(|l| l.id == id).unwrap();
    assert!(line.audio.is_some() && line.audio_ms.unwrap() > 0);
    assert_eq!((line.start_ms, line.end_ms), (start, start + 2_000));
    assert!(live.with(|s| s.changed_ids()).is_empty());
}

#[tokio::test]
async fn nothing_changed_means_nothing_is_sent_and_nothing_is_written() {
    let live = Live::finished(json!({})).await;
    let before = live.clip_files();
    let mix = std::fs::read(live.work.join("dub-mix.wav")).unwrap();
    let out = go(&live, None).await;
    assert_eq!(out.state, ResynthState::Completed);
    assert!(live.requests().is_empty());
    assert_eq!(live.clip_files(), before);
    assert_eq!(std::fs::read(live.work.join("dub-mix.wav")).unwrap(), mix);
}

#[tokio::test]
async fn without_the_work_folder_synthesis_says_so_and_text_edits_still_work() {
    let live = Live::finished(json!({})).await;
    let id = live.lines()[0].id.clone();
    std::fs::remove_dir_all(&live.work).unwrap();
    live.edit_text(&id, "Still editable");
    let err = resynthesize(&live.shared, None, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, EDIT_WORK_FOLDER_MISSING);
    assert_eq!(live.with(|s| s.changed_ids()), vec![id]);
}

// --- stopping, edits during a run, cloud ---------------------------------------------------

fn edit_five(live: &Live) -> Vec<String> {
    let ids: Vec<String> = live.lines().iter().take(5).map(|l| l.id.clone()).collect();
    for (n, id) in ids.iter().enumerate() {
        live.edit_text(id, &format!("Edited line number {n}"));
    }
    ids
}

#[tokio::test]
async fn a_stop_keeps_the_swapped_clips_and_the_next_call_does_the_rest() {
    let live = Live::finished(json!({ "lineDelayMs": 500 })).await;
    let ids = edit_five(&live);
    let cancel = CancelToken::default();
    let trigger = cancel.clone();
    let done = Arc::new(Mutex::new(Vec::new()));
    let seen = done.clone();
    let out = resynthesize(&live.shared, None, cancel, &mut move |e| {
        if let ResynthEvent::LineDone(l) = e {
            seen.lock().unwrap().push(l.id);
            if seen.lock().unwrap().len() == 2 {
                trigger.cancel();
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(out.state, ResynthState::Stopped);
    let swapped = done.lock().unwrap().clone();
    assert_eq!(swapped.len(), 2, "{swapped:?}");
    assert_eq!(out.remaining_changed_ids.len(), 3);
    assert_eq!(live.with(|s| s.changed_ids()), out.remaining_changed_ids);
    for id in &swapped {
        assert!(!out.remaining_changed_ids.contains(id));
        assert!(ids.contains(id));
    }
    // The swapped clips are in the mix: no section is left stale.
    assert!(live.with(|s| s
        .dub()
        .run
        .clone()
        .unwrap()
        .sections
        .iter()
        .all(|s| !s.mix_stale)));

    let before = live.requests().len();
    let rest = go(&live, None).await;
    assert_eq!(rest.state, ResynthState::Completed);
    let second: Vec<String> = live.requests()[before..].concat();
    let mut want = out.remaining_changed_ids.clone();
    want.sort();
    let mut got = second.clone();
    got.sort();
    assert_eq!(got, want, "exactly the three that were left");
    assert!(live.with(|s| s.changed_ids()).is_empty());
}

#[tokio::test]
async fn an_edit_during_a_run_leaves_that_line_changed() {
    let live = Live::finished(json!({ "lineDelayMs": 400 })).await;
    let ids = edit_five(&live);
    let shared = live.shared.clone();
    let victim = ids[4].clone();
    let touched = Arc::new(Mutex::new(false));
    let flag = touched.clone();
    let victim_edit = victim.clone();
    let out = resynthesize(&live.shared, None, CancelToken::default(), &mut move |e| {
        if matches!(e, ResynthEvent::LineDone(_)) && !*flag.lock().unwrap() {
            *flag.lock().unwrap() = true;
            // The window thread edits a line the run has not reached yet.
            shared
                .lock()
                .unwrap()
                .apply(EditOp::UpdateLine {
                    id: victim_edit.clone(),
                    patch: flickfluent_core::editor::ops::LinePatch {
                        translated: Some("Changed while the run was going".into()),
                        ..Default::default()
                    },
                })
                .unwrap();
        }
    })
    .await
    .unwrap();

    assert_eq!(out.state, ResynthState::Completed);
    assert_eq!(
        out.remaining_changed_ids,
        vec![victim.clone()],
        "its clip was made from the old text"
    );
    let line = live.lines().into_iter().find(|l| l.id == victim).unwrap();
    assert_eq!(
        line.translated.as_deref(),
        Some("Changed while the run was going")
    );
    assert_eq!(live.with(|s| s.changed_ids()), vec![victim]);
}

#[tokio::test]
async fn cloud_lines_pass_the_shared_guard_and_the_usage_is_recorded() {
    let appdata = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(
        appdata.path(),
        &fake,
        &[("google-gemini", "mine", KEY, None)],
    );
    c.consent.grant("google-gemini", Step::Speak, 20).unwrap();
    let live = Live::finished_with(json!({}), Some(c.runtime.clone()), Some("google-gemini")).await;
    let id = live.lines()[0].id.clone();
    live.edit_text(&id, "Hello there"); // 11 characters

    let out = go(&live, Some(vec![id.as_str()])).await;
    assert_eq!(out.state, ResynthState::Completed);
    assert!(live.requests().is_empty(), "the local engine is not used");
    assert_eq!(fake.count(), 1, "one request for one line");
    let usage = live.with(|s| s.dub().cloud_usage.clone());
    let speak = usage.iter().find(|u| u.step == "speak").unwrap();
    assert_eq!(speak.chars, 11);
    let line = live.lines().into_iter().find(|l| l.id == id).unwrap();
    assert!(line.audio.is_some());
}

#[tokio::test]
async fn an_editing_session_that_outgrows_the_agreement_stops_before_sending() {
    let appdata = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(
        appdata.path(),
        &fake,
        &[("google-gemini", "mine", KEY, None)],
    );
    c.consent.grant("google-gemini", Step::Speak, 2).unwrap(); // up to 10 characters in all
    let live = Live::finished_with(json!({}), Some(c.runtime.clone()), Some("google-gemini")).await;
    let ids: Vec<String> = live.lines().iter().take(3).map(|l| l.id.clone()).collect();
    for id in &ids {
        live.edit_text(id, "A long sentence of text");
    }
    let err = resynthesize(
        &live.shared,
        Some(ids.clone()),
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, CLOUD_CONSENT_REQUIRED);
    assert!(err.details.unwrap()["projectedChars"].as_u64().unwrap() > 10);
    assert_eq!(fake.count(), 0, "nothing was sent");
    assert!(live.with(|s| s.changed_ids()).len() >= 3);
}

#[tokio::test]
async fn progress_keeps_coming_while_a_slow_engine_works() {
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    flickfluent_core::editor::resynth::HEARTBEAT_MS.store(300, Ordering::Relaxed);
    let live = Live::finished(json!({ "lineDelayMs": 1200 })).await;
    for id in live.lines().iter().take(2).map(|l| l.id.clone()) {
        live.edit_text(&id, "Said slowly");
    }
    let times = Arc::new(Mutex::new(vec![Instant::now()]));
    let seen = times.clone();
    let out = resynthesize(&live.shared, None, CancelToken::default(), &mut move |e| {
        if matches!(e, ResynthEvent::Progress { .. }) {
            seen.lock().unwrap().push(Instant::now());
        }
    })
    .await
    .unwrap();
    flickfluent_core::editor::resynth::HEARTBEAT_MS.store(4000, Ordering::Relaxed);
    assert_eq!(out.state, ResynthState::Completed);
    let t = times.lock().unwrap().clone();
    assert!(t.len() >= 5, "heartbeats between lines: {}", t.len());
    let worst = t.windows(2).map(|w| w[1] - w[0]).max().unwrap();
    assert!(worst.as_millis() < 900, "longest gap {worst:?}");
}

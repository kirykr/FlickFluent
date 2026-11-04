//! A run completes, can be cancelled or killed in any stage, and resumes redoing only the section
//! that was in progress. Uses the fake sidecar, so it tests the core, not the models.

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flickfluent_core::error::{NOTHING_TO_RESUME, NO_SPEECH_FOUND, SOURCE_CHANGED};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunEvent};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{RunState, Stage, StageStatus};
use serde_json::json;
use support::run_fixture::{config, dub_for, long_clip};

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

/// Modified times of every file under the section folders.
fn stamps(work: &Path) -> BTreeMap<String, std::time::SystemTime> {
    let mut out = BTreeMap::new();
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, std::time::SystemTime>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.insert(
                    p.strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    e.metadata().unwrap().modified().unwrap(),
                );
            }
        }
    }
    walk(&work.join("sections"), work, &mut out);
    out
}

#[tokio::test]
async fn a_full_run_completes_with_lines_a_mix_and_events() {
    let root = tempfile::tempdir().unwrap();
    let cfg = config(root.path(), "a.flick", json!({}));
    let dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let out = run_dub(
        &cfg,
        dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| sink.lock().unwrap().push(e),
    )
    .await
    .unwrap();

    let run = out.run.as_ref().unwrap();
    assert_eq!(run.state, RunState::Completed);
    assert!(!out.lines.is_empty());
    for l in &out.lines {
        assert!(l.end_ms > l.start_ms && l.translated.as_deref().unwrap().starts_with("T:"));
        assert!(l.audio_ms.is_some() && l.audio.is_some());
    }
    assert!(out.lines.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    let work = WorkFolder::folder_for(&cfg.project_path);
    assert!(work.join("dub-mix.wav").exists());
    assert!(!walk_any(&work, "part"), "no .part files left");
    let events = events.lock().unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::Done(RunState::Completed))));
    assert!(events.iter().any(|e| matches!(e, RunEvent::Line(_))));
    for stage in [
        Stage::Extract,
        Stage::Transcribe,
        Stage::Synthesize,
        Stage::Mix,
    ] {
        assert!(events.iter().any(|e| matches!(e, RunEvent::Stage { stage: s, status: StageStatus::Done } if *s == stage)), "{stage:?}");
    }
    // Speakers got a male and a female voice choice from the fake gender.
    assert_eq!(out.speakers.len(), 2);
}

fn walk_any(dir: &Path, ext: &str) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let p = e.path();
            if p.is_dir() {
                walk_any(&p, ext)
            } else {
                p.extension().is_some_and(|x| x == ext)
            }
        })
}

/// Kill the run (drop it) while `worker` lingers in section 0001, then resume.
async fn kill_then_resume(worker: &str, stage: Stage) {
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 12);
    let slow = json!({ "slowWorker": worker, "slowSection": "0001", "slowSeconds": 60 });
    let cfg = config(root.path(), "k.flick", slow);
    let dub = dub_for(&source, "en").await;

    let reached = Arc::new(AtomicBool::new(false));
    let flag = reached.clone();
    let cfg = Arc::new(cfg);
    let task_cfg = cfg.clone();
    let task_dub = dub.clone();
    let seen_section = Arc::new(Mutex::new(0u32));
    let seen = seen_section.clone();
    let handle = tokio::spawn(async move {
        run_dub(
            &task_cfg,
            task_dub,
            Mode::Fresh,
            CancelToken::default(),
            &mut move |e| {
                if let RunEvent::Progress { section_index, .. } = &e {
                    *seen.lock().unwrap() = *section_index;
                }
                let _ = &e;
                flag.store(true, Ordering::SeqCst);
            },
        )
        .await
    });
    // Wait until section 0001's directory has the stage's predecessor outputs, i.e. the worker
    // is lingering there. The lingering worker never finishes, so poll the work folder.
    let work = WorkFolder::folder_for(&cfg.project_path);
    let started = std::time::Instant::now();
    loop {
        let section_one = work.join("sections/0001");
        let lingering = match stage {
            Stage::Separate => section_one.exists() && !section_one.join("sec-vocals.wav").exists(),
            Stage::Transcribe => {
                section_one.join("sec-vocals.wav").exists()
                    && !section_one.join("sec.json").exists()
            }
            Stage::Translate => {
                section_one.join("lines.json").exists()
                    && !section_one.join("translated.json").exists()
            }
            _ => {
                section_one.join("translated.json").exists()
                    && !section_one.join("synth.json").exists()
            }
        };
        // Give the worker a moment to enter its linger loop.
        if lingering
            && started.elapsed() > Duration::from_secs(1)
            && work.join("sections/0000/synth.json").exists()
        {
            tokio::time::sleep(Duration::from_millis(500)).await;
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "never reached {stage:?} in section 1"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    handle.abort();
    let _ = handle.await;
    let _ = reached;

    let wf = WorkFolder::open(&cfg.project_path).unwrap();
    assert!(wf.is_interrupted(), "a killed run is found as interrupted");
    let before = stamps(&work);
    let section0: Vec<_> = before
        .keys()
        .filter(|k| k.contains("sections/0000") || k.contains("sections\\0000"))
        .cloned()
        .collect();
    assert!(!section0.is_empty());

    // Resume without the slow worker.
    let fast = Arc::new(support::run_fixture::config(
        root.path(),
        "k.flick",
        json!({}),
    ));
    let out = run_dub(
        &fast,
        dub,
        Mode::Resume,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert!(work.join("dub-mix.wav").exists());
    let after = stamps(&work);
    for k in &section0 {
        assert_eq!(before[k], after[k], "{k} must not be redone");
    }
    assert!(
        after.keys().any(|k| k.contains("0002")),
        "later sections were done"
    );
    assert!(
        out.lines.iter().any(|l| l.section_index == 1)
            && out.lines.iter().any(|l| l.section_index == 2)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn killed_during_separate_resumes_redoing_only_that_section() {
    kill_then_resume("separate", Stage::Separate).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn killed_during_transcribe_resumes_redoing_only_that_section() {
    kill_then_resume("transcribe", Stage::Transcribe).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn killed_during_translate_resumes_redoing_only_that_section() {
    kill_then_resume("translate", Stage::Translate).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn killed_during_synthesize_resumes_redoing_only_that_section() {
    kill_then_resume("synthesize", Stage::Synthesize).await;
}

#[tokio::test]
async fn cancel_keeps_finished_work_and_leaves_no_part_files_then_resume_completes() {
    let root = tempfile::tempdir().unwrap();
    let source = long_clip(root.path(), 12);
    let cfg = config(
        root.path(),
        "c.flick",
        json!({ "slowWorker": "translate", "slowSection": "0001", "slowSeconds": 60 }),
    );
    let dub = dub_for(&source, "en").await;
    let cancel = CancelToken::default();
    let trigger = cancel.clone();
    let work = WorkFolder::folder_for(&cfg.project_path);
    let watcher = tokio::spawn({
        let work = work.clone();
        async move {
            while !work.join("sections/0001/lines.json").exists() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
            trigger.cancel();
        }
    });
    let started = std::time::Instant::now();
    let out = run_dub(&cfg, dub.clone(), Mode::Fresh, cancel, &mut |_| {})
        .await
        .unwrap();
    watcher.await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(40),
        "cancel stops within seconds"
    );
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Cancelled);
    assert!(!walk_any(&work, "part"));
    assert!(
        work.join("sections/0000/synth.json").exists(),
        "finished work is kept"
    );

    let fast = config(root.path(), "c.flick", json!({}));
    let done = run_dub(
        &fast,
        dub,
        Mode::Resume,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(done.run.unwrap().state, RunState::Completed);
}

#[tokio::test]
async fn resume_needs_an_unfinished_run_and_an_unchanged_source() {
    let root = tempfile::tempdir().unwrap();
    let cfg = config(root.path(), "r.flick", json!({}));
    let source = root.path().join("src.mp4");
    std::fs::copy(clip("clip-20s-fr.mp4"), &source).unwrap();
    let dub = dub_for(&source, "en").await;

    let err = run_dub(
        &cfg,
        dub.clone(),
        Mode::Resume,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, NOTHING_TO_RESUME);

    run_dub(
        &cfg,
        dub.clone(),
        Mode::Fresh,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    let mut bytes = std::fs::read(&source).unwrap();
    bytes[10] ^= 0xff;
    std::fs::write(&source, bytes).unwrap();
    let err = run_dub(&cfg, dub, Mode::Resume, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, SOURCE_CHANGED);
}

#[tokio::test]
async fn a_speech_free_video_completes_with_the_original_audio_and_a_notice() {
    let root = tempfile::tempdir().unwrap();
    let cfg = config(root.path(), "n.flick", json!({ "speech": false }));
    let dub = dub_for(&clip("clip-no-speech.mp4"), "en").await;
    let out = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    let run = out.run.unwrap();
    assert_eq!(run.state, RunState::Completed);
    assert_eq!(run.error.unwrap().code, NO_SPEECH_FOUND);
    assert!(out.lines.is_empty());
    assert!(WorkFolder::folder_for(&cfg.project_path)
        .join("dub-mix.wav")
        .exists());
}

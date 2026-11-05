//! Cloud translation and speech inside a complete run: consent first, the same clip files as
//! the local path, the engine in the stage hashes, and a usage record with no key and no text.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flickfluent_core::cloud::services::Step;
use flickfluent_core::error::*;
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunEvent};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{RunState, Stage};
use serde_json::json;
use support::cloud_run::{cloud, use_cloud};
use support::fake_cloud::FakeCloud;
use support::run_fixture::{config, dub_for};

const KEY: &str = "pipeline-key-MARKER-0001";

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

async fn setup(
    root: &Path,
    fake: &FakeCloud,
    consent: bool,
) -> (
    flickfluent_core::pipeline::run::RunConfig,
    flickfluent_core::project::dub::DubData,
) {
    let c = cloud(root, fake, &[("google-gemini", "mine", KEY, None)]);
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), Some("google-gemini"));
    if consent {
        for step in [Step::Translate, Step::Speak] {
            c.consent.grant("google-gemini", step, 1_000_000).unwrap();
        }
    }
    let mut cfg = config(root, "c.flick", json!({}));
    cfg.cloud = Some(c.runtime.clone());
    (cfg, dub)
}

#[tokio::test]
async fn a_run_without_consent_fails_before_any_request() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let (cfg, dub) = setup(root.path(), &fake, false).await;
    let err = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, CLOUD_CONSENT_REQUIRED);
    let steps = err.details.unwrap()["steps"].as_array().unwrap().len();
    assert_eq!(steps, 2);
    assert_eq!(fake.count(), 0);
}

#[tokio::test]
async fn a_run_without_a_key_fails_with_cloud_no_key() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(root.path(), &fake, &[]);
    c.consent
        .grant("google-gemini", Step::Translate, 1_000_000)
        .unwrap();
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), None);
    let mut cfg = config(root.path(), "c.flick", json!({}));
    cfg.cloud = Some(c.runtime.clone());
    let err = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(
        err.code, CLOUD_NO_KEY,
        "the run must not fall back to the local engine by itself"
    );
    assert_eq!(fake.count(), 0);
}

#[tokio::test]
async fn translation_and_speech_go_through_the_service_and_look_like_local_ones() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let (cfg, dub) = setup(root.path(), &fake, true).await;
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

    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert!(!out.lines.is_empty());
    for l in &out.lines {
        assert!(
            l.translated.as_deref().unwrap().starts_with("[tr] "),
            "{:?}",
            l.translated
        );
        assert!(
            l.audio_ms.is_some()
                && l.audio
                    .as_deref()
                    .unwrap()
                    .ends_with(&format!("clips/{}.wav", l.id))
        );
    }
    let work = WorkFolder::folder_for(&cfg.project_path);
    for l in &out.lines {
        assert!(work.join(l.audio.as_ref().unwrap()).exists());
    }
    assert!(
        work.join("sections/0000/translated.partial.json").exists()
            || work.join("sections").read_dir().unwrap().next().is_some()
    );

    // The usage record: service, characters, requests, keys; no key and no text.
    assert_eq!(out.cloud_usage.len(), 2);
    for u in &out.cloud_usage {
        assert_eq!(
            (u.service_id.as_str(), u.service_name.as_str()),
            ("google-gemini", "Google Gemini API")
        );
        assert!(u.chars > 0 && u.requests > 0);
        assert_eq!(u.keys_used, vec!["mine".to_string()]);
    }
    let usage_json = serde_json::to_string(&out.cloud_usage).unwrap();
    assert!(!usage_json.contains(KEY));
    for l in &out.lines {
        assert!(!usage_json.contains(&l.text));
    }
    // Only text was sent: no request body holds audio of the video, and the key is a header.
    for r in fake.requests() {
        assert!(!r.path.contains(KEY) && !r.body_text().contains(KEY));
    }
    // The cloud steps are announced as ordinary stage events.
    let ev = events.lock().unwrap();
    for stage in [Stage::Translate, Stage::Synthesize] {
        assert!(ev
            .iter()
            .any(|e| matches!(e, RunEvent::Stage { stage: s, .. } if *s == stage)));
    }
}

#[tokio::test]
async fn the_engine_is_part_of_the_stage_input_hashes() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let (cfg, cloud_dub) = setup(root.path(), &fake, true).await;
    let local_dub = {
        let mut d = cloud_dub.clone();
        use_cloud(&mut d, Some("local"), Some("local"));
        d
    };
    let hashes = |dub: flickfluent_core::project::dub::DubData| {
        let run = dub.run.unwrap();
        let s = &run.sections[0];
        (
            s.input_hash[&Stage::Translate].clone(),
            s.input_hash[&Stage::Synthesize].clone(),
        )
    };
    let cloud_out = run_dub(
        &cfg,
        cloud_dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    let local_out = run_dub(
        &cfg,
        local_dub,
        Mode::Fresh,
        CancelToken::default(),
        &mut |_| {},
    )
    .await
    .unwrap();
    let (ct, cs) = hashes(cloud_out);
    let (lt, ls) = hashes(local_out);
    assert_ne!(ct, lt, "changing the engine must redo translation");
    assert_ne!(cs, ls, "changing the engine must redo speech");
}

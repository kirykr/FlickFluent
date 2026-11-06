//! The editor picks voices, models and engine the way a run does (research R17).

mod support;

use std::path::{Path, PathBuf};

use flickfluent_core::cloud::services::Step as CloudStep;
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::error::MODELS_MISSING;
use flickfluent_core::pipeline::plan::plan_models_full;
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{DubData, Quality};
use serde_json::json;
use support::run_fixture::{catalogue, config, dub_for};

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

async fn finished_run(root: &Path) -> DubData {
    let cfg = config(root, "a.flick", json!({}));
    let dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap()
}

#[tokio::test]
async fn it_picks_the_voice_and_engine_the_run_planned() {
    let root = tempfile::tempdir().unwrap();
    let dub = finished_run(root.path()).await;
    let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);

    let expected = plan_models_full(
        &catalogue(),
        rt.config().class,
        &rt.config().os,
        &rt.config().arch,
        "fr",
        "en",
        dub.options.quality == Quality::Fast,
        &["voice-en-f".to_string(), "voice-en-m".to_string()],
        &dub.options.fallbacks,
    );
    for speaker in &dub.speakers {
        let gender = match speaker.gender {
            flickfluent_core::project::dub::SpeakerGender::Male => {
                flickfluent_core::models::Gender::Male
            }
            _ => flickfluent_core::models::Gender::Female,
        };
        assert_eq!(
            rt.voice_for(&speaker.id).unwrap().model,
            expected.voices[&gender].model,
            "{}",
            speaker.id
        );
    }
    // spk1 is male and spk2 female in the fake transcript.
    assert_eq!(rt.voice_for("spk1").unwrap().model, "voice-en-m");
    assert_eq!(rt.voice_for("spk2").unwrap().model, "voice-en-f");
    assert_eq!(rt.speech_engine_id("spk1"), "local:voice-en-m");
    assert_eq!(rt.engine_id(CloudStep::Translate), "local:m2m");
    assert!(rt.cloud().is_none());
}

#[tokio::test]
async fn a_cloud_engine_in_the_settings_changes_the_engine_id() {
    let root = tempfile::tempdir().unwrap();
    let mut dub = finished_run(root.path()).await;
    let local = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub)
        .speech_engine_id("spk1");

    support::cloud_run::use_cloud(&mut dub, Some("google-gemini"), Some("google-gemini"));
    let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);
    assert_ne!(rt.speech_engine_id("spk1"), local);
    assert_eq!(rt.speech_engine_id("spk1"), "cloud:google-gemini");
    assert_eq!(rt.engine_id(CloudStep::Translate), "cloud:google-gemini");
}

#[tokio::test]
async fn missing_models_are_reported_by_name() {
    let root = tempfile::tempdir().unwrap();
    let dub = finished_run(root.path()).await;
    let cfg = config(root.path(), "a.flick", json!({}));
    let rt = EditorRuntime::for_project(cfg, &dub);
    assert!(rt.check_models(&["spk1", "spk2"]).is_ok());

    std::fs::remove_dir_all(root.path().join("models/voice-en-m")).unwrap();
    let rt = EditorRuntime::for_project(config_without_creating(root.path()), &dub);
    let err = rt.check_models(&["spk1", "spk2"]).unwrap_err();
    assert_eq!(err.code, MODELS_MISSING);
    assert_eq!(err.details.unwrap()["missing"], json!(["voice-en-m"]));
    // The speaker whose voice is still there is fine.
    assert!(rt.check_models(&["spk2"]).is_ok());
}

/// `config` recreates every model folder; this one leaves the models folder as it is.
fn config_without_creating(root: &Path) -> flickfluent_core::pipeline::run::RunConfig {
    let mut cfg = config(root, "a.flick", json!({}));
    // `config` made the folders again, so remove the one the test wants gone.
    let _ = std::fs::remove_dir_all(root.join("models/voice-en-m"));
    cfg.models_dir = root.join("models");
    cfg
}

#[tokio::test]
async fn a_project_without_its_work_folder_still_opens() {
    let root = tempfile::tempdir().unwrap();
    let dub = finished_run(root.path()).await;
    let cfg = config(root.path(), "a.flick", json!({}));
    let rt = EditorRuntime::for_project(cfg, &dub);
    assert!(rt.work_folder().is_some());

    WorkFolder::remove(&root.path().join("a.flick")).unwrap();
    let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);
    assert!(rt.work_folder().is_none());
    assert_eq!(rt.voice_for("spk1").unwrap().model, "voice-en-m");
}

#[tokio::test]
async fn a_fresh_run_is_already_editable() {
    use flickfluent_core::editor::hash::changed_ids;
    let root = tempfile::tempdir().unwrap();
    let dub = finished_run(root.path()).await;
    assert!(!dub.lines.is_empty());
    for l in &dub.lines {
        assert_eq!(l.raw_audio, l.audio, "the engine's clip is the raw clip");
        assert_eq!(l.raw_ms, l.audio_ms);
        assert!(!l.speech_hash.is_empty(), "{} has no hash", l.id);
        assert_eq!((l.speed, l.volume_db, l.pitch_semitones), (1.0, 0.0, 0.0));
    }
    // The hash the run recorded is the hash the editor computes: nothing is changed.
    let rt = EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), &dub);
    assert!(changed_ids(&rt, &dub.speakers, &dub.lines).is_empty());
}

#[tokio::test]
async fn a_later_run_keeps_a_gender_the_user_set() {
    use flickfluent_core::project::dub::{GenderSource, Speaker, SpeakerGender};
    let root = tempfile::tempdir().unwrap();
    let cfg = config(root.path(), "a.flick", json!({}));
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    dub.speakers = vec![Speaker {
        id: "spk1".into(),
        gender: SpeakerGender::Female, // the fake transcript says male
        gender_source: GenderSource::User,
        ..Default::default()
    }];
    let out = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    let spk1 = out.speakers.iter().find(|s| s.id == "spk1").unwrap();
    assert_eq!(
        (spk1.gender, spk1.gender_source),
        (SpeakerGender::Female, GenderSource::User)
    );
}

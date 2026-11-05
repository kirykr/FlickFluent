//! A cloud run that stops (every key limited, a lost connection) keeps what it finished, resumes
//! without asking again for it, and continues its usage record. Keys of another service are
//! never used.

mod support;

use std::path::{Path, PathBuf};

use flickfluent_core::cloud::services::Step;
use flickfluent_core::error::*;
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode, RunEvent};
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{RunState, Stage, StageStatus};
use serde_json::json;
use support::cloud_run::{cloud, use_cloud};
use support::fake_cloud::{FakeCloud, Mode as Fake};
use support::run_fixture::{config, dub_for};

const KEY: &str = "resume-key-MARKER-0001";
const AZURE: &str = "azure-key-MARKER-0002";

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

fn translate_requests(fake: &FakeCloud) -> usize {
    fake.requests()
        .iter()
        .filter(|r| r.body_text().contains("responseSchema"))
        .count()
}

#[tokio::test]
async fn a_run_that_hits_the_wait_limit_during_speech_keeps_its_translation_and_resumes() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(root.path(), &fake, &[("google-gemini", "mine", KEY, None)]);
    for step in [Step::Translate, Step::Speak] {
        c.consent.grant("google-gemini", step, 1_000_000).unwrap();
    }
    let mut cfg = config(root.path(), "r.flick", json!({}));
    cfg.cloud = Some(c.runtime.clone());
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), Some("google-gemini"));

    // As speech starts, the service asks for a wait longer than the app's ten minutes.
    let err = run_dub(
        &cfg,
        dub.clone(),
        Mode::Fresh,
        CancelToken::default(),
        &mut |e| {
            if matches!(
                e,
                RunEvent::Stage {
                    stage: Stage::Synthesize,
                    status: StageStatus::Running
                }
            ) {
                fake.set_default(Fake::RateLimit {
                    times: 1000,
                    retry_after: Some(700),
                });
            }
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, CLOUD_KEYS_EXHAUSTED);
    assert_eq!(err.details.as_ref().unwrap()["retryAfterSeconds"], 700);

    let work = WorkFolder::open_for(&cfg.project_path, &cfg.project_id).unwrap();
    let run = work.load_run().unwrap().unwrap();
    assert_eq!(
        run.state,
        RunState::Failed,
        "the run is paused in a resumable state"
    );
    assert_eq!(run.error.as_ref().unwrap().code, CLOUD_KEYS_EXHAUSTED);
    let before = translate_requests(&fake);
    assert!(before >= 1);
    let usage_file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(work.path().join("cloud-usage.json")).unwrap())
            .unwrap();
    assert_eq!(usage_file["runId"], run.id.as_str());
    let translate_before = usage_file["usage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["step"] == "translate")
        .unwrap()["requests"]
        .as_u64()
        .unwrap();

    // Later the limit has passed. Resume: nothing finished is asked for again.
    fake.reset();
    let out = run_dub(&cfg, dub, Mode::Resume, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert_eq!(
        translate_requests(&fake),
        before,
        "translation was not requested again"
    );
    assert!(out.lines.iter().all(|l| l.audio_ms.is_some()));

    // The usage record continues instead of starting over.
    let translate = out
        .cloud_usage
        .iter()
        .find(|u| u.step == "translate")
        .unwrap();
    let speak = out.cloud_usage.iter().find(|u| u.step == "speak").unwrap();
    assert_eq!(translate.requests, translate_before);
    assert!(speak.requests >= out.lines.len() as u64);
    assert_eq!(
        out.cloud_usage
            .iter()
            .map(|u| u.run_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_lost_connection_pauses_the_run_with_cloud_offline() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(root.path(), &fake, &[("google-gemini", "mine", KEY, None)]);
    c.consent
        .grant("google-gemini", Step::Translate, 1_000_000)
        .unwrap();
    let mut cfg = config(root.path(), "o.flick", json!({}));
    // Point the adapters at nothing: the connection cannot be made.
    let dead = {
        use flickfluent_core::cloud::adapters::gemini::GeminiAdapter;
        use flickfluent_core::cloud::services::Endpoints;
        let url = "http://127.0.0.1:1".to_string();
        let rt = flickfluent_core::cloud::runtime::CloudRuntime {
            book: c.book.clone(),
            consent: c.consent.clone(),
            app_data: root.path().join("appdata"),
            adapters: std::sync::Arc::new(move |s| {
                Some(std::sync::Arc::new(GeminiAdapter::new(
                    s.clone().with_endpoints(Endpoints {
                        check: url.clone(),
                        translate: Some(url.clone()),
                        speak: Some(url.clone()),
                        ..Endpoints::default()
                    }),
                )))
            }),
            clock: c.clock.clone(),
            sleeper: c.clock.clone(),
        };
        std::sync::Arc::new(rt)
    };
    cfg.cloud = Some(dead);
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), None);
    let err = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, CLOUD_OFFLINE);
    let work = WorkFolder::open_for(&cfg.project_path, &cfg.project_id).unwrap();
    assert_eq!(work.load_run().unwrap().unwrap().state, RunState::Failed);
}

#[tokio::test]
async fn another_services_key_is_never_used_for_a_run() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(
        root.path(),
        &fake,
        &[
            ("google-gemini", "g", KEY, None),
            ("azure-ai", "az", AZURE, Some("westeurope")),
        ],
    );
    c.consent
        .grant("google-gemini", Step::Translate, 1_000_000)
        .unwrap();
    // The Gemini key is refused; the Azure key must not be tried in its place.
    fake.set_for_key(KEY, Fake::Reject);
    let mut cfg = config(root.path(), "x.flick", json!({}));
    cfg.cloud = Some(c.runtime.clone());
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), None);
    let err = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code, CLOUD_AUTH_FAILED);
    assert_eq!(
        fake.count_for_key(AZURE),
        0,
        "the user agreed to send data to Gemini only"
    );
}

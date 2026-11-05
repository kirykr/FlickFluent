//! With the default engines a complete dub makes no request to any cloud service (SC-003),
//! even when a verified key is saved.

mod support;

use std::path::{Path, PathBuf};

use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode};
use flickfluent_core::project::dub::RunState;
use serde_json::json;
use support::cloud_run::cloud;
use support::fake_cloud::FakeCloud;
use support::run_fixture::{config, dub_for};

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

#[tokio::test]
async fn a_full_dub_with_default_settings_sends_nothing() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(
        root.path(),
        &fake,
        &[("google-gemini", "k", "gem-key-0001", None)],
    );
    c.consent
        .grant(
            "google-gemini",
            flickfluent_core::cloud::services::Step::Translate,
            1_000_000,
        )
        .unwrap();
    let mut cfg = config(root.path(), "a.flick", json!({}));
    cfg.cloud = Some(c.runtime.clone());
    let dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await; // engines are local by default

    let out = run_dub(&cfg, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(out.run.as_ref().unwrap().state, RunState::Completed);
    assert!(out
        .lines
        .iter()
        .all(|l| l.translated.as_deref().unwrap().starts_with("T:")));
    assert!(out.cloud_usage.is_empty());
    assert_eq!(
        fake.count(),
        0,
        "a cloud request was made with the default engines"
    );
}

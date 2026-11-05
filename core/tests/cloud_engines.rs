//! Engine choice per step: local by default, a service only with a verified key, never switched
//! silently.

use std::sync::Arc;

use flickfluent_core::cloud::choice::{validate_engine, Engines, LOCAL};
use flickfluent_core::cloud::keys::KeyBook;
use flickfluent_core::cloud::keystore::MemoryStore;
use flickfluent_core::cloud::services::{self, Step};
use flickfluent_core::error::*;
use flickfluent_core::settings::Settings;
use serde_json::json;

fn book(dir: &std::path::Path) -> KeyBook {
    KeyBook::new(
        dir.to_path_buf(),
        Arc::new(MemoryStore::new()),
        services::load().unwrap(),
    )
}

#[test]
fn a_fresh_settings_file_is_local_for_both_steps() {
    let s = Settings::default();
    assert_eq!(
        (s.engines.translate.as_str(), s.engines.speak.as_str()),
        (LOCAL, LOCAL)
    );
    assert_eq!(s.engines.cloud(Step::Translate), None);
}

#[test]
fn an_old_settings_file_without_engines_still_loads() {
    let old = r#"{"settingsVersion":1,"theme":"dark","analyticsEnabled":false}"#;
    let s: Settings = serde_json::from_str(old).unwrap();
    assert_eq!(s.engines, Engines::default());
}

#[test]
fn engines_can_be_patched_into_the_settings() {
    let mut s = Settings::default();
    s.apply_patch(&json!({ "engines": { "translate": "google-gemini", "speak": "local" } }))
        .unwrap();
    assert_eq!(s.engines.cloud(Step::Translate), Some("google-gemini"));
    assert_eq!(s.engines.cloud(Step::Speak), None);
}

#[test]
fn a_service_needs_a_verified_key_and_must_offer_the_step() {
    let dir = tempfile::tempdir().unwrap();
    let b = book(dir.path());
    assert!(validate_engine(&b, Step::Translate, LOCAL).is_ok());
    assert_eq!(
        validate_engine(&b, Step::Translate, "google-gemini")
            .unwrap_err()
            .code,
        CLOUD_NO_KEY
    );
    assert_eq!(
        validate_engine(&b, Step::Translate, "nope")
            .unwrap_err()
            .code,
        INVALID_SETTING
    );

    let key = b
        .add("google-gemini", "k", "gem-key-0001".into(), None, false)
        .unwrap();
    // Added but not verified yet: still refused.
    assert_eq!(
        validate_engine(&b, Step::Translate, "google-gemini")
            .unwrap_err()
            .code,
        CLOUD_NO_KEY
    );
    b.set_state(&key.id, flickfluent_core::cloud::keys::KeyState::Verified)
        .unwrap();
    assert!(validate_engine(&b, Step::Translate, "google-gemini").is_ok());
    assert!(validate_engine(&b, Step::Speak, "google-gemini").is_ok());

    // Removing the last key makes the choice invalid again; nothing falls back by itself.
    b.remove(&key.id).unwrap();
    assert_eq!(
        validate_engine(&b, Step::Translate, "google-gemini")
            .unwrap_err()
            .code,
        CLOUD_NO_KEY
    );
}

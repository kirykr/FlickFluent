//! "Verify connection": success names the languages, each failure has its reason, and a key
//! is only marked rejected when the service refused it.

mod support;

use std::sync::Arc;

use flickfluent_core::cloud::adapters::gemini::GeminiAdapter;
use flickfluent_core::cloud::keys::{KeyBook, KeyState, VerifyReason};
use flickfluent_core::cloud::keystore::MemoryStore;
use flickfluent_core::cloud::services::{self, Endpoints};
use support::fake_cloud::{FakeCloud, Mode};

const KEY: &str = "verify-key-0001";

fn setup(fake: &FakeCloud) -> (tempfile::TempDir, KeyBook, GeminiAdapter, String) {
    let dir = tempfile::tempdir().unwrap();
    let book = KeyBook::new(
        dir.path().to_path_buf(),
        Arc::new(MemoryStore::new()),
        services::load().unwrap(),
    );
    let id = book
        .add("google-gemini", "k", KEY.into(), None, false)
        .unwrap()
        .id;
    let url = fake.gemini_models_url();
    let svc = services::find(book.services(), "google-gemini")
        .unwrap()
        .clone()
        .with_endpoints(Endpoints {
            check: url,
            ..Endpoints::default()
        });
    (dir, book, GeminiAdapter::new(svc), id)
}

#[tokio::test]
async fn success_marks_the_key_verified_and_caches_the_live_languages() {
    let fake = FakeCloud::start();
    let (_dir, book, adapter, id) = setup(&fake);
    let r = book.verify(&id, &adapter).await.unwrap();
    assert!(r.ok && r.languages.is_some() && r.reason.is_none());
    let key = book.info(&id).unwrap().unwrap();
    assert_eq!(key.state, KeyState::Verified);
    assert!(key.verified_at.is_some());
    assert!(book.cached_languages("google-gemini").is_some());
}

#[tokio::test]
async fn a_refusal_marks_the_key_rejected_with_the_reason() {
    for (mode, reason) in [
        (Mode::Reject, VerifyReason::Rejected),
        (Mode::NoCredit, VerifyReason::NoCredit),
    ] {
        let fake = FakeCloud::start();
        fake.set_default(mode);
        let (_dir, book, adapter, id) = setup(&fake);
        let r = book.verify(&id, &adapter).await.unwrap();
        assert!(!r.ok);
        assert_eq!(r.reason, Some(reason));
        assert!(!r.message.unwrap().contains(KEY));
        assert_eq!(book.info(&id).unwrap().unwrap().state, KeyState::Rejected);
    }
}

#[tokio::test]
async fn offline_or_a_service_problem_keeps_the_key_as_it_was() {
    let fake = FakeCloud::start();
    let (_dir, book, adapter, id) = setup(&fake);
    book.verify(&id, &adapter).await.unwrap();
    let verified_at = book.info(&id).unwrap().unwrap().verified_at;

    fake.set_default(Mode::ServerError);
    let r = book.verify(&id, &adapter).await.unwrap();
    assert_eq!(r.reason, Some(VerifyReason::Unreachable));

    let dead = services::find(book.services(), "google-gemini")
        .unwrap()
        .clone()
        .with_endpoints(Endpoints {
            check: "http://127.0.0.1:1".into(),
            ..Endpoints::default()
        });
    let r = book.verify(&id, &GeminiAdapter::new(dead)).await.unwrap();
    assert_eq!(r.reason, Some(VerifyReason::Offline));

    fake.set_default(Mode::Junk);
    let r = book.verify(&id, &adapter).await.unwrap();
    assert_eq!(r.reason, Some(VerifyReason::Unexpected));

    let key = book.info(&id).unwrap().unwrap();
    assert_eq!(
        key.state,
        KeyState::Verified,
        "a failure to reach the service must not unverify the key"
    );
    assert_eq!(key.verified_at, verified_at);
}

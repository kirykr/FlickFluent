//! Consent: nothing is sent before the user agrees, the amount is asked again only when a run
//! is more than five times the agreed one, and a withdrawal asks again.

use flickfluent_core::cloud::consent::{ConsentBook, Reason, NOTICE_VERSION};
use flickfluent_core::cloud::services::Step;

fn book() -> (tempfile::TempDir, ConsentBook) {
    let dir = tempfile::tempdir().unwrap();
    let b = ConsentBook::new(dir.path().to_path_buf());
    (dir, b)
}

#[test]
fn with_no_record_consent_is_required() {
    let (_d, b) = book();
    assert_eq!(
        b.needs("google-gemini", Step::Translate, 100).unwrap(),
        Some(Reason::First)
    );
}

#[test]
fn granting_stores_service_step_amount_and_notice_version() {
    let (d, b) = book();
    let rec = b.grant("google-gemini", Step::Translate, 1_000).unwrap();
    assert_eq!(
        (rec.service_id.as_str(), rec.step, rec.agreed_chars),
        ("google-gemini", Step::Translate, 1_000)
    );
    assert_eq!(rec.notice_version, NOTICE_VERSION);
    assert!(!rec.agreed_at.is_empty());
    assert_eq!(b.list().unwrap().len(), 1);
    // The file holds no key.
    let file = std::fs::read_to_string(d.path().join("cloud-consent.json")).unwrap();
    assert!(file.contains("google-gemini") && !file.to_lowercase().contains("secret"));
}

#[test]
fn a_later_run_asks_again_only_above_five_times_the_agreed_amount() {
    let (_d, b) = book();
    b.grant("azure-ai", Step::Speak, 1_000).unwrap();
    assert_eq!(b.needs("azure-ai", Step::Speak, 1_000).unwrap(), None);
    assert_eq!(
        b.needs("azure-ai", Step::Speak, 5_000).unwrap(),
        None,
        "exactly 5x is allowed"
    );
    assert_eq!(
        b.needs("azure-ai", Step::Speak, 5_001).unwrap(),
        Some(Reason::LargerRun)
    );
    assert_eq!(
        b.needs("azure-ai", Step::Speak, 10).unwrap(),
        None,
        "a smaller run never asks"
    );
}

#[test]
fn granting_again_replaces_the_amount() {
    let (_d, b) = book();
    b.grant("azure-ai", Step::Speak, 1_000).unwrap();
    b.grant("azure-ai", Step::Speak, 20_000).unwrap();
    assert_eq!(b.list().unwrap().len(), 1);
    assert_eq!(b.needs("azure-ai", Step::Speak, 100_000).unwrap(), None);
}

#[test]
fn consent_for_one_step_or_service_does_not_cover_another() {
    let (_d, b) = book();
    b.grant("google-gemini", Step::Translate, 1_000).unwrap();
    assert_eq!(
        b.needs("google-gemini", Step::Speak, 10).unwrap(),
        Some(Reason::First)
    );
    assert_eq!(
        b.needs("azure-ai", Step::Translate, 10).unwrap(),
        Some(Reason::First)
    );
}

#[test]
fn withdrawing_removes_the_record_and_asks_again() {
    let (_d, b) = book();
    b.grant("google-gemini", Step::Translate, 1_000).unwrap();
    b.withdraw("google-gemini", Step::Translate).unwrap();
    assert!(b.list().unwrap().is_empty());
    assert_eq!(
        b.needs("google-gemini", Step::Translate, 10).unwrap(),
        Some(Reason::First)
    );
    b.withdraw("google-gemini", Step::Translate).unwrap(); // already gone: fine
}

#[test]
fn an_older_notice_version_asks_again() {
    let (d, b) = book();
    b.grant("google-gemini", Step::Translate, 1_000).unwrap();
    let path = d.path().join("cloud-consent.json");
    let text = std::fs::read_to_string(&path).unwrap().replace(
        &format!("\"noticeVersion\": {NOTICE_VERSION}"),
        "\"noticeVersion\": 0",
    );
    std::fs::write(&path, text).unwrap();
    assert_eq!(
        b.needs("google-gemini", Step::Translate, 10).unwrap(),
        Some(Reason::NoticeChanged)
    );
}

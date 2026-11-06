//! The amount guard shared by a run and the editor (research R18).

use flickfluent_core::cloud::consent::ConsentBook;
use flickfluent_core::cloud::estimate::read_floor;
use flickfluent_core::cloud::guard::{guard_amount, merge_usage, used_chars};
use flickfluent_core::cloud::services::Step;
use flickfluent_core::error::CLOUD_CONSENT_REQUIRED;
use flickfluent_core::project::dub::CloudUsage;

const SERVICE: &str = "google-gemini";

fn book(agreed: Option<u64>) -> (tempfile::TempDir, ConsentBook) {
    let dir = tempfile::tempdir().unwrap();
    let book = ConsentBook::new(dir.path().join("appdata"));
    if let Some(chars) = agreed {
        book.grant(SERVICE, Step::Speak, chars).unwrap();
    }
    (dir, book)
}

fn usage(step: &str, chars: u64, keys: &[&str]) -> CloudUsage {
    CloudUsage {
        step: step.into(),
        service_id: SERVICE.into(),
        service_name: "Gemini".into(),
        chars,
        requests: 1,
        keys_used: keys.iter().map(|k| k.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn it_passes_under_the_allowed_amount() {
    let (dir, book) = book(Some(2)); // allowed: 5 times 2
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    guard_amount(&book, &work, Step::Speak, SERVICE, 4, 6, 10_000, 10_000).unwrap();
    assert!(!work.join("cloud-needed.json").exists());
}

#[test]
fn it_stops_above_it_and_keeps_the_projection() {
    let (dir, book) = book(Some(2));
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    // 4 used, 7 more: 11 against 10 allowed. Half way through the video, so about 22 in all.
    let err = guard_amount(&book, &work, Step::Speak, SERVICE, 4, 7, 5_000, 10_000).unwrap_err();
    assert_eq!(err.code, CLOUD_CONSENT_REQUIRED);
    let d = err.details.unwrap();
    assert_eq!(d["projectedChars"], 22);
    assert_eq!(d["steps"][0]["step"], "speak");
    assert_eq!(d["steps"][0]["serviceId"], SERVICE);
    assert_eq!(read_floor(&work, Step::Speak, SERVICE), 22);
}

#[test]
fn it_passes_when_there_is_no_agreement_record() {
    // The run checked consent before it started; the guard only watches the amount.
    let (dir, book) = book(None);
    guard_amount(&book, dir.path(), Step::Speak, SERVICE, 0, 1_000_000, 1, 1).unwrap();
}

#[test]
fn usage_adds_to_an_existing_record_and_starts_a_new_one() {
    let base = vec![usage("speak", 100, &["a"])];
    let merged = merge_usage(
        base.clone(),
        vec![
            usage("speak", 30, &["a", "b"]),
            usage("translate", 5, &["a"]),
        ],
        "run-2",
    );
    let speak = merged.iter().find(|u| u.step == "speak").unwrap();
    assert_eq!(speak.chars, 130);
    assert_eq!(speak.requests, 2);
    assert_eq!(speak.keys_used, vec!["a", "b"]);
    let translate = merged.iter().find(|u| u.step == "translate").unwrap();
    assert_eq!((translate.chars, translate.run_id.as_str()), (5, "run-2"));
    assert_eq!(used_chars(&merged, Step::Speak, SERVICE), 130);
    assert_eq!(used_chars(&merged, Step::Translate, "other"), 0);
}

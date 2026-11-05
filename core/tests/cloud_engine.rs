//! The cloud engine: batches, kept progress, rotation without lost or doubled lines, and speech
//! fitted to its slot.

mod support;

use std::path::Path;
use std::sync::Arc;

use flickfluent_core::cloud::adapters::gemini::GeminiAdapter;
use flickfluent_core::cloud::clock::FakeClock;
use flickfluent_core::cloud::engine::{CloudEngine, SpeakJob};
use flickfluent_core::cloud::rotation::{KeyPool, PoolKey};
use flickfluent_core::cloud::secret::Secret;
use flickfluent_core::cloud::services::{self, Endpoints, Gender, Step};
use flickfluent_core::error::*;
use flickfluent_core::pipeline::ffmpeg::Ffmpeg;
use support::fake_cloud::{FakeCloud, Mode};

const K1: &str = "engine-key-0001";
const K2: &str = "engine-key-0002";

fn engine(
    fake: &FakeCloud,
    step: Step,
    keys: &[(&str, &str)],
    clock: &Arc<FakeClock>,
) -> CloudEngine {
    let all = services::load().unwrap();
    let url = fake.gemini_models_url();
    let service = services::find(&all, "google-gemini")
        .unwrap()
        .clone()
        .with_endpoints(Endpoints {
            check: url.clone(),
            translate: Some(url.clone()),
            speak: Some(url),
            ..Endpoints::default()
        });
    let pool = KeyPool::new(
        keys.iter()
            .map(|(l, s)| PoolKey {
                label: (*l).into(),
                region: None,
                secret: Secret::new((*s).into()),
            })
            .collect(),
        clock.clone(),
        clock.clone(),
    );
    CloudEngine::new(
        step,
        service.clone(),
        Arc::new(GeminiAdapter::new(service)),
        pool,
        None,
    )
}

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("ligne {i}")).collect()
}

fn never() -> impl Fn() -> bool + Sync {
    || false
}

/// How many lines each translation request carried, from the fake server's log.
fn batch_sizes(fake: &FakeCloud) -> Vec<usize> {
    fake.requests()
        .iter()
        .filter_map(|r| {
            let body = r.body_text();
            let json: serde_json::Value = serde_json::from_str(&body).ok()?;
            let prompt = json["contents"][0]["parts"][0]["text"]
                .as_str()?
                .to_string();
            let list: Vec<String> =
                serde_json::from_str(prompt.split("LINES:").nth(1)?.trim()).ok()?;
            Some(list.len())
        })
        .collect()
}

#[tokio::test]
async fn translation_goes_in_batches_of_at_most_fifty_lines_and_keeps_progress() {
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let mut e = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let dir = tempfile::tempdir().unwrap();
    let partial = dir.path().join("translated.partial.json");
    let texts = lines(120);
    let out = e
        .translate_batches("fr", "en", &texts, &partial, &mut |_| {}, &never())
        .await
        .unwrap();
    assert_eq!(out.len(), 120);
    assert_eq!(out[0], "[tr] ligne 0");
    assert_eq!(out[119], "[tr] ligne 119");
    assert_eq!(batch_sizes(&fake), vec![50, 50, 20]);
    assert!(partial.exists());
    let usage = e.usage();
    assert_eq!(
        (
            usage.requests,
            usage.step.as_str(),
            usage.service_id.as_str()
        ),
        (3, "translate", "google-gemini")
    );
    assert_eq!(
        usage.chars,
        texts.iter().map(|t| t.chars().count() as u64).sum::<u64>()
    );
}

#[tokio::test]
async fn a_long_line_set_is_split_by_characters_too() {
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let mut e = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let dir = tempfile::tempdir().unwrap();
    // 10 lines of 1,000 characters: at most 4,000 characters per request.
    let texts: Vec<String> = (0..10).map(|i| format!("{i}{}", "x".repeat(999))).collect();
    e.translate_batches(
        "fr",
        "en",
        &texts,
        &dir.path().join("p.json"),
        &mut |_| {},
        &never(),
    )
    .await
    .unwrap();
    assert_eq!(batch_sizes(&fake), vec![4, 4, 2]);
}

#[tokio::test]
async fn a_restart_does_not_ask_again_for_finished_batches() {
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let dir = tempfile::tempdir().unwrap();
    let partial = dir.path().join("p.json");
    let texts = lines(120);

    // Stop after the first batch (the stop check is asked before each batch).
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let stop_second = || calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 1;
    let mut e = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let err = e
        .translate_batches("fr", "en", &texts, &partial, &mut |_| {}, &stop_second)
        .await
        .unwrap_err();
    assert_eq!(err.code, CANCELLED);
    assert_eq!(batch_sizes(&fake), vec![50]);

    let mut again = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let out = again
        .translate_batches("fr", "en", &texts, &partial, &mut |_| {}, &never())
        .await
        .unwrap();
    assert_eq!(out.len(), 120);
    assert_eq!(
        batch_sizes(&fake),
        vec![50, 50, 20],
        "only the two unfinished batches were requested"
    );
    assert_eq!(again.usage().requests, 2);

    // Different text: the old partial file is ignored.
    let mut other = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let out = other
        .translate_batches("fr", "en", &lines(3), &partial, &mut |_| {}, &never())
        .await
        .unwrap();
    assert_eq!(out, vec!["[tr] ligne 0", "[tr] ligne 1", "[tr] ligne 2"]);
}

#[tokio::test]
async fn rotation_mid_run_loses_and_repeats_no_line() {
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let mut e = engine(
        &fake,
        Step::Translate,
        &[("first", K1), ("second", K2)],
        &clock,
    );
    let dir = tempfile::tempdir().unwrap();
    let texts = lines(150);
    // The first key does one batch, then is limited; a new run over all lines goes on with the
    // second key.
    let mut events = Vec::new();
    let partial = dir.path().join("p.json");
    e.translate_batches("fr", "en", &texts[..50], &partial, &mut |_| {}, &never())
        .await
        .unwrap();
    fake.set_for_key(
        K1,
        Mode::RateLimit {
            times: 1000,
            retry_after: Some(100),
        },
    );
    let mut e2 = engine(
        &fake,
        Step::Translate,
        &[("first", K1), ("second", K2)],
        &clock,
    );
    let full = dir.path().join("full.json");
    let out = e2
        .translate_batches(
            "fr",
            "en",
            &texts,
            &full,
            &mut |ev| events.push(ev),
            &never(),
        )
        .await
        .unwrap();
    assert_eq!(out.len(), 150);
    for (i, line) in out.iter().enumerate() {
        assert_eq!(
            line,
            &format!("[tr] ligne {i}"),
            "line {i} missing or out of order"
        );
    }
    // Every line was translated by a successful request exactly once.
    let usage = e2.usage();
    assert_eq!(
        usage.chars,
        texts.iter().map(|t| t.chars().count() as u64).sum::<u64>()
    );
    assert_eq!(
        usage.keys_used,
        vec!["first".to_string(), "second".to_string()]
    );
    assert_eq!(usage.switches, 1);
    assert_eq!(
        fake.count_for_key(K2),
        3,
        "the second key took the three batches"
    );
}

fn wav_ms(path: &Path) -> u64 {
    flickfluent_core::cloud::audio::wav_duration_ms(path).unwrap()
}

#[tokio::test]
async fn speech_writes_one_clip_per_line_fitted_to_its_slot_and_keeps_existing_ones() {
    let ffmpeg =
        Ffmpeg::locate(Path::new("/nonexistent")).expect("ffmpeg on PATH or FLICK_FFMPEG_DIR");
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let mut e = engine(&fake, Step::Speak, &[("a", K1)], &clock);
    let dir = tempfile::tempdir().unwrap();
    let clips = dir.path().join("clips");
    let jobs = vec![
        // The fake speaks 200 ms. A slot of 1,000 ms leaves it alone.
        SpeakJob {
            id: "s0-l0".into(),
            text: "bonjour".into(),
            language: "fr".into(),
            gender: Gender::Female,
            target_ms: 1000,
        },
        // A 100 ms slot wants 2x, capped at 1.3x: about 154 ms, so it stays too long.
        SpeakJob {
            id: "s0-l1".into(),
            text: "salut".into(),
            language: "fr".into(),
            gender: Gender::Male,
            target_ms: 100,
        },
    ];
    let out = e
        .speak_lines(&jobs, &clips, &ffmpeg, &mut |_| {}, &never())
        .await
        .unwrap();
    assert_eq!(out.len(), 2);
    assert!(
        (195..=205).contains(&out[0].audio_ms),
        "{}",
        out[0].audio_ms
    );
    assert!(
        (140..=170).contains(&out[1].audio_ms),
        "{}",
        out[1].audio_ms
    );
    assert!(
        out[1].audio_ms > 100,
        "beyond 1.3x the clip stays longer than its slot, so it is flagged"
    );
    assert_eq!(wav_ms(&clips.join("s0-l0.wav")), out[0].audio_ms);
    assert!(!clips.join("s0-l0.raw.wav").exists() && !clips.join("s0-l1.part.wav").exists());

    // The Gemini voice is the first of the speaker's gender in the curated list.
    let bodies: Vec<String> = fake.requests().iter().map(|r| r.body_text()).collect();
    assert!(
        bodies[0].contains("\"voiceName\":\"Zephyr\""),
        "{}",
        bodies[0]
    );
    assert!(
        bodies[1].contains("\"voiceName\":\"Puck\""),
        "{}",
        bodies[1]
    );

    // Run again: the clips exist, so nothing is requested.
    let before = fake.count();
    let mut again = engine(&fake, Step::Speak, &[("a", K1)], &clock);
    again
        .speak_lines(&jobs, &clips, &ffmpeg, &mut |_| {}, &never())
        .await
        .unwrap();
    assert_eq!(fake.count(), before);
    assert_eq!(again.usage().requests, 0);
}

#[tokio::test]
async fn failures_come_back_as_app_errors_with_the_service_named() {
    let fake = FakeCloud::start();
    let clock = Arc::new(FakeClock::new());
    let dir = tempfile::tempdir().unwrap();

    fake.set_default(Mode::RateLimit {
        times: 1000,
        retry_after: Some(400),
    });
    let mut e = engine(&fake, Step::Translate, &[("a", K1)], &clock);
    let err = e
        .translate_batches(
            "fr",
            "en",
            &lines(2),
            &dir.path().join("a.json"),
            &mut |_| {},
            &never(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, CLOUD_KEYS_EXHAUSTED);
    assert_eq!(err.details.as_ref().unwrap()["retryAfterSeconds"], 400);
    assert_eq!(
        err.details.as_ref().unwrap()["service"],
        "Google Gemini API"
    );

    let fake2 = FakeCloud::start();
    fake2.set_default(Mode::Reject);
    let mut e = engine(&fake2, Step::Translate, &[("mine", K1)], &clock);
    let err = e
        .translate_batches(
            "fr",
            "en",
            &lines(2),
            &dir.path().join("b.json"),
            &mut |_| {},
            &never(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, CLOUD_AUTH_FAILED);
    assert_eq!(err.details.as_ref().unwrap()["labels"], "mine");
    assert!(!serde_json::to_string(&err).unwrap().contains(K1));
}

//! The fake cloud server itself: it answers, counts, and fails on demand.

mod support;
use support::fake_cloud::{FakeCloud, Mode};

async fn get(url: &str, key: &str) -> reqwest::Result<reqwest::Response> {
    reqwest::Client::new()
        .get(url)
        .header("x-goog-api-key", key)
        .send()
        .await
}

#[tokio::test]
async fn answers_counts_and_logs_headers() {
    let fake = FakeCloud::start();
    let r = get(&fake.gemini_models_url(), "k1").await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(fake.count(), 1);
    assert_eq!(fake.requests()[0].key().map(String::as_str), Some("k1"));
}

#[tokio::test]
async fn behaviour_follows_the_key_and_rate_limit_runs_out() {
    let fake = FakeCloud::start();
    fake.set_for_key("bad", Mode::Reject);
    fake.set_for_key(
        "busy",
        Mode::RateLimit {
            times: 2,
            retry_after: Some(7),
        },
    );
    assert_eq!(
        get(&fake.gemini_models_url(), "bad")
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        get(&fake.gemini_models_url(), "good")
            .await
            .unwrap()
            .status(),
        200
    );
    let r = get(&fake.gemini_models_url(), "busy").await.unwrap();
    assert_eq!(r.status(), 429);
    assert_eq!(r.headers()["retry-after"], "7");
    assert_eq!(
        get(&fake.gemini_models_url(), "busy")
            .await
            .unwrap()
            .status(),
        429
    );
    assert_eq!(
        get(&fake.gemini_models_url(), "busy")
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(fake.count_for_key("busy"), 3);
}

#[tokio::test]
async fn a_dropped_connection_is_an_error_not_an_answer() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::Drop);
    assert!(get(&fake.gemini_models_url(), "k").await.is_err());
}

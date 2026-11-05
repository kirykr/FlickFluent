//! Several keys: move on at a limit or a refusal, wait when all are limited, keep the budget.

mod support;

use std::sync::Arc;

use flickfluent_core::cloud::adapter::{Auth, CloudAdapter};
use flickfluent_core::cloud::adapters::gemini::GeminiAdapter;
use flickfluent_core::cloud::clock::FakeClock;
use flickfluent_core::cloud::rotation::{KeyPool, PoolEvent, PoolFailure, PoolKey};
use flickfluent_core::cloud::secret::Secret;
use flickfluent_core::cloud::services::{self, Endpoints};
use support::fake_cloud::{FakeCloud, Mode};

const K1: &str = "key-one-0001";
const K2: &str = "key-two-0002";

fn adapter(url: &str) -> GeminiAdapter {
    let all = services::load().unwrap();
    let s = services::find(&all, "google-gemini")
        .unwrap()
        .clone()
        .with_endpoints(Endpoints {
            check: url.into(),
            translate: Some(url.into()),
            speak: Some(url.into()),
            ..Endpoints::default()
        });
    GeminiAdapter::new(s)
}

fn pool(clock: &Arc<FakeClock>, keys: &[(&str, &str)]) -> KeyPool {
    let keys = keys
        .iter()
        .map(|(label, secret)| PoolKey {
            label: (*label).into(),
            region: None,
            secret: Secret::new((*secret).into()),
        })
        .collect();
    KeyPool::new(keys, clock.clone(), clock.clone())
}

async fn translate(
    p: &mut KeyPool,
    a: &GeminiAdapter,
    events: &mut Vec<PoolEvent>,
) -> Result<Vec<String>, PoolFailure> {
    let texts = vec!["un".to_string(), "deux".to_string()];
    p.call(
        |key, region| {
            let (a, texts) = (a, &texts);
            async move {
                let auth = Auth {
                    key: &key,
                    region: region.as_deref(),
                };
                a.translate(&auth, "fr", "en", texts).await
            }
        },
        &mut |e| events.push(e),
    )
    .await
}

#[tokio::test]
async fn a_limited_first_key_hands_over_to_the_second_and_stays_there() {
    let fake = FakeCloud::start();
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("first", K1), ("second", K2)]);
    let mut ev = Vec::new();

    assert_eq!(translate(&mut p, &a, &mut ev).await.unwrap().len(), 2);
    assert_eq!(fake.count_for_key(K1), 1);

    fake.set_for_key(
        K1,
        Mode::RateLimit {
            times: 1000,
            retry_after: Some(100),
        },
    );
    assert_eq!(translate(&mut p, &a, &mut ev).await.unwrap().len(), 2);
    assert_eq!(
        ev,
        vec![PoolEvent::Switched {
            label: "second".into()
        }]
    );

    // The first key is limited for 100 s; the next request goes straight to the second.
    assert_eq!(translate(&mut p, &a, &mut ev).await.unwrap().len(), 2);
    assert_eq!(
        fake.count_for_key(K1),
        2,
        "first key tried once more only before it was marked limited"
    );
    assert_eq!(
        (p.switches, p.keys_used.clone()),
        (1, vec!["first".to_string(), "second".to_string()])
    );
    assert!(clock.sleeps().is_empty());
}

#[tokio::test]
async fn a_refused_key_is_set_aside_and_named_and_the_other_key_carries_on() {
    for (mode, reason) in [(Mode::Reject, "rejected"), (Mode::NoCredit, "noCredit")] {
        let fake = FakeCloud::start();
        fake.set_for_key(K1, mode);
        let a = adapter(&fake.gemini_models_url());
        let clock = Arc::new(FakeClock::new());
        let mut p = pool(&clock, &[("first", K1), ("second", K2)]);
        let mut ev = Vec::new();
        translate(&mut p, &a, &mut ev).await.unwrap();
        translate(&mut p, &a, &mut ev).await.unwrap();
        assert!(
            ev.contains(&PoolEvent::SetAside {
                label: "first".into(),
                reason: reason.into()
            }),
            "{ev:?}"
        );
        assert_eq!(
            fake.count_for_key(K1),
            1,
            "a set-aside key is not tried again in the run"
        );
    }
}

#[tokio::test]
async fn a_set_aside_key_is_ready_again_at_the_start_of_the_next_run() {
    let fake = FakeCloud::start();
    fake.set_for_key(K1, Mode::Reject);
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut ev = Vec::new();
    translate(
        &mut pool(&clock, &[("first", K1), ("second", K2)]),
        &a,
        &mut ev,
    )
    .await
    .unwrap();
    fake.set_for_key(K1, Mode::Ok);
    let mut next = pool(&clock, &[("first", K1), ("second", K2)]);
    translate(&mut next, &a, &mut ev).await.unwrap();
    assert_eq!(next.keys_used, vec!["first".to_string()]);
}

#[tokio::test]
async fn when_every_key_is_limited_it_waits_the_stated_time_and_starts_from_the_first() {
    let fake = FakeCloud::start();
    fake.set_for_key(
        K1,
        Mode::RateLimit {
            times: 1,
            retry_after: Some(90),
        },
    );
    fake.set_for_key(
        K2,
        Mode::RateLimit {
            times: 1,
            retry_after: Some(90),
        },
    );
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("first", K1), ("second", K2)]);
    let mut ev = Vec::new();
    translate(&mut p, &a, &mut ev).await.unwrap();
    assert_eq!(clock.total_slept(), 90);
    assert!(ev.contains(&PoolEvent::Waiting { seconds: 90 }) && ev.contains(&PoolEvent::Resumed));
    // A heartbeat at least every 5 s, counting down.
    let beats: Vec<u64> = ev
        .iter()
        .filter_map(|e| {
            if let PoolEvent::Waiting { seconds } = e {
                Some(*seconds)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(beats.len(), 18);
    assert!(beats.windows(2).all(|w| w[0] - w[1] == 5) && beats[0] == 90 && beats[17] == 5);
    assert!(clock.sleeps().iter().all(|s| *s <= 5));
    assert_eq!(p.waited_seconds, 90);
    assert_eq!(
        fake.count_for_key(K1),
        2,
        "after the wait the first key is tried first"
    );
}

#[tokio::test]
async fn without_a_stated_time_it_waits_thirty_seconds() {
    let fake = FakeCloud::start();
    fake.set_for_key(
        K1,
        Mode::RateLimit {
            times: 1,
            retry_after: None,
        },
    );
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("only", K1)]);
    translate(&mut p, &a, &mut Vec::new()).await.unwrap();
    assert_eq!(clock.total_slept(), 30);
}

#[tokio::test]
async fn after_ten_minutes_of_waiting_it_stops_and_says_how_long_to_wait() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::RateLimit {
        times: 1000,
        retry_after: Some(400),
    });
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("first", K1), ("second", K2)]);
    let err = translate(&mut p, &a, &mut Vec::new()).await.unwrap_err();
    assert_eq!(
        err,
        PoolFailure::Exhausted {
            retry_after_seconds: 400
        }
    );
    assert_eq!(
        clock.total_slept(),
        400,
        "no second wait: 400 + 400 is over the 600 s budget"
    );
}

#[tokio::test]
async fn every_key_refused_lists_each_label_and_reason() {
    let fake = FakeCloud::start();
    fake.set_for_key(K1, Mode::Reject);
    fake.set_for_key(K2, Mode::NoCredit);
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("first", K1), ("second", K2)]);
    let err = translate(&mut p, &a, &mut Vec::new()).await.unwrap_err();
    assert_eq!(
        err,
        PoolFailure::AuthFailed {
            keys: vec![
                ("first".into(), "rejected".into()),
                ("second".into(), "noCredit".into())
            ]
        }
    );
}

#[tokio::test]
async fn offline_and_a_bad_reply_stop_the_request() {
    let clock = Arc::new(FakeClock::new());
    let dead = adapter("http://127.0.0.1:1");
    let err = translate(&mut pool(&clock, &[("a", K1)]), &dead, &mut Vec::new())
        .await
        .unwrap_err();
    assert!(matches!(err, PoolFailure::Offline { .. }));

    let fake = FakeCloud::start();
    fake.set_default(Mode::WrongCount);
    let a = adapter(&fake.gemini_models_url());
    let err = translate(&mut pool(&clock, &[("a", K1)]), &a, &mut Vec::new())
        .await
        .unwrap_err();
    assert!(matches!(err, PoolFailure::BadReply { .. }));
}

#[tokio::test]
async fn a_server_error_is_retried_once_after_two_seconds_then_treated_as_a_limit() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::ServerError);
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("only", K1)]);
    let err = translate(&mut p, &a, &mut Vec::new()).await.unwrap_err();
    // 2 s pause, retry fails, limited for 30 s, waits 30 s, tries again: the budget runs out
    // after the waits add up past ten minutes.
    assert!(matches!(err, PoolFailure::Exhausted { .. }), "{err:?}");
    assert_eq!(clock.sleeps()[0], 2);
    // Every wait after the first pause is a 30 s limit, slept in steps of at most 5 s.
    let rest: u64 = clock.sleeps().iter().skip(1).sum();
    assert!(rest > 0 && rest.is_multiple_of(30), "{rest}");
}

#[tokio::test]
async fn no_key_has_more_than_one_request_in_flight() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::Slow(30));
    let a = adapter(&fake.gemini_models_url());
    let clock = Arc::new(FakeClock::new());
    let mut p = pool(&clock, &[("a", K1), ("b", K2)]);
    let mut ev = Vec::new();
    for _ in 0..4 {
        translate(&mut p, &a, &mut ev).await.unwrap();
    }
    assert_eq!(fake.max_in_flight_for_key(K1), 1);
    let _: &dyn CloudAdapter = &a;
}

//! Each adapter against the fake server: right headers, one result per input, every failure
//! kind mapped, the key only ever in a header and never in a message.

mod support;

use std::time::Duration;

use flickfluent_core::cloud::adapter::{Auth, CloudAdapter, FailureKind};
use flickfluent_core::cloud::adapters::{azure::AzureAdapter, gemini::GeminiAdapter};
use flickfluent_core::cloud::http::{client_with_timeout, retry_after_seconds};
use flickfluent_core::cloud::secret::Secret;
use flickfluent_core::cloud::services::{self, Endpoints, Gender, Service};
use support::fake_cloud::{FakeCloud, Mode};

const KEY: &str = "SECRET-KEY-MARKER-12345";

fn service(id: &str) -> Service {
    let all = services::load().unwrap();
    services::find(&all, id).unwrap().clone()
}

fn gemini(fake: &FakeCloud) -> GeminiAdapter {
    let url = fake.gemini_models_url();
    let s = service("google-gemini").with_endpoints(Endpoints {
        check: url.clone(),
        translate: Some(url.clone()),
        speak: Some(url),
        ..Endpoints::default()
    });
    GeminiAdapter::new(s)
}

fn azure_endpoints(fake: &FakeCloud) -> Endpoints {
    Endpoints {
        check: fake.azure_url("/sts"),
        translate: Some(fake.azure_url("")),
        speak: Some(fake.azure_url("/speak")),
        voices: Some(fake.azure_url("/voices")),
        languages: Some(fake.azure_url("/languages")),
    }
}

fn azure(fake: &FakeCloud) -> AzureAdapter {
    AzureAdapter::new(service("azure-ai").with_endpoints(azure_endpoints(fake)))
}

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("ligne {i}")).collect()
}

macro_rules! auth {
    ($secret:ident, $auth:ident) => {
        let $secret = Secret::new(KEY.to_string());
        let $auth = Auth {
            key: &$secret,
            region: Some("westeurope"),
        };
    };
}

// ---- check -----------------------------------------------------------------------------

#[tokio::test]
async fn check_succeeds_and_reports_languages_for_both_services() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    let g = gemini(&fake).check(&auth).await.unwrap();
    assert!(g.translate.contains(&"km".to_string()) && g.speak.contains(&"km".to_string()));

    let a = azure(&fake).check(&auth).await.unwrap();
    assert!(a.translate.contains(&"km".to_string()) && a.translate.contains(&"zh".to_string()));
    assert!(a.speak.contains(&"fr".to_string()) && a.speak.contains(&"km".to_string()));
    let _ = secret;
}

#[tokio::test]
async fn check_maps_every_failure_kind() {
    let cases = [
        (Mode::Reject, FailureKind::Rejected),
        (Mode::NoCredit, FailureKind::NoCredit),
        (Mode::ServerError, FailureKind::Unavailable),
        (Mode::Junk, FailureKind::Unexpected),
        (
            Mode::RateLimit {
                times: 5,
                retry_after: Some(9),
            },
            FailureKind::RateLimited,
        ),
    ];
    for (mode, want) in cases {
        let fake = FakeCloud::start();
        fake.set_default(mode.clone());
        auth!(secret, auth);
        let g = gemini(&fake).check(&auth).await.unwrap_err();
        let a = azure(&fake).check(&auth).await.unwrap_err();
        assert_eq!(g.kind, want, "gemini {mode:?}");
        assert_eq!(a.kind, want, "azure {mode:?}");
        let _ = secret;
    }
}

#[tokio::test]
async fn a_closed_port_is_offline() {
    auth!(secret, auth);
    let dead = "http://127.0.0.1:1".to_string();
    let g = GeminiAdapter::new(service("google-gemini").with_endpoints(Endpoints {
        check: dead.clone(),
        ..Endpoints::default()
    }));
    assert_eq!(g.check(&auth).await.unwrap_err().kind, FailureKind::Offline);
    let a = AzureAdapter::new(service("azure-ai").with_endpoints(Endpoints {
        check: dead,
        ..Endpoints::default()
    }));
    assert_eq!(a.check(&auth).await.unwrap_err().kind, FailureKind::Offline);
    let _ = secret;
}

#[tokio::test]
async fn a_reply_slower_than_the_timeout_is_unavailable() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::Slow(1500));
    auth!(secret, auth);
    let http = client_with_timeout(Duration::from_millis(300));
    let url = fake.gemini_models_url();
    let g = GeminiAdapter::with_client(
        service("google-gemini").with_endpoints(Endpoints {
            check: url,
            ..Endpoints::default()
        }),
        http,
    );
    assert_eq!(
        g.check(&auth).await.unwrap_err().kind,
        FailureKind::Unavailable
    );
    let _ = secret;
}

#[tokio::test]
async fn the_key_is_only_ever_in_a_header_and_never_in_a_message() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    gemini(&fake).check(&auth).await.unwrap();
    azure(&fake).check(&auth).await.unwrap();
    gemini(&fake)
        .translate(&auth, "fr", "en", &lines(2))
        .await
        .unwrap();
    for r in fake.requests() {
        assert!(!r.path.contains(KEY), "key in url {}", r.path);
        assert!(!r.body_text().contains(KEY), "key in body of {}", r.path);
        if r.path.contains("/sts")
            || r.path.contains("/gemini/models") && r.headers.contains_key("x-goog-api-key")
        {
            assert_eq!(r.key().map(String::as_str), Some(KEY));
        }
    }

    // The fake's refusal text repeats the key; the failure message must not.
    let bad = FakeCloud::start();
    bad.set_default(Mode::Reject);
    let g = gemini(&bad).check(&auth).await.unwrap_err();
    let a = azure(&bad).check(&auth).await.unwrap_err();
    for f in [g, a] {
        assert!(!f.message.contains(KEY), "key in message: {}", f.message);
        assert!(f.message.contains("[key]"));
    }
    let _ = secret;
}

// ---- translate, voices, speak ------------------------------------------------------------

#[tokio::test]
async fn translate_returns_one_string_per_input_in_order_at_the_batch_limit() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    let input = lines(50);
    for out in [
        gemini(&fake)
            .translate(&auth, "fr", "en", &input)
            .await
            .unwrap(),
        azure(&fake)
            .translate(&auth, "fr", "en", &input)
            .await
            .unwrap(),
    ] {
        assert_eq!(out.len(), 50);
        assert_eq!(out[0], "[tr] ligne 0");
        assert_eq!(out[49], "[tr] ligne 49");
    }
    let _ = secret;
}

#[tokio::test]
async fn a_reply_with_another_count_is_unexpected() {
    let fake = FakeCloud::start();
    fake.set_default(Mode::WrongCount);
    auth!(secret, auth);
    let g = gemini(&fake)
        .translate(&auth, "fr", "en", &lines(3))
        .await
        .unwrap_err();
    let a = azure(&fake)
        .translate(&auth, "fr", "en", &lines(3))
        .await
        .unwrap_err();
    assert_eq!(g.kind, FailureKind::Unexpected);
    assert_eq!(a.kind, FailureKind::Unexpected);
    let _ = secret;
}

#[tokio::test]
async fn rate_limit_and_rejection_map_and_keep_retry_after_in_seconds() {
    let fake = FakeCloud::start();
    fake.set_for_key(
        KEY,
        Mode::RateLimit {
            times: 2,
            retry_after: Some(42),
        },
    );
    auth!(secret, auth);
    let g = gemini(&fake)
        .translate(&auth, "fr", "en", &lines(1))
        .await
        .unwrap_err();
    assert_eq!(
        (g.kind, g.retry_after),
        (FailureKind::RateLimited, Some(42))
    );
    let a = azure(&fake)
        .translate(&auth, "fr", "en", &lines(1))
        .await
        .unwrap_err();
    assert_eq!(
        (a.kind, a.retry_after),
        (FailureKind::RateLimited, Some(42))
    );
    fake.set_for_key(KEY, Mode::Reject);
    let r = gemini(&fake)
        .translate(&auth, "fr", "en", &lines(1))
        .await
        .unwrap_err();
    assert_eq!(r.kind, FailureKind::Rejected);
    let _ = secret;
}

#[test]
fn retry_after_reads_seconds_and_http_dates() {
    use time::format_description::well_known::Rfc2822;
    assert_eq!(retry_after_seconds("90"), Some(90));
    assert_eq!(retry_after_seconds(" 5 "), Some(5));
    let later = time::OffsetDateTime::now_utc() + time::Duration::seconds(120);
    let seconds = retry_after_seconds(&later.format(&Rfc2822).unwrap()).unwrap();
    assert!((115..=120).contains(&seconds), "{seconds}");
    let past = time::OffsetDateTime::now_utc() - time::Duration::seconds(60);
    assert_eq!(
        retry_after_seconds(&past.format(&Rfc2822).unwrap()),
        Some(0)
    );
    assert_eq!(retry_after_seconds("soon"), None);
}

#[tokio::test]
async fn language_codes_are_mapped_inside_the_adapter() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    azure(&fake)
        .translate(&auth, "fr", "zh", &lines(1))
        .await
        .unwrap();
    azure(&fake)
        .translate(&auth, "pt", "en", &lines(1))
        .await
        .unwrap();
    gemini(&fake)
        .translate(&auth, "fr", "zh", &lines(1))
        .await
        .unwrap();
    let paths: Vec<String> = fake.requests().iter().map(|r| r.path.clone()).collect();
    assert!(
        paths.iter().any(|p| p.contains("from=fr&to=zh-Hans")),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.contains("from=pt&to=en")),
        "{paths:?}"
    );
    let body = fake.requests().last().unwrap().body_text();
    assert!(body.contains("Chinese (Simplified)"), "{body}");
    let _ = secret;
}

#[tokio::test]
async fn voices_come_from_the_curated_list_or_the_live_list() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    let g = gemini(&fake).voices(&auth, "km").await.unwrap();
    assert_eq!(g.len(), 30);
    assert!(
        g.iter().any(|v| v.gender == Gender::Female) && g.iter().any(|v| v.gender == Gender::Male)
    );
    let a = azure(&fake).voices(&auth, "km").await.unwrap();
    assert_eq!(a.len(), 2);
    assert!(a
        .iter()
        .any(|v| v.id == "km-KH-SreymomNeural" && v.gender == Gender::Female));
    assert!(a
        .iter()
        .any(|v| v.id == "km-KH-PisethNeural" && v.gender == Gender::Male));
    let _ = secret;
}

fn wav_facts(wav: &[u8]) -> (u16, u32, u16) {
    assert_eq!(&wav[..4], b"RIFF");
    (
        u16::from_le_bytes([wav[22], wav[23]]),
        u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
        u16::from_le_bytes([wav[34], wav[35]]),
    )
}

#[tokio::test]
async fn speak_returns_a_24khz_16bit_mono_wav_with_the_chosen_voice() {
    let fake = FakeCloud::start();
    auth!(secret, auth);
    let g = gemini(&fake)
        .speak(&auth, "Kore", "km", "សួស្តី")
        .await
        .unwrap();
    assert_eq!(wav_facts(&g), (1, 24000, 16));
    let req = fake.requests().last().unwrap().clone();
    assert!(
        req.body_text().contains("\"voiceName\":\"Kore\""),
        "{}",
        req.body_text()
    );

    let a = azure(&fake)
        .speak(&auth, "fr-FR-DeniseNeural", "fr", "Bonjour & <bienvenue>")
        .await
        .unwrap();
    assert_eq!(wav_facts(&a), (1, 24000, 16));
    let req = fake.requests().last().unwrap().clone();
    assert_eq!(
        req.headers["x-microsoft-outputformat"],
        "riff-24khz-16bit-mono-pcm"
    );
    assert!(req
        .body_text()
        .contains("<voice name='fr-FR-DeniseNeural'>"));
    assert!(req.body_text().contains("Bonjour &amp; &lt;bienvenue&gt;"));
    assert_eq!(req.headers["ocp-apim-subscription-region"], "westeurope");
    let _ = secret;
}

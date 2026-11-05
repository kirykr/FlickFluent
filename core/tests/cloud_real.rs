//! Real services, by hand, with your own keys. Never runs in CI.
//!
//! ```text
//! FLICK_TEST_GEMINI_KEY=...                                  Google Gemini API key
//! FLICK_TEST_AZURE_KEY=... FLICK_TEST_AZURE_REGION=westeurope  Azure AI services key and region
//! cargo test -p flickfluent-core --test cloud_real -- --ignored --nocapture
//! ```
//!
//! For each service with a key: verify it, translate three French lines into English, and speak
//! one line. It prints the languages each service reports; compare them with
//! `config/cloud-services.json` and update `checkedOn` and any entries that differ.

use flickfluent_core::cloud::adapter::Auth;
use flickfluent_core::cloud::adapters::adapter_for;
use flickfluent_core::cloud::audio::wav_duration_ms_of;
use flickfluent_core::cloud::secret::Secret;
use flickfluent_core::cloud::services::{self, Gender};

const LINES: [&str; 3] = [
    "Bonjour, je m'appelle Thomas.",
    "Aujourd'hui nous allons visiter la ville.",
    "Merci beaucoup, j'ai hâte de voir le grand musée.",
];

async fn exercise(id: &str, key: String, region: Option<String>) {
    let all = services::load().unwrap();
    let service = services::find(&all, id).unwrap();
    let adapter = adapter_for(service).expect("an adapter for the service");
    let secret = Secret::new(key);
    let auth = Auth {
        key: &secret,
        region: region.as_deref(),
    };

    let languages = adapter
        .check(&auth)
        .await
        .unwrap_or_else(|f| panic!("{id}: check failed: {f:?}"));
    println!("{id}: translate {:?}", languages.translate);
    println!("{id}: speak {:?}", languages.speak);

    let texts: Vec<String> = LINES.iter().map(|s| s.to_string()).collect();
    let out = adapter
        .translate(&auth, "fr", "en", &texts)
        .await
        .unwrap_or_else(|f| panic!("{id}: translate failed: {f:?}"));
    assert_eq!(out.len(), 3);
    for (a, b) in LINES.iter().zip(&out) {
        println!("{id}: {a}  ->  {b}");
    }

    let voices = adapter
        .voices(&auth, "en")
        .await
        .unwrap_or_else(|f| panic!("{id}: voices failed: {f:?}"));
    let voice = voices
        .iter()
        .find(|v| v.gender == Gender::Female)
        .or(voices.first())
        .expect("a voice");
    let wav = adapter
        .speak(&auth, &voice.id, "en", &out[0])
        .await
        .unwrap_or_else(|f| panic!("{id}: speak failed: {f:?}"));
    let ms = wav_duration_ms_of(&wav).expect("a WAV file");
    println!("{id}: spoke {} ms with voice {}", ms, voice.id);
    assert!(ms > 300);
}

#[tokio::test]
#[ignore = "needs real keys (see the header)"]
async fn the_real_services_verify_translate_and_speak() {
    let mut ran = false;
    if let Ok(key) = std::env::var("FLICK_TEST_GEMINI_KEY") {
        exercise("google-gemini", key, None).await;
        ran = true;
    }
    if let (Ok(key), Ok(region)) = (
        std::env::var("FLICK_TEST_AZURE_KEY"),
        std::env::var("FLICK_TEST_AZURE_REGION"),
    ) {
        exercise("azure-ai", key, Some(region)).await;
        ran = true;
    }
    assert!(
        ran,
        "set FLICK_TEST_GEMINI_KEY, or FLICK_TEST_AZURE_KEY with FLICK_TEST_AZURE_REGION"
    );
}

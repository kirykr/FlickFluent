//! SC-002: after a whole story with a key (add, verify, a cloud run with a limit and a refusal,
//! a key switch, an export, removal) the key's text is in no file, no message and no event, in
//! any of its forms.

mod support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine;
use flickfluent_core::cloud::adapters::gemini::GeminiAdapter;
use flickfluent_core::cloud::keys::KeyState;
use flickfluent_core::cloud::services::{self, Step};
use flickfluent_core::pipeline::run::{run_dub, CancelToken, Mode};
use flickfluent_core::pipeline::stages::export_subtitles;
use support::cloud_run::{cloud, gemini_endpoints, use_cloud};
use support::fake_cloud::{FakeCloud, Mode as Fake};
use support::run_fixture::{config, dub_for};

const MARKER_ONE: &str = "AIzaSy-LEAKMARKER/one+1=x";
const MARKER_TWO: &str = "AIzaSy-LEAKMARKER/two+2=y";

fn forms(secret: &str) -> Vec<String> {
    let url: String = secret
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    vec![
        secret.to_string(),
        url,
        base64::engine::general_purpose::STANDARD.encode(secret),
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(secret),
    ]
}

fn all_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            all_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn clip(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

#[tokio::test]
async fn the_key_text_is_found_nowhere_after_the_whole_story() {
    let root = tempfile::tempdir().unwrap();
    let fake = FakeCloud::start();
    let c = cloud(root.path(), &fake, &[]);

    // Add two keys and verify them through the real adapter.
    let one = c
        .book
        .add("google-gemini", "one", MARKER_ONE.into(), None, false)
        .unwrap();
    let two = c
        .book
        .add("google-gemini", "two", MARKER_TWO.into(), None, false)
        .unwrap();
    let all = services::load().unwrap();
    let service = services::find(&all, "google-gemini")
        .unwrap()
        .clone()
        .with_endpoints(gemini_endpoints(&fake));
    let adapter = GeminiAdapter::new(service);
    for id in [&one.id, &two.id] {
        assert!(c.book.verify(id, &adapter).await.unwrap().ok);
        assert_eq!(c.book.info(id).unwrap().unwrap().state, KeyState::Verified);
    }
    // A refused key's failure text (which the fake makes echo the key) must not carry it.
    fake.set_for_key(MARKER_ONE, Fake::Reject);
    let refused = c.book.verify(&one.id, &adapter).await.unwrap();
    let mut texts: Vec<String> = vec![format!("{refused:?}")];
    fake.reset();
    c.book.set_state(&one.id, KeyState::Verified).unwrap();

    // A cloud run: the first key is limited and then refused, so the run switches to the second.
    for step in [Step::Translate, Step::Speak] {
        c.consent.grant("google-gemini", step, 1_000_000).unwrap();
    }
    fake.set_for_key(
        MARKER_ONE,
        Fake::RateLimit {
            times: 1,
            retry_after: Some(30),
        },
    );
    let mut cfg = config(root.path(), "leak.flick", serde_json::json!({}));
    cfg.cloud = Some(c.runtime.clone());
    let mut dub = dub_for(&clip("clip-20s-fr.mp4"), "en").await;
    use_cloud(&mut dub, Some("google-gemini"), Some("google-gemini"));
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = events.clone();
    let out = run_dub(
        &cfg,
        dub.clone(),
        Mode::Fresh,
        CancelToken::default(),
        &mut move |e| {
            sink.lock().unwrap().push(format!("{e:?}"));
        },
    )
    .await
    .unwrap();
    texts.extend(events.lock().unwrap().iter().cloned());
    texts.push(serde_json::to_string(&out).unwrap());

    // A second run where every key is refused: the error text must not carry the keys either.
    fake.reset();
    fake.set_default(Fake::Reject);
    let mut cfg2 = config(root.path(), "leak2.flick", serde_json::json!({}));
    cfg2.cloud = Some(c.runtime.clone());
    let err = run_dub(&cfg2, dub, Mode::Fresh, CancelToken::default(), &mut |_| {})
        .await
        .unwrap_err();
    texts.push(serde_json::to_string(&err).unwrap());
    texts.push(format!("{err:?}"));
    fake.reset();

    // Export, then remove both keys.
    let export_dir = root.path().join("exports");
    std::fs::create_dir_all(&export_dir).unwrap();
    export_subtitles(&out.lines, &export_dir, "leak-test", true).unwrap();
    c.book.remove(&one.id).unwrap();
    c.book.remove(&two.id).unwrap();
    assert!(c.book.list().unwrap().is_empty());

    // Search every file under the temp folder (app data, project, work and export folders) and
    // every message for each key in each form. The fake server's own log is not a file of the app.
    let mut files = Vec::new();
    all_files(root.path(), &mut files);
    assert!(
        files.len() > 10,
        "the story should have written files, found {}",
        files.len()
    );
    for marker in [MARKER_ONE, MARKER_TWO] {
        for form in forms(marker) {
            for f in &files {
                let bytes = std::fs::read(f).unwrap();
                assert!(
                    !bytes.windows(form.len()).any(|w| w == form.as_bytes()),
                    "key text {form:?} found in {}",
                    f.display()
                );
            }
            for t in &texts {
                assert!(
                    !t.contains(&form),
                    "key text found in a message: {}",
                    &t[..t.len().min(200)]
                );
            }
        }
    }
}

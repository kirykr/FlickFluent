#![allow(dead_code)] // each test file uses a different part of this module
//! Helpers to run a dub against the fake cloud server: a book with a verified key, consent, and
//! a runtime whose adapters point at the fake.

use std::path::Path;
use std::sync::Arc;

use flickfluent_core::cloud::adapters::{azure::AzureAdapter, gemini::GeminiAdapter};
use flickfluent_core::cloud::clock::FakeClock;
use flickfluent_core::cloud::consent::ConsentBook;
use flickfluent_core::cloud::keys::{KeyBook, KeyState};
use flickfluent_core::cloud::keystore::MemoryStore;
use flickfluent_core::cloud::runtime::CloudRuntime;
use flickfluent_core::cloud::services::{self, Endpoints, Service, Step};
use flickfluent_core::project::dub::DubData;

use super::fake_cloud::FakeCloud;

pub fn gemini_endpoints(fake: &FakeCloud) -> Endpoints {
    let url = fake.gemini_models_url();
    Endpoints {
        check: url.clone(),
        translate: Some(url.clone()),
        speak: Some(url),
        ..Endpoints::default()
    }
}

pub fn azure_endpoints(fake: &FakeCloud) -> Endpoints {
    Endpoints {
        check: fake.azure_url("/sts"),
        translate: Some(fake.azure_url("")),
        speak: Some(fake.azure_url("/speak")),
        voices: Some(fake.azure_url("/voices")),
        languages: Some(fake.azure_url("/languages")),
    }
}

pub struct Cloud {
    pub runtime: Arc<CloudRuntime>,
    pub book: Arc<KeyBook>,
    pub consent: Arc<ConsentBook>,
    pub clock: Arc<FakeClock>,
}

/// Keys are `(service id, label, secret, region)`; each is added and marked verified.
pub fn cloud(root: &Path, fake: &FakeCloud, keys: &[(&str, &str, &str, Option<&str>)]) -> Cloud {
    let app_data = root.join("appdata");
    let book = Arc::new(KeyBook::new(
        app_data.clone(),
        Arc::new(MemoryStore::new()),
        services::load().unwrap(),
    ));
    for (service, label, secret, region) in keys {
        let k = book
            .add(
                service,
                label,
                (*secret).to_string(),
                region.map(str::to_string),
                false,
            )
            .unwrap();
        book.set_state(&k.id, KeyState::Verified).unwrap();
    }
    let consent = Arc::new(ConsentBook::new(app_data.clone()));
    let clock = Arc::new(FakeClock::new());
    let (g, a) = (gemini_endpoints(fake), azure_endpoints(fake));
    let runtime = CloudRuntime {
        book: book.clone(),
        consent: consent.clone(),
        app_data,
        adapters: Arc::new(move |s: &Service| match s.id.as_str() {
            "google-gemini" => Some(Arc::new(GeminiAdapter::new(
                s.clone().with_endpoints(g.clone()),
            ))),
            "azure-ai" => Some(Arc::new(AzureAdapter::new(
                s.clone().with_endpoints(a.clone()),
            ))),
            _ => None,
        }),
        clock: clock.clone(),
        sleeper: clock.clone(),
    };
    Cloud {
        runtime: Arc::new(runtime),
        book,
        consent,
        clock,
    }
}

/// Send these steps to a service for this run.
pub fn use_cloud(dub: &mut DubData, translate: Option<&str>, speak: Option<&str>) {
    let e = &mut dub.options.fallbacks.engines;
    if let Some(id) = translate {
        e.set(Step::Translate, id);
    }
    if let Some(id) = speak {
        e.set(Step::Speak, id);
    }
}

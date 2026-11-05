//! Which languages the app can work with, and what a language pair needs. Everything here is
//! derived from the model catalogue: adding a catalogue entry adds the language, with no other
//! change (FR-007, SC-005).

mod pair;

use std::collections::BTreeMap;

use serde::Serialize;

use crate::models::{CatalogueEntry, LicenceClass, Quality, Step};

pub use pair::{
    check_pair, check_pair_with_cloud, Fallback, FallbackKind, PairCheck, StepCheck, StepKind,
};

/// `pt-BR` and `pt` are the same language here.
pub fn primary(code: &str) -> String {
    code.split(['-', '_']).next().unwrap_or(code).to_lowercase()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakSupport {
    pub voices: u32,
    /// Voices whose licence is commercial-friendly.
    pub permissive_voices: u32,
}

/// What can be done with one language, and how well.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub code: String,
    pub transcribe: bool,
    pub translate_from: bool,
    pub translate_to: bool,
    pub speak: SpeakSupport,
    pub quality: Qualities,
    /// Ids of cloud services (with a verified key) that offer this language. Empty for a language
    /// the local models cover alone.
    pub cloud_services: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Qualities {
    pub transcribe: Option<Quality>,
    pub translate: Option<Quality>,
    pub speak: Option<Quality>,
}

/// An entry counts when it is on by default, or the user opted in to it.
pub(crate) fn enabled(e: &CatalogueEntry, opt_in: &[String]) -> bool {
    e.default_enabled || opt_in.contains(&e.id)
}

fn better(old: Option<Quality>, new: Option<Quality>) -> Option<Quality> {
    match (old, new) {
        (Some(a), Some(b)) => Some(if b.rank() > a.rank() { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// What one cloud service offers, for the language list and the pair check. Built from the
/// service data, or from the live list after a key was verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudOffer {
    pub service_id: String,
    pub service_name: String,
    pub translate: Vec<String>,
    pub speak: Vec<String>,
    /// The user has a verified key for this service.
    pub has_verified_key: bool,
}

/// [`list`], plus the languages of every cloud service that has a verified key. A language only a
/// service offers appears here with no change to any screen (SC-008).
pub fn list_with_cloud(
    catalogue: &[CatalogueEntry],
    opt_in: &[String],
    cloud: &[CloudOffer],
) -> Vec<Language> {
    let mut map: BTreeMap<String, Language> = list(catalogue, opt_in)
        .into_iter()
        .map(|l| (l.code.clone(), l))
        .collect();
    for offer in cloud.iter().filter(|o| o.has_verified_key) {
        let mut touch = |code: &str, translate: bool, speak: bool| {
            let code = primary(code);
            let row = map.entry(code.clone()).or_insert_with(|| Language {
                code,
                ..Default::default()
            });
            if translate {
                row.translate_from = true;
                row.translate_to = true;
            }
            if speak {
                row.speak.voices = row.speak.voices.max(1);
            }
            if !row.cloud_services.contains(&offer.service_id) {
                row.cloud_services.push(offer.service_id.clone());
            }
        };
        for l in &offer.translate {
            touch(l, true, false);
        }
        for l in &offer.speak {
            touch(l, false, true);
        }
    }
    map.into_values().collect()
}

/// Every language any enabled catalogue entry mentions, sorted by code.
pub fn list(catalogue: &[CatalogueEntry], opt_in: &[String]) -> Vec<Language> {
    let mut map: BTreeMap<String, Language> = BTreeMap::new();
    for e in catalogue.iter().filter(|e| enabled(e, opt_in)) {
        let permissive = e.licence_class == LicenceClass::Permissive;
        let mut touch = |code: &str, voices: u32, quality: Option<Quality>| {
            let code = primary(code);
            let row = map.entry(code.clone()).or_insert_with(|| Language {
                code,
                ..Default::default()
            });
            match e.step {
                Step::Transcribe => {
                    row.transcribe = true;
                    row.quality.transcribe = better(row.quality.transcribe, quality);
                }
                Step::Translate => {
                    row.translate_from = true;
                    row.translate_to = true;
                    row.quality.translate = better(row.quality.translate, quality);
                }
                Step::Speak => {
                    row.speak.voices += voices;
                    if permissive {
                        row.speak.permissive_voices += voices;
                    }
                    row.quality.speak = better(row.quality.speak, quality);
                }
                _ => {}
            }
        };
        if e.step == Step::Speak && !e.voices.is_empty() {
            for v in &e.voices {
                for l in &v.languages {
                    touch(l, 1, Some(v.quality));
                }
            }
        } else {
            for l in &e.languages {
                let q = e
                    .quality_by_language
                    .get(&primary(l))
                    .copied()
                    .or_else(|| e.quality_by_language.get(l).copied());
                touch(l, u32::from(e.step == Step::Speak), q);
            }
        }
    }
    map.into_values().collect()
}

//! What a language pair needs from each step, and what to offer when a step cannot do it.

use serde::Serialize;

use super::{enabled, primary, CloudOffer};
use crate::models::{CatalogueEntry, LicenceClass, Quality, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    Transcribe,
    Translate,
    Speak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepCheck {
    pub step: StepKind,
    pub supported: bool,
    pub quality: Option<Quality>,
    /// An i18n key saying why, when `supported` is false or the quality is low.
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FallbackKind {
    /// Do the transcript and translation, skip the speech.
    SubtitlesOnly,
    /// Read the translation with a voice of another language.
    GenericVoice,
    /// Translate in two hops, through English.
    PivotViaEnglish,
    /// Turn on a non-commercial model the user did not enable yet.
    OptInNonCommercial,
    /// Let a cloud service the user has a verified key for do the step.
    CloudEngine,
    /// A cloud service could do the step, but the user has no verified key for it yet.
    SetupCloudKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    pub kind: FallbackKind,
    pub label_key: &'static str,
    /// Models the fallback would turn on (opt-in kinds only).
    pub model_ids: Vec<String>,
    /// Licence notices to show before the user accepts, one per model.
    pub notice_keys: Vec<String>,
    /// Cloud services that would do it (`CloudEngine` and `SetupCloudKey` only).
    pub service_ids: Vec<String>,
    /// The step the fallback is for (cloud kinds only).
    pub step: Option<StepKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairCheck {
    pub ok: bool,
    pub steps: Vec<StepCheck>,
    pub fallbacks: Vec<Fallback>,
}

fn covers(e: &CatalogueEntry, code: &str) -> bool {
    let want = primary(code);
    e.languages.iter().any(|l| primary(l) == want)
}

fn voices_for(e: &CatalogueEntry, code: &str) -> bool {
    let want = primary(code);
    if e.voices.is_empty() {
        covers(e, code)
    } else {
        e.voices
            .iter()
            .any(|v| v.languages.iter().any(|l| primary(l) == want))
    }
}

fn quality_of(e: &CatalogueEntry, code: &str) -> Option<Quality> {
    e.quality_by_language.get(&primary(code)).copied()
}

/// Can one enabled model translate `from` into `to`?
fn translates<'a>(
    catalogue: &'a [CatalogueEntry],
    from: &str,
    to: &str,
    opt_in: &[String],
) -> Option<&'a CatalogueEntry> {
    catalogue
        .iter()
        .filter(|e| {
            e.step == Step::Translate && enabled(e, opt_in) && covers(e, from) && covers(e, to)
        })
        .max_by_key(|e| quality_of(e, to).map_or(1, Quality::rank))
}

fn fallback(kind: FallbackKind, label_key: &'static str) -> Fallback {
    Fallback {
        kind,
        label_key,
        model_ids: Vec::new(),
        notice_keys: Vec::new(),
        service_ids: Vec::new(),
        step: None,
    }
}

/// Check `source` (a code or `auto`) to `target`. `opt_in` lists the non-commercial entries the
/// user has turned on. The UI must show this before a dub starts.
pub fn check_pair(
    catalogue: &[CatalogueEntry],
    source: &str,
    target: &str,
    opt_in: &[String],
) -> PairCheck {
    let auto = source == "auto";
    let same = !auto && primary(source) == primary(target);
    let mut steps = Vec::new();
    let mut fallbacks = Vec::new();

    // Transcribe: the source language, or any recogniser when it is detected.
    let recognisers = catalogue
        .iter()
        .filter(|e| e.step == Step::Transcribe && enabled(e, opt_in));
    let transcribe = recognisers
        .filter(|e| auto || covers(e, source))
        .max_by_key(|e| quality_of(e, source).map_or(1, Quality::rank));
    steps.push(StepCheck {
        step: StepKind::Transcribe,
        supported: transcribe.is_some(),
        quality: transcribe.and_then(|e| quality_of(e, source)),
        reason: transcribe
            .is_none()
            .then_some("languages.reason.noRecogniser"),
    });

    // Translate: nothing to do for the same language.
    let direct = if same {
        None
    } else {
        translates(catalogue, if auto { "en" } else { source }, target, opt_in)
    };
    let translate_ok = same
        || (auto
            && catalogue
                .iter()
                .any(|e| e.step == Step::Translate && enabled(e, opt_in) && covers(e, target)))
        || direct.is_some();
    steps.push(StepCheck {
        step: StepKind::Translate,
        supported: translate_ok,
        // A pair is as good as its weaker language.
        quality: direct.and_then(|e| {
            let (a, b) = (quality_of(e, source), quality_of(e, target));
            match (a, b) {
                (Some(a), Some(b)) => Some(if a.rank() <= b.rank() { a } else { b }),
                (a, b) => a.or(b),
            }
        }),
        reason: (!translate_ok).then_some("languages.reason.noTranslator"),
    });
    if !translate_ok {
        let via = |from: &str, to: &str| translates(catalogue, from, to, opt_in).is_some();
        if primary(source) != "en"
            && primary(target) != "en"
            && via(source, "en")
            && via("en", target)
        {
            fallbacks.push(fallback(
                FallbackKind::PivotViaEnglish,
                "languages.fallback.pivot",
            ));
        }
        // A non-commercial model the user has not turned on that would do it.
        let hidden: Vec<&CatalogueEntry> = catalogue
            .iter()
            .filter(|e| {
                e.step == Step::Translate
                    && !enabled(e, opt_in)
                    && e.licence_class == LicenceClass::NonCommercial
            })
            .filter(|e| (auto || covers(e, source)) && covers(e, target))
            .collect();
        if !hidden.is_empty() {
            let mut f = fallback(FallbackKind::OptInNonCommercial, "languages.fallback.optIn");
            f.model_ids = hidden.iter().map(|e| e.id.clone()).collect();
            f.notice_keys = hidden
                .iter()
                .filter_map(|e| e.licence_notice_key.clone())
                .collect();
            fallbacks.push(f);
        }
    }

    // Speak: a voice for the target language.
    let voices = catalogue
        .iter()
        .filter(|e| e.step == Step::Speak && enabled(e, opt_in) && voices_for(e, target));
    let speak_ok = voices.count() > 0;
    steps.push(StepCheck {
        step: StepKind::Speak,
        supported: speak_ok,
        quality: None,
        reason: (!speak_ok).then_some("languages.reason.noVoice"),
    });
    if !speak_ok {
        fallbacks.push(fallback(
            FallbackKind::SubtitlesOnly,
            "languages.fallback.subtitles",
        ));
        if catalogue
            .iter()
            .any(|e| e.step == Step::Speak && enabled(e, opt_in))
        {
            fallbacks.push(fallback(
                FallbackKind::GenericVoice,
                "languages.fallback.genericVoice",
            ));
        }
        let hidden: Vec<&CatalogueEntry> = catalogue
            .iter()
            .filter(|e| {
                e.step == Step::Speak
                    && !enabled(e, opt_in)
                    && e.licence_class == LicenceClass::NonCommercial
                    && voices_for(e, target)
            })
            .collect();
        if !hidden.is_empty() {
            let mut f = fallback(FallbackKind::OptInNonCommercial, "languages.fallback.optIn");
            f.model_ids = hidden.iter().map(|e| e.id.clone()).collect();
            f.notice_keys = hidden
                .iter()
                .filter_map(|e| e.licence_notice_key.clone())
                .collect();
            // One opt-in entry covers both steps when both need it.
            if let Some(existing) = fallbacks
                .iter_mut()
                .find(|x| x.kind == FallbackKind::OptInNonCommercial)
            {
                for (id, key) in f.model_ids.iter().zip(
                    f.notice_keys
                        .iter()
                        .chain(std::iter::repeat(&String::new())),
                ) {
                    if !existing.model_ids.contains(id) {
                        existing.model_ids.push(id.clone());
                        if !key.is_empty() {
                            existing.notice_keys.push(key.clone());
                        }
                    }
                }
            } else {
                fallbacks.push(f);
            }
        }
    }

    let ok = steps.iter().all(|s| s.supported);
    PairCheck {
        ok,
        steps,
        fallbacks,
    }
}

/// [`check_pair`], plus a way forward through the cloud for each step this computer cannot do:
/// "use the cloud service" when a verified key covers the pair, or "set up a cloud key" when a
/// service covers it but no key is verified. A step the local models do can never be turned into
/// a cloud step here, and a computer that can do everything locally is never nudged.
pub fn check_pair_with_cloud(
    catalogue: &[CatalogueEntry],
    source: &str,
    target: &str,
    opt_in: &[String],
    cloud: &[CloudOffer],
) -> PairCheck {
    let mut check = check_pair(catalogue, source, target, opt_in);
    let auto = source == "auto";
    let covers_code =
        |list: &[String], code: &str| list.iter().any(|l| primary(l) == primary(code));
    for kind in [StepKind::Translate, StepKind::Speak] {
        let missing = check.steps.iter().any(|s| s.step == kind && !s.supported);
        if !missing {
            continue;
        }
        let covering: Vec<&CloudOffer> = cloud
            .iter()
            .filter(|o| match kind {
                StepKind::Translate => {
                    covers_code(&o.translate, target) && (auto || covers_code(&o.translate, source))
                }
                _ => covers_code(&o.speak, target),
            })
            .collect();
        let with_key: Vec<String> = covering
            .iter()
            .filter(|o| o.has_verified_key)
            .map(|o| o.service_id.clone())
            .collect();
        let names =
            |ids: &[&CloudOffer]| ids.iter().map(|o| o.service_id.clone()).collect::<Vec<_>>();
        let mut f = if !with_key.is_empty() {
            let mut f = fallback(FallbackKind::CloudEngine, "languages.fallback.cloudEngine");
            f.service_ids = with_key;
            f
        } else if !covering.is_empty() {
            let mut f = fallback(
                FallbackKind::SetupCloudKey,
                "languages.fallback.setupCloudKey",
            );
            f.service_ids = names(&covering);
            f
        } else {
            continue;
        };
        f.step = Some(kind);
        check.fallbacks.push(f);
    }
    check
}

//! Which models a dub needs on this machine, what is still missing, and how much disk it takes.

use std::collections::BTreeMap;

use crate::hardware::{Accelerator, MachineInput};
use crate::models::{CatalogueEntry, Gender, MachineClass, Quality, Step};
use crate::project::dub::FallbackChoices;

/// `fullLocal` needs the full-local spec (8 cores, 16 GB, a GPU or Apple Silicon), `mid` needs
/// 6 cores and 12 GB, anything else is `minimum`. Unknown values count as not meeting a tier.
pub fn machine_class(m: &MachineInput) -> MachineClass {
    let cores = m.cpu_cores.unwrap_or(0);
    let ram = m.ram_gb.unwrap_or(0.0);
    let fast = matches!(
        m.accelerator,
        Some(Accelerator::AppleSilicon | Accelerator::DiscreteGpu)
    );
    if cores >= 8 && ram >= 16.0 && fast {
        MachineClass::FullLocal
    } else if cores >= 6 && ram >= 12.0 {
        MachineClass::Mid
    } else {
        MachineClass::Minimum
    }
}

/// `pt-BR` and `pt` both match `pt`.
fn primary(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

fn covers(entry: &CatalogueEntry, language: &str) -> bool {
    let want = primary(language);
    entry
        .languages
        .iter()
        .any(|l| primary(l).eq_ignore_ascii_case(want))
}

/// On by default or opted in, and built for this platform.
fn usable(entry: &CatalogueEntry, os: &str, arch: &str, opt_in: &[String]) -> bool {
    (entry.default_enabled || opt_in.contains(&entry.id))
        && entry
            .platforms
            .as_ref()
            .is_none_or(|p| p.supports(os, arch))
}

/// The best entry of `step` for the machine. An entry the user opted into wins, since they asked
/// for it by name. Then the highest class not above the machine's. Among equals, one that is
/// already installed wins (the user should not download a second voice when
/// one works), then the first in the catalogue.
#[allow(clippy::too_many_arguments)]
fn best<'a>(
    catalogue: &'a [CatalogueEntry],
    step: Step,
    class: MachineClass,
    os: &str,
    arch: &str,
    installed: &[String],
    opt_in: &[String],
    keep: impl Fn(&CatalogueEntry) -> bool,
) -> Option<&'a CatalogueEntry> {
    catalogue
        .iter()
        .filter(|e| {
            e.step == step && e.machine_class <= class && usable(e, os, arch, opt_in) && keep(e)
        })
        .rev()
        .max_by_key(|e| {
            (
                opt_in.contains(&e.id),
                e.machine_class,
                installed.contains(&e.id),
            )
        })
}

/// A voice chosen for one speaker gender: which model holds it and how to ask for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePick {
    /// Catalogue id of the model that holds the voice.
    pub model: String,
    /// Voice of that model (`0` for a model that has one voice).
    pub speaker_id: u32,
    /// Engine language tag, empty when the model needs none.
    pub lang: String,
    /// Pronunciation list inside the model folder, when the voice needs one.
    pub lexicon: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelPlan {
    pub runtime: Option<String>,
    pub ffmpeg: Option<String>,
    pub separate: Option<String>,
    pub transcribe: Option<String>,
    pub translate: Option<String>,
    /// The model for the second hop of a translation through English, when that was chosen.
    pub pivot: Option<String>,
    pub speaker: Option<String>,
    /// Voice entry per speaker gender (`male`, `female`). A language with one voice has it under
    /// both keys.
    pub voices: BTreeMap<Gender, VoicePick>,
    /// Steps nobody can do for this language pair, as `(step, language)`.
    pub unsupported: Vec<(Step, String)>,
}

impl ModelPlan {
    /// Ids to install, each followed by the ids it `requires`, without repeats.
    pub fn required_ids(&self, catalogue: &[CatalogueEntry]) -> Vec<String> {
        let mut ids: Vec<String> = [
            &self.runtime,
            &self.ffmpeg,
            &self.separate,
            &self.transcribe,
            &self.translate,
            &self.pivot,
            &self.speaker,
        ]
        .into_iter()
        .flatten()
        .cloned()
        .chain(self.voices.values().map(|v| v.model.clone()))
        .collect();
        let mut i = 0;
        while i < ids.len() {
            if let Some(e) = catalogue.iter().find(|e| e.id == ids[i]) {
                for r in &e.requires {
                    if !ids.contains(r) {
                        ids.push(r.clone());
                    }
                }
            }
            i += 1;
        }
        let mut seen = Vec::new();
        ids.retain(|id| {
            let fresh = !seen.contains(id);
            seen.push(id.clone());
            fresh
        });
        ids
    }
}

/// Choose models for `source` to `target` (`source` is a real code here; auto-detect uses the
/// detected one, or the multilingual recogniser when none is known yet).
pub fn plan_models(
    catalogue: &[CatalogueEntry],
    class: MachineClass,
    os: &str,
    arch: &str,
    source: &str,
    target: &str,
    quality_fast: bool,
) -> ModelPlan {
    plan_models_with(
        catalogue,
        class,
        os,
        arch,
        source,
        target,
        quality_fast,
        &[],
    )
}

/// Like `plan_models`, preferring models in `installed` (ids) when several fit equally.
#[allow(clippy::too_many_arguments)]
pub fn plan_models_with(
    catalogue: &[CatalogueEntry],
    class: MachineClass,
    os: &str,
    arch: &str,
    source: &str,
    target: &str,
    quality_fast: bool,
    installed: &[String],
) -> ModelPlan {
    plan_models_full(
        catalogue,
        class,
        os,
        arch,
        source,
        target,
        quality_fast,
        installed,
        &FallbackChoices::default(),
    )
}

/// Like `plan_models_with`, also applying what the user chose for an unsupported pair.
#[allow(clippy::too_many_arguments)]
pub fn plan_models_full(
    catalogue: &[CatalogueEntry],
    class: MachineClass,
    os: &str,
    arch: &str,
    source: &str,
    target: &str,
    quality_fast: bool,
    installed: &[String],
    choices: &FallbackChoices,
) -> ModelPlan {
    let opt_in = &choices.opt_in;
    let pick = |step: Step, keep: &dyn Fn(&CatalogueEntry) -> bool| {
        best(catalogue, step, class, os, arch, installed, opt_in, keep).map(|e| e.id.clone())
    };
    let mut plan = ModelPlan {
        runtime: pick(Step::Runtime, &|_| true),
        ffmpeg: pick(Step::Ffmpeg, &|_| true),
        separate: if quality_fast {
            None
        } else {
            pick(Step::Separate, &|_| true)
        },
        transcribe: pick(Step::Transcribe, &|e| source == "auto" || covers(e, source)),
        // With `auto` the source is not known until the language is detected, so only the target
        // must be covered now; the run plans again once the language is known.
        translate: pick(Step::Translate, &|e| {
            (source == "auto" || covers(e, source)) && covers(e, target)
        }),
        speaker: pick(Step::Speaker, &|_| true),
        ..Default::default()
    };
    if plan.separate.is_none() && !quality_fast {
        plan.unsupported.push((Step::Separate, String::new()));
    }
    if plan.transcribe.is_none() {
        plan.unsupported
            .push((Step::Transcribe, source.to_string()));
    }
    if primary(source) != primary(target) && plan.translate.is_none() {
        // Through English: one model into English, one out of it (often the same model).
        let hop =
            |from: &str, to: &str| pick(Step::Translate, &|e| covers(e, from) && covers(e, to));
        if choices.pivot_via_english && source != "auto" {
            if let (Some(first), Some(second)) = (hop(source, "en"), hop("en", target)) {
                plan.translate = Some(first);
                plan.pivot = Some(second);
            }
        }
        if plan.translate.is_none() {
            plan.unsupported
                .push((Step::Translate, format!("{source}>{target}")));
        }
    }
    let mut candidates = voice_candidates(catalogue, class, os, arch, installed, opt_in, target);
    if candidates.is_empty() && choices.generic_voice {
        // A voice of another language reads the translation: the best one there is.
        candidates = voice_candidates(catalogue, class, os, arch, installed, opt_in, "");
    }
    let best_voice = |gender: Option<Gender>| {
        candidates
            .iter()
            .filter(|c| gender.is_none() || c.gender == gender)
            // Better quality first, then one already installed, then the earlier catalogue entry.
            .max_by_key(|c| (c.quality.rank(), c.installed, std::cmp::Reverse(c.order)))
            .map(|c| c.pick.clone())
    };
    for gender in [Gender::Female, Gender::Male] {
        if let Some(v) = best_voice(Some(gender)).or_else(|| best_voice(None)) {
            plan.voices.insert(gender, v);
        }
    }
    if choices.subtitles_only {
        // No speech at all: the voices are not needed and not missing.
        plan.voices.clear();
    } else if plan.voices.is_empty() {
        plan.unsupported.push((Step::Speak, target.to_string()));
    }
    // A step the user sent to a cloud service needs no local model and cannot be unsupported
    // locally. Whether the service covers the pair is checked with the service's language lists.
    use crate::cloud::services::Step as CloudStep;
    if choices.engines.cloud(CloudStep::Translate).is_some() {
        plan.translate = None;
        plan.pivot = None;
        plan.unsupported.retain(|(s, _)| *s != Step::Translate);
    }
    if choices.engines.cloud(CloudStep::Speak).is_some() {
        plan.voices.clear();
        plan.unsupported.retain(|(s, _)| *s != Step::Speak);
    }
    plan
}

struct VoiceCandidate {
    gender: Option<Gender>,
    quality: Quality,
    installed: bool,
    order: usize,
    pick: VoicePick,
}

/// Every voice that can speak `target` on this machine: each voice of a multi-voice model, and
/// each single-voice model.
fn voice_candidates(
    catalogue: &[CatalogueEntry],
    class: MachineClass,
    os: &str,
    arch: &str,
    installed: &[String],
    opt_in: &[String],
    target: &str,
) -> Vec<VoiceCandidate> {
    let want = primary(target).to_lowercase();
    let mut out = Vec::new();
    for (order, e) in catalogue.iter().enumerate() {
        if e.step != Step::Speak || e.machine_class > class || !usable(e, os, arch, opt_in) {
            continue;
        }
        let is_installed = installed.contains(&e.id);
        // An empty `target` means any language (the generic-voice fallback).
        let any = target.is_empty();
        if e.voices.is_empty() {
            if any || covers(e, target) {
                out.push(VoiceCandidate {
                    gender: e.gender,
                    quality: e
                        .quality_by_language
                        .get(&want)
                        .copied()
                        .unwrap_or(Quality::Fair),
                    installed: is_installed,
                    order,
                    pick: VoicePick {
                        model: e.id.clone(),
                        speaker_id: 0,
                        lang: String::new(),
                        lexicon: None,
                    },
                });
            }
            continue;
        }
        for v in e.voices.iter().filter(|v| {
            any || v
                .languages
                .iter()
                .any(|l| primary(l).eq_ignore_ascii_case(&want))
        }) {
            out.push(VoiceCandidate {
                gender: Some(v.gender),
                quality: v.quality,
                installed: is_installed,
                order,
                pick: VoicePick {
                    model: e.id.clone(),
                    speaker_id: v.speaker_id,
                    lang: v.lang.clone(),
                    lexicon: v.lexicon.clone(),
                },
            });
        }
    }
    out
}

/// Bytes the work folder will need: about three times the stereo 44.1 kHz source audio (full
/// audio, vocals and music, plus the mix) and the synthesized clips (speech is under half of a
/// movie, mono 22 kHz).
pub fn disk_needed_bytes(duration_ms: u64) -> u64 {
    let secs = duration_ms / 1000 + 1;
    3 * secs * 176_400 + secs * 44_100 / 2
}

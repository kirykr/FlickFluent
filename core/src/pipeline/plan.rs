//! Which models a dub needs on this machine, what is still missing, and how much disk it takes.

use std::collections::BTreeMap;

use crate::hardware::{Accelerator, MachineInput};
use crate::models::{CatalogueEntry, Gender, MachineClass, Step};

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

fn usable(entry: &CatalogueEntry, os: &str, arch: &str) -> bool {
    entry.default_enabled
        && entry
            .platforms
            .as_ref()
            .is_none_or(|p| p.supports(os, arch))
}

/// The best entry of `step` for the machine: the highest class not above the machine's. Among
/// equals, one that is already installed wins (the user should not download a second voice when
/// one works), then the first in the catalogue.
fn best<'a>(
    catalogue: &'a [CatalogueEntry],
    step: Step,
    class: MachineClass,
    os: &str,
    arch: &str,
    installed: &[String],
    keep: impl Fn(&CatalogueEntry) -> bool,
) -> Option<&'a CatalogueEntry> {
    catalogue
        .iter()
        .filter(|e| e.step == step && e.machine_class <= class && usable(e, os, arch) && keep(e))
        .rev()
        .max_by_key(|e| (e.machine_class, installed.contains(&e.id)))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelPlan {
    pub runtime: Option<String>,
    pub ffmpeg: Option<String>,
    pub separate: Option<String>,
    pub transcribe: Option<String>,
    pub translate: Option<String>,
    pub speaker: Option<String>,
    /// Voice entry per speaker gender (`male`, `female`). A language with one voice has it under
    /// both keys.
    pub voices: BTreeMap<Gender, String>,
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
            &self.speaker,
        ]
        .into_iter()
        .flatten()
        .cloned()
        .chain(self.voices.values().cloned())
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
    let pick = |step: Step, keep: &dyn Fn(&CatalogueEntry) -> bool| {
        best(catalogue, step, class, os, arch, installed, keep).map(|e| e.id.clone())
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
        translate: pick(Step::Translate, &|e| covers(e, source) && covers(e, target)),
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
        plan.unsupported
            .push((Step::Translate, format!("{source}>{target}")));
    }
    let voice = |gender: Gender| {
        pick(Step::Speak, &|e| {
            covers(e, target) && e.gender == Some(gender)
        })
    };
    let any = pick(Step::Speak, &|e| covers(e, target));
    for gender in [Gender::Female, Gender::Male] {
        if let Some(id) = voice(gender).or_else(|| any.clone()) {
            plan.voices.insert(gender, id);
        }
    }
    if plan.voices.is_empty() {
        plan.unsupported.push((Step::Speak, target.to_string()));
    }
    plan
}

/// Bytes the work folder will need: about three times the stereo 44.1 kHz source audio (full
/// audio, vocals and music, plus the mix) and the synthesized clips (speech is under half of a
/// movie, mono 22 kHz).
pub fn disk_needed_bytes(duration_ms: u64) -> u64 {
    let secs = duration_ms / 1000 + 1;
    3 * secs * 176_400 + secs * 44_100 / 2
}

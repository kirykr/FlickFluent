//! Before a dub starts: which models are missing, whether the disk is big enough, whether this
//! computer can run every step. Nothing is downloaded here.

use std::path::Path;

use serde::Serialize;

use super::plan::{disk_needed_bytes, plan_models, ModelPlan};
use crate::cloud::consent::Reason;
use crate::cloud::estimate::{estimate_chars, read_floor};
use crate::cloud::keys::KeyState;
use crate::cloud::runtime::CloudRuntime;
use crate::cloud::services::Step as CloudStep;
use crate::error::{
    AppError, CLOUD_CONSENT_REQUIRED, CLOUD_NO_KEY, MACHINE_TOO_WEAK, MODELS_MISSING,
    NO_FALLBACK_CHOSEN, NO_SPACE,
};
use crate::models::{free_space, installed, CatalogueEntry, MachineClass, Step};
use crate::project::dub::DubData;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MissingModel {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    pub missing_models: Vec<MissingModel>,
    /// Size of everything in `missing_models`.
    pub download_bytes: u64,
    /// What the work folder needs for this video.
    pub disk_needed_bytes: u64,
    pub disk_free_bytes: Option<u64>,
    pub machine_class: MachineClass,
    /// i18n keys for things worth knowing, such as no music separation.
    pub warnings: Vec<String>,
    /// Steps that cannot run on this computer, as `{step, reason}`.
    pub blockers: Vec<Blocker>,
    /// One entry per step that will use a cloud service. Empty when everything is local. Filled
    /// by [`cloud_preflight`].
    #[serde(default)]
    pub cloud: Vec<CloudPreflight>,
}

/// What a cloud step will do, so the screen can show the amount and ask first.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CloudPreflight {
    pub step: CloudStep,
    pub service_id: String,
    pub service_name: String,
    /// About how many characters this step will send.
    pub estimated_chars: u64,
    pub needs_consent: bool,
    /// `first`, `noticeChanged` or `largerRun` when `needs_consent`.
    pub reason: Option<Reason>,
    /// Whether the service has a verified key.
    pub has_key: bool,
}

/// The cloud part of the preflight for a configured dub.
pub fn cloud_preflight(
    dub: &DubData,
    rt: Option<&CloudRuntime>,
    work: &std::path::Path,
) -> Result<Vec<CloudPreflight>, AppError> {
    let engines = &dub.options.fallbacks.engines;
    let base = estimate_chars(dub.source.as_ref().map(|s| s.duration_ms).unwrap_or(0));
    let mut out = Vec::new();
    for step in [CloudStep::Translate, CloudStep::Speak] {
        if step == CloudStep::Speak && dub.options.fallbacks.subtitles_only {
            continue;
        }
        let Some(id) = engines.cloud(step) else {
            continue;
        };
        // An earlier try that outgrew its agreement left the amount it projected.
        let estimate = base.max(read_floor(work, step, id));
        let name = rt
            .and_then(|r| crate::cloud::services::find(r.book.services(), id))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| id.to_string());
        let (reason, has_key) = match rt {
            Some(r) => (
                r.consent.needs(id, step, estimate)?,
                r.book
                    .for_service(id)?
                    .iter()
                    .any(|k| k.state == KeyState::Verified),
            ),
            None => (Some(Reason::First), false),
        };
        out.push(CloudPreflight {
            step,
            service_id: id.to_string(),
            service_name: name,
            estimated_chars: estimate,
            needs_consent: reason.is_some(),
            reason,
            has_key,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    pub step: Step,
    /// `machine` or `language`.
    pub reason: &'static str,
    pub detail: String,
}

#[allow(clippy::too_many_arguments)]
fn blockers(
    catalogue: &[CatalogueEntry],
    plan: &ModelPlan,
    class: MachineClass,
    os: &str,
    arch: &str,
    source: &str,
    target: &str,
    fast: bool,
) -> Vec<Blocker> {
    // A step is blocked by the machine if a stronger machine could run it.
    let strongest = plan_models(
        catalogue,
        MachineClass::FullLocal,
        os,
        arch,
        source,
        target,
        fast,
    );
    plan.unsupported
        .iter()
        .map(|(step, detail)| {
            let machine = !strongest.unsupported.iter().any(|(s, _)| s == step)
                && class != MachineClass::FullLocal;
            Blocker {
                step: *step,
                reason: if machine { "machine" } else { "language" },
                detail: detail.clone(),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn preflight(
    catalogue: &[CatalogueEntry],
    plan: &ModelPlan,
    models_dir: &Path,
    work_dir: &Path,
    class: MachineClass,
    os: &str,
    arch: &str,
    source: &str,
    target: &str,
    fast: bool,
    duration_ms: u64,
) -> Preflight {
    let have: Vec<String> = installed(models_dir).into_iter().map(|m| m.id).collect();
    let missing_models: Vec<MissingModel> = plan
        .required_ids(catalogue)
        .into_iter()
        .filter(|id| !have.contains(id))
        .filter_map(|id| catalogue.iter().find(|e| e.id == id))
        .map(|e| MissingModel {
            id: e.id.clone(),
            name: e.name.clone(),
            size_bytes: e.total_download_bytes(),
        })
        .collect();
    let mut warnings = Vec::new();
    if fast {
        warnings.push("dub.warn.fastNoMusic".to_string());
    }
    Preflight {
        download_bytes: missing_models.iter().map(|m| m.size_bytes).sum(),
        missing_models,
        disk_needed_bytes: disk_needed_bytes(duration_ms),
        disk_free_bytes: free_space(work_dir),
        machine_class: class,
        warnings,
        blockers: blockers(catalogue, plan, class, os, arch, source, target, fast),
        cloud: Vec::new(),
    }
}

/// The cloud part of `check_start`: a key for every cloud step, and consent for each.
pub fn check_cloud(cloud: &[CloudPreflight]) -> Result<(), AppError> {
    if cloud.iter().any(|c| !c.has_key) {
        return Err(AppError::new(CLOUD_NO_KEY, "dub.error.cloudNoKey").with_details(
            serde_json::json!({ "steps": cloud.iter().filter(|c| !c.has_key).map(|c| c.step.as_str()).collect::<Vec<_>>() }),
        ));
    }
    if cloud.iter().any(|c| c.needs_consent) {
        return Err(AppError::new(CLOUD_CONSENT_REQUIRED, "dub.error.cloudConsentRequired").with_details(
            serde_json::json!({ "steps": cloud.iter().filter(|c| c.needs_consent).map(|c| c.step.as_str()).collect::<Vec<_>>() }),
        ));
    }
    Ok(())
}

/// The error `dub_start` gives, or `Ok` when the run may begin.
pub fn check_start(pre: &Preflight) -> Result<(), AppError> {
    let machine: Vec<String> = pre
        .blockers
        .iter()
        .filter(|b| b.reason == "machine")
        .map(|b| format!("{:?}", b.step).to_lowercase())
        .collect();
    check_cloud(&pre.cloud)?;
    if !machine.is_empty() {
        return Err(AppError::new(MACHINE_TOO_WEAK, "dub.error.machineTooWeak")
            .with_details(serde_json::json!({ "missing": machine })));
    }
    if !pre.blockers.is_empty() {
        return Err(AppError::new(NO_FALLBACK_CHOSEN, "dub.error.noFallbackChosen")
            .with_details(serde_json::json!({ "steps": pre.blockers.iter().map(|b| format!("{:?}", b.step).to_lowercase()).collect::<Vec<_>>() })));
    }
    if !pre.missing_models.is_empty() {
        return Err(AppError::new(MODELS_MISSING, "dub.error.modelsMissing")
            .with_details(serde_json::json!({ "missing": pre.missing_models.iter().map(|m| m.id.clone()).collect::<Vec<_>>() })));
    }
    if let Some(free) = pre.disk_free_bytes {
        if free < pre.disk_needed_bytes {
            return Err(AppError::new(NO_SPACE, "dub.error.noSpace").with_details(
                serde_json::json!({ "neededBytes": pre.disk_needed_bytes, "freeBytes": free }),
            ));
        }
    }
    Ok(())
}

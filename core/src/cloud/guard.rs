//! The amount guard and the usage merge, shared by a dub run and the editor (research R18).
//! Plain functions, so the same rule holds wherever text is about to go to a cloud service.

use std::path::Path;

use serde_json::json;

use super::consent::ConsentBook;
use super::estimate::{project_chars, write_floor};
use super::services::Step;
use crate::error::{AppError, CLOUD_CONSENT_REQUIRED};
use crate::project::dub::CloudUsage;

/// Stop before a send that would take the step past what the user agreed to. The amount shown
/// beforehand is only an estimate; this is the check against the real text. Nothing is sent when
/// it fails. The projected total for the whole video is kept in the work folder, so the next
/// preflight asks for that amount and the work goes on from where it stopped.
///
/// `used` is what the step has sent so far, `about_to_send` the characters of the next request
/// batch, `section_end_ms` how far into the video the work has got and `duration_ms` the length
/// of the video. With no agreement on record the guard passes: consent was checked before.
#[allow(clippy::too_many_arguments)]
pub fn guard_amount(
    consent: &ConsentBook,
    work: &Path,
    step: Step,
    service_id: &str,
    used: u64,
    about_to_send: u64,
    section_end_ms: u64,
    duration_ms: u64,
) -> Result<(), AppError> {
    let Some(allowed) = consent.allowed_chars(service_id, step)? else {
        return Ok(());
    };
    let total = used + about_to_send;
    if total <= allowed {
        return Ok(());
    }
    let projected = project_chars(total, section_end_ms, duration_ms);
    write_floor(work, step, service_id, projected);
    Err(
        AppError::new(CLOUD_CONSENT_REQUIRED, "dub.error.cloudConsentRequired").with_details(
            json!({
                "steps": [{ "step": step.as_str(), "serviceId": service_id }],
                "projectedChars": projected,
            }),
        ),
    )
}

/// Characters this step has sent to this service so far.
pub fn used_chars(usage: &[CloudUsage], step: Step, service_id: &str) -> u64 {
    usage
        .iter()
        .find(|u| u.step == step.as_str() && u.service_id == service_id)
        .map(|u| u.chars)
        .unwrap_or(0)
}

/// Add `add` (what engines used since they were made) to `base` (what the project already had).
/// A step and service that `base` does not have yet starts a record under `run_id`.
pub fn merge_usage(base: Vec<CloudUsage>, add: Vec<CloudUsage>, run_id: &str) -> Vec<CloudUsage> {
    let mut out = base;
    for u in add {
        match out
            .iter_mut()
            .find(|b| b.step == u.step && b.service_id == u.service_id)
        {
            Some(b) => {
                b.chars += u.chars;
                b.requests += u.requests;
                b.switches += u.switches;
                b.waited_seconds += u.waited_seconds;
                for k in u.keys_used {
                    if !b.keys_used.contains(&k) {
                        b.keys_used.push(k);
                    }
                }
            }
            None => out.push(CloudUsage {
                run_id: run_id.to_string(),
                ..u
            }),
        }
    }
    out
}

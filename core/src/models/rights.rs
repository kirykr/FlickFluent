//! Rights records for voices the project trains itself (`config/voice-rights/<id>.json`).
//!
//! A voice may be shipped only when someone with the right to say so has allowed training a
//! model on the recordings and shipping it, and every person whose voice it is has agreed in
//! writing (Constitution VII). Recordings being public or easy to find is not permission, so
//! `assumed-public` is a value the file can hold and this check rejects.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightsRecord {
    pub voice_id: String,
    pub status: String,
    pub licence: Licence,
    pub covers: Vec<String>,
    pub rights_holders: Vec<Holder>,
    pub speakers: Vec<Speaker>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Licence {
    pub class: String,
    #[serde(default)]
    pub spdx: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    pub evidence_kind: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Speaker {
    pub consent: String,
    pub evidence: String,
}

/// Why a voice cannot ship. Each is a separate, testable reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    NotGranted,
    CannotTrain,
    CannotShip,
    NoRightsHolder,
    /// A holder rests on "it is public" instead of a licence, contract or message.
    AssumedPublic,
    NoSpeakers,
    SpeakerWithoutConsent,
    /// Permission does not cover commercial use, but the record calls the voice permissive.
    NonCommercialNotMarked,
    /// A share-alike licence must be shipped with a notice, not as a plain permissive voice.
    ShareAlikeNotMarked,
}

/// Every reason this record does not allow shipping the voice. Empty means it does.
pub fn check(r: &RightsRecord) -> Vec<Problem> {
    let mut out = Vec::new();
    if r.status != "granted" {
        out.push(Problem::NotGranted);
    }
    if !r.covers.iter().any(|c| c == "train") {
        out.push(Problem::CannotTrain);
    }
    if !r.covers.iter().any(|c| c == "ship") {
        out.push(Problem::CannotShip);
    }
    if r.rights_holders.is_empty()
        || r.rights_holders
            .iter()
            .any(|h| h.evidence.trim().is_empty())
    {
        out.push(Problem::NoRightsHolder);
    }
    if r.rights_holders
        .iter()
        .any(|h| h.evidence_kind == "assumed-public")
    {
        out.push(Problem::AssumedPublic);
    }
    if r.speakers.is_empty() {
        out.push(Problem::NoSpeakers);
    } else if r
        .speakers
        .iter()
        .any(|s| s.consent == "none" || s.evidence.trim().is_empty())
    {
        out.push(Problem::SpeakerWithoutConsent);
    }
    let commercial = r.covers.iter().any(|c| c == "commercial");
    if !commercial && r.licence.class != "nonCommercial" {
        out.push(Problem::NonCommercialNotMarked);
    }
    let share_alike = r
        .licence
        .spdx
        .as_deref()
        .is_some_and(|s| s.to_ascii_uppercase().contains("-SA"));
    if share_alike && r.licence.class == "permissive" {
        out.push(Problem::ShareAlikeNotMarked);
    }
    out
}

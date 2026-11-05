use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::keys::now_rfc3339;
use super::services::Step;
use super::{cloud_consent_path, read_json, write_json};
use crate::error::AppError;

/// Bump when the notice text changes, so everyone is asked again.
pub const NOTICE_VERSION: u32 = 1;

/// A run asks again only when it will send more than this many times the agreed amount.
const ASK_AGAIN_FACTOR: u64 = 5;

/// The user's agreement to send one kind of data to one service for one step. Kept on this
/// computer only, never in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsentRecord {
    pub service_id: String,
    pub step: Step,
    pub agreed_at: String,
    /// The amount shown when the user agreed (an estimate).
    pub agreed_chars: u64,
    pub notice_version: u32,
}

/// Why the app must ask before sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// No agreement yet (including after a withdrawal, which removes it).
    First,
    NoticeChanged,
    LargerRun,
}

#[derive(Default, Serialize, Deserialize)]
struct ConsentFile {
    records: Vec<ConsentRecord>,
}

pub struct ConsentBook {
    app_data: PathBuf,
}

impl ConsentBook {
    pub fn new(app_data: PathBuf) -> Self {
        Self { app_data }
    }

    fn read(&self) -> Result<ConsentFile, AppError> {
        read_json(&cloud_consent_path(&self.app_data))
    }

    pub fn list(&self) -> Result<Vec<ConsentRecord>, AppError> {
        Ok(self.read()?.records)
    }

    /// `Some(reason)` when the app must show the notice before this run sends `estimated_chars`.
    pub fn needs(
        &self,
        service_id: &str,
        step: Step,
        estimated_chars: u64,
    ) -> Result<Option<Reason>, AppError> {
        let file = self.read()?;
        let Some(rec) = file
            .records
            .iter()
            .find(|r| r.service_id == service_id && r.step == step)
        else {
            return Ok(Some(Reason::First));
        };
        if rec.notice_version < NOTICE_VERSION {
            return Ok(Some(Reason::NoticeChanged));
        }
        if estimated_chars > rec.agreed_chars.saturating_mul(ASK_AGAIN_FACTOR) {
            return Ok(Some(Reason::LargerRun));
        }
        Ok(None)
    }

    /// Record the agreement. A new one replaces the old, including its amount.
    pub fn grant(
        &self,
        service_id: &str,
        step: Step,
        estimated_chars: u64,
    ) -> Result<ConsentRecord, AppError> {
        let mut file = self.read()?;
        file.records
            .retain(|r| !(r.service_id == service_id && r.step == step));
        let rec = ConsentRecord {
            service_id: service_id.to_string(),
            step,
            agreed_at: now_rfc3339(),
            agreed_chars: estimated_chars,
            notice_version: NOTICE_VERSION,
        };
        file.records.push(rec.clone());
        write_json(&cloud_consent_path(&self.app_data), &file)?;
        Ok(rec)
    }

    pub fn withdraw(&self, service_id: &str, step: Step) -> Result<(), AppError> {
        let mut file = self.read()?;
        file.records
            .retain(|r| !(r.service_id == service_id && r.step == step));
        write_json(&cloud_consent_path(&self.app_data), &file)
    }
}

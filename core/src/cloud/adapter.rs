use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use super::secret::Secret;
use super::services::Gender;

/// Why a cloud call failed. The rest of the code learns about a problem only through this
/// (`specs/003-hybrid-cloud-keys/contracts/cloud-adapter.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    RateLimited,
    Rejected,
    NoCredit,
    Unavailable,
    Offline,
    Unexpected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudFailure {
    pub kind: FailureKind,
    /// Seconds the service asked us to wait.
    pub retry_after: Option<u64>,
    /// Already passed through `scrub`: it never holds a key.
    pub message: String,
}

impl CloudFailure {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            retry_after: None,
            message: message.into(),
        }
    }

    pub fn retry_after(mut self, seconds: Option<u64>) -> Self {
        self.retry_after = seconds;
        self
    }
}

/// A key and, for services that need one, its region.
pub struct Auth<'a> {
    pub key: &'a Secret,
    pub region: Option<&'a str>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Languages {
    pub translate: Vec<String>,
    pub speak: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub gender: Gender,
    /// App language code (`km`, `fr`), not the vendor's.
    pub language: String,
}

pub type CloudResult<'a, T> = Pin<Box<dyn Future<Output = Result<T, CloudFailure>> + Send + 'a>>;

/// One implementation per service. Each method sends only what it is given.
pub trait CloudAdapter: Send + Sync {
    /// Prove the key works and report the languages the service offers.
    fn check<'a>(&'a self, auth: &'a Auth<'a>) -> CloudResult<'a, Languages>;

    /// One translation per input, in order. A reply with another count is `Unexpected`.
    fn translate<'a>(
        &'a self,
        auth: &'a Auth<'a>,
        source: &'a str,
        target: &'a str,
        texts: &'a [String],
    ) -> CloudResult<'a, Vec<String>>;

    /// The voices that can speak `language` (the curated list from the service data, or live).
    fn voices<'a>(&'a self, auth: &'a Auth<'a>, language: &'a str) -> CloudResult<'a, Vec<Voice>>;

    /// WAV bytes, 24 kHz, 16-bit, mono.
    fn speak<'a>(
        &'a self,
        auth: &'a Auth<'a>,
        voice: &'a str,
        language: &'a str,
        text: &'a str,
    ) -> CloudResult<'a, Vec<u8>>;
}

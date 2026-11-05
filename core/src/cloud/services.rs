use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, INVALID_SETTING};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Translate,
    Speak,
}

impl Step {
    pub fn as_str(self) -> &'static str {
        match self {
            Step::Translate => "translate",
            Step::Speak => "speak",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Gender {
    Female,
    Male,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceEntry {
    pub id: String,
    pub gender: Gender,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataNotice {
    pub translate: Option<String>,
    pub speak: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    pub check: String,
    pub translate: Option<String>,
    pub speak: Option<String>,
    pub voices: Option<String>,
    pub languages: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLanguages {
    #[serde(default)]
    pub translate: Vec<String>,
    #[serde(default)]
    pub speak: Vec<String>,
    pub checked_on: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Models {
    pub translate: Option<String>,
    pub speak: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub batch_lines: usize,
    pub batch_chars: usize,
}

/// One cloud service, described in `config/cloud-services.json`. Adding one is a data entry plus
/// an adapter; no screen changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub name: String,
    pub steps: Vec<Step>,
    pub needs_region: bool,
    pub terms_url: String,
    pub data_notice: DataNotice,
    pub endpoints: Endpoints,
    pub languages: ServiceLanguages,
    #[serde(default)]
    pub models: Models,
    #[serde(default)]
    pub voices: Option<Vec<VoiceEntry>>,
    pub limits: Limits,
    #[serde(default, rename = "_comment", skip_serializing)]
    pub comment: Option<String>,
}

impl Service {
    pub fn offers(&self, step: Step) -> bool {
        self.steps.contains(&step)
    }

    /// An endpoint with `{region}` replaced by the key's region.
    pub fn url(&self, base: &str, region: Option<&str>) -> String {
        base.replace("{region}", region.unwrap_or_default())
    }

    /// Point every endpoint at another base (tests use a local fake server). Each endpoint keeps
    /// its path below `prefix`, which the fake server decides.
    pub fn with_endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }
}

pub fn load() -> Result<Vec<Service>, AppError> {
    parse(include_str!("../../../config/cloud-services.json"))
}

pub fn parse(text: &str) -> Result<Vec<Service>, AppError> {
    let bad = |why: String| {
        AppError::new(INVALID_SETTING, "error.invalidSetting")
            .with_details(serde_json::json!({ "detail": format!("cloud-services.json: {why}") }))
    };
    let services: Vec<Service> = serde_json::from_str(text).map_err(|e| bad(e.to_string()))?;
    if services.is_empty() {
        return Err(bad("no service".into()));
    }
    let mut ids = HashSet::new();
    for s in &services {
        if !ids.insert(s.id.clone()) {
            return Err(bad(format!("duplicate id {}", s.id)));
        }
        if s.steps.is_empty() {
            return Err(bad(format!("{} offers no step", s.id)));
        }
        if s.limits.batch_lines == 0 || s.limits.batch_chars == 0 {
            return Err(bad(format!("{} has an empty batch limit", s.id)));
        }
    }
    Ok(services)
}

pub fn find<'a>(services: &'a [Service], id: &str) -> Option<&'a Service> {
    services.iter().find(|s| s.id == id)
}

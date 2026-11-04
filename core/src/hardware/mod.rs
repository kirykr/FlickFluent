//! Hardware check: probe the machine, compare with the one spec definition, pick a mode.

mod evaluate;
mod message;
mod probe;

pub use evaluate::evaluate;
pub use message::blocked_message;
pub use probe::probe;

use serde::{Deserialize, Serialize};

/// The minimum and full-local spec. Loaded from `config/spec-definition.json` (single source).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecDefinition {
    pub version: u32,
    pub provisional: bool,
    pub minimum: SpecTier,
    pub full_local: SpecTier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecTier {
    pub cpu_bits: u32,
    pub cpu_cores: u32,
    pub ram_gb: f64,
    pub free_disk_gb: f64,
    pub os: OsRequirement,
    pub accelerator: AcceleratorRequirement,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRequirement {
    pub windows: String,
    pub macos: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AcceleratorRequirement {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "appleSilicon_or_discreteGpu")]
    AppleSiliconOrDiscreteGpu,
}

impl SpecDefinition {
    pub fn bundled() -> Self {
        serde_json::from_str(include_str!("../../../config/spec-definition.json"))
            .expect("bundled spec-definition.json is valid")
    }
}

/// What was read from the machine. Every field can be missing; missing means "unknown".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineInput {
    pub cpu_bits: Option<u32>,
    pub cpu_cores: Option<u32>,
    pub ram_gb: Option<f64>,
    pub free_disk_gb: Option<f64>,
    pub os: Option<OsInfo>,
    pub accelerator: Option<Accelerator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// `windows`, `macos` or anything else the platform reports.
    pub family: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accelerator {
    None,
    AppleSilicon,
    DiscreteGpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    FullLocal,
    Hybrid,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemStatus {
    Ok,
    BelowMinimum,
    BelowFull,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckItem {
    pub key: String,
    pub required: String,
    pub detected: String,
    pub status: ItemStatus,
    /// Used to build the blocked message, e.g. `8 GB RAM`.
    pub needed_label: String,
    /// Used to build the blocked message, e.g. `4 GB`.
    pub found_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCapabilityReport {
    pub items: Vec<CheckItem>,
    pub mode: Mode,
    pub missing: Vec<CheckItem>,
    pub blocked_message: Option<String>,
    /// True while the spec numbers have not been benchmarked yet.
    pub provisional: bool,
    pub generated_at: String,
}

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What a catalogue entry is for. `Test` marks the download self-test file from feature 001.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Runtime,
    Ffmpeg,
    Separate,
    Transcribe,
    Translate,
    Speak,
    Vad,
    Speaker,
    /// Support files other entries need, such as pronunciation data. Not a pipeline step.
    Data,
    #[default]
    Test,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LicenceClass {
    #[default]
    Permissive,
    CopyleftFile,
    NonCommercial,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MachineClass {
    #[default]
    Minimum,
    Mid,
    FullLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    Good,
    Fair,
    Weak,
}

/// One file of a multi-file model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSpec {
    /// Name on disk inside the model folder.
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platforms {
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
}

impl Platforms {
    /// `os` is `windows` or `macos`, `arch` is `x64` or `arm64`.
    pub fn supports(&self, os: &str, arch: &str) -> bool {
        let list = match os {
            "windows" => &self.windows,
            "macos" => &self.macos,
            _ => return false,
        };
        list.iter().any(|a| a == arch)
    }
}

/// A model the app can download. Everything the user needs to decide is here, and it is shown
/// before any download starts (FR-016).
///
/// A single-file entry uses `url` and `sha256`. A multi-file entry lists `files` instead and
/// `size_bytes` is their sum.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
    /// Stable key.
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub licence: String,
    pub size_bytes: u64,
    /// Bytes fetched after the files, such as the packages a runtime pack installs.
    #[serde(default)]
    pub extra_download_bytes: u64,
    #[serde(default)]
    pub url: String,
    /// SHA-256 of the finished file, lower-case hex.
    #[serde(default)]
    pub sha256: String,
    /// File name on disk when it should differ from the last part of `url`.
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub files: Vec<FileSpec>,
    #[serde(default)]
    pub licence_class: LicenceClass,
    #[serde(default)]
    pub licence_notice_key: Option<String>,
    #[serde(default)]
    pub step: Step,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub gender: Option<Gender>,
    #[serde(default)]
    pub machine_class: MachineClass,
    #[serde(default)]
    pub quality_by_language: BTreeMap<String, Quality>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub platforms: Option<Platforms>,
    /// False for every non-commercial entry: the user must opt in.
    #[serde(default = "yes")]
    pub default_enabled: bool,
}

fn yes() -> bool {
    true
}

impl CatalogueEntry {
    /// Everything the user downloads for this entry: the files and any packages after them.
    pub fn total_download_bytes(&self) -> u64 {
        self.size_bytes + self.extra_download_bytes
    }

    /// Files to fetch, as single-file pieces. A single-file entry gives one piece.
    pub fn pieces(&self) -> Vec<CatalogueEntry> {
        if self.files.is_empty() {
            return vec![self.clone()];
        }
        self.files
            .iter()
            .map(|f| CatalogueEntry {
                id: self.id.clone(),
                name: self.name.clone(),
                size_bytes: f.size_bytes,
                url: f.url.clone(),
                sha256: f.sha256.clone(),
                file_name: Some(f.name.clone()),
                files: Vec::new(),
                ..self.clone()
            })
            .collect()
    }
}

/// The catalogue shipped with the app, from `config/model-catalogue.json`.
pub fn bundled() -> Vec<CatalogueEntry> {
    serde_json::from_str(include_str!("../../../config/model-catalogue.json"))
        .expect("bundled model-catalogue.json is valid")
}

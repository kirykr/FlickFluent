//! The dubbing data stored in `project.json` from format 2 on. See
//! `specs/002-core-dub-pipeline/data-model.md`. Times are milliseconds from the start of the source.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;

/// Source video facts. `fingerprint` detects a changed or moved file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub path: String,
    pub duration_ms: u64,
    pub has_audio: bool,
    pub has_video: bool,
    pub size_bytes: u64,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportKind {
    #[default]
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    /// Skips voice and music separation. Music cannot be kept apart from speech.
    Fast,
    #[default]
    Standard,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub mute_original: bool,
    pub export_kind: ExportKind,
    pub export_name: String,
    pub quality: Quality,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerGender {
    Male,
    Female,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
    pub id: String,
    pub gender: SpeakerGender,
    pub voice_id: String,
    /// 0 to 1. A low value is shown to the user as a hint.
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineFlag {
    Overlap,
    TooLong,
    TooShort,
    MixedLanguage,
    LowConfidence,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineStatus {
    #[default]
    Transcribed,
    Translated,
    Synthesized,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLine {
    pub id: String,
    pub section_index: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker_id: String,
    pub language: String,
    pub text: String,
    pub translated: Option<String>,
    /// Path of the synthesized clip, relative to the work folder.
    pub audio: Option<String>,
    pub audio_ms: Option<u64>,
    pub flags: BTreeSet<LineFlag>,
    pub status: LineStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Extract,
    Separate,
    Transcribe,
    Translate,
    Synthesize,
    Mix,
    Export,
}

impl Stage {
    pub const ALL: [Stage; 7] = [
        Stage::Extract,
        Stage::Separate,
        Stage::Transcribe,
        Stage::Translate,
        Stage::Synthesize,
        Stage::Mix,
        Stage::Export,
    ];

    /// Stages that run once per section.
    pub const PER_SECTION: [Stage; 4] = [
        Stage::Separate,
        Stage::Transcribe,
        Stage::Translate,
        Stage::Synthesize,
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageStatus {
    #[default]
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub index: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(default)]
    pub status: BTreeMap<Stage, StageStatus>,
    /// Resume skips a `Done` stage only if this hash matches the current input and settings.
    #[serde(default)]
    pub input_hash: BTreeMap<Stage, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageState {
    pub stage: Stage,
    pub status: StageStatus,
    pub progress: f32,
    pub eta_seconds: Option<u64>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

impl StageState {
    pub fn new(stage: Stage) -> Self {
        Self {
            stage,
            status: StageStatus::Pending,
            progress: 0.0,
            eta_seconds: None,
            started_at: None,
            ended_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    #[default]
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunError {
    pub code: String,
    pub stage: Option<Stage>,
    pub section_index: Option<u32>,
    pub message_key: String,
    pub next_step_key: String,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRun {
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub state: RunState,
    /// Languages, models and options. A change invalidates affected stages.
    pub settings_hash: String,
    pub sections: Vec<Section>,
    pub stages: Vec<StageState>,
    pub error: Option<RunError>,
}

/// Everything feature 002 adds to `project.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DubData {
    pub source: Option<Source>,
    /// ISO code or `auto`.
    pub source_language: String,
    pub detected_language: Option<String>,
    pub target_language: String,
    pub options: Options,
    pub speakers: Vec<Speaker>,
    pub lines: Vec<TranscriptLine>,
    pub run: Option<PipelineRun>,
}

impl DubData {
    /// Rules from data-model.md: `endMs > startMs`, lines inside their section.
    pub fn validate(&self) -> Result<(), AppError> {
        let sections: BTreeMap<u32, (u64, u64)> = self
            .run
            .iter()
            .flat_map(|r| r.sections.iter())
            .map(|s| (s.index, (s.start_ms, s.end_ms)))
            .collect();
        for line in &self.lines {
            if line.end_ms <= line.start_ms {
                return Err(invalid(format!("line {} ends before it starts", line.id)));
            }
            if let Some((start, end)) = sections.get(&line.section_index) {
                if line.start_ms < *start || line.end_ms > *end {
                    return Err(invalid(format!("line {} is outside its section", line.id)));
                }
            }
        }
        Ok(())
    }
}

fn invalid(detail: String) -> AppError {
    AppError::new(crate::error::INVALID_PROJECT, "error.invalidProject")
        .with_details(serde_json::json!({ "detail": detail }))
}

/// Read the dubbing fields out of `project.json`'s data. Missing or damaged fields are defaults.
pub fn read_from(data: &Value) -> DubData {
    serde_json::from_value(data.clone()).unwrap_or_default()
}

/// Write the dubbing fields into `project.json`'s data, keeping every other key (`notes`).
pub fn write_into(data: &mut Value, dub: &DubData) {
    if let (Value::Object(map), Ok(Value::Object(fields))) = (&mut *data, serde_json::to_value(dub))
    {
        for (k, v) in fields {
            map.insert(k, v);
        }
    } else if let Ok(v) = serde_json::to_value(dub) {
        *data = v;
    }
}

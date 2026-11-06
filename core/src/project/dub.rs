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
    /// The translated lines as an `.srt` file. The only export of a subtitles-only run.
    Subtitles,
}

/// What the user chose when a language pair is not fully supported (`languages_check_pair`
/// offers the choices). Kept in the project so a resume uses the same ones.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FallbackChoices {
    /// Transcribe and translate, but do not speak.
    pub subtitles_only: bool,
    /// Read the translation with a voice of another language.
    pub generic_voice: bool,
    /// Translate through English.
    pub pivot_via_english: bool,
    /// Non-commercial models the user turned on after reading their notice.
    pub opt_in: Vec<String>,
    /// The engine for each heavy step in this run. Copied from the settings when the run starts,
    /// so a resume uses the same ones. `local` means no data leaves the computer.
    pub engines: crate::cloud::choice::Engines,
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
    #[serde(default)]
    pub fallbacks: FallbackChoices,
    /// The user confirmed that source and target are the same language.
    #[serde(default)]
    pub confirm_same: bool,
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
    /// Where `gender` came from. A later run does not overwrite a value the user set.
    #[serde(default)]
    pub gender_source: GenderSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GenderSource {
    #[default]
    Detected,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineFlag {
    Overlap,
    TooLong,
    TooShort,
    MixedLanguage,
    LowConfidence,
    /// The editor emptied the text: the line is kept without speech (feature 004).
    EmptyText,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The clip as the engine returned it, relative to the work folder (feature 004).
    #[serde(default)]
    pub raw_audio: Option<String>,
    #[serde(default)]
    pub raw_ms: Option<u64>,
    /// Post-chain speed, volume and pitch applied to the raw clip to make `audio`.
    #[serde(default = "neutral_speed")]
    pub speed: f64,
    #[serde(default)]
    pub volume_db: f64,
    #[serde(default)]
    pub pitch_semitones: f64,
    /// Where the mix places the clip. Absent means `start_ms`.
    #[serde(default)]
    pub dub_start_ms: Option<u64>,
    /// Hash of what the current clip was made from. Empty on a project from before 004.
    #[serde(default)]
    pub speech_hash: String,
    /// The user touched this line, so a later run does not overwrite it.
    #[serde(default)]
    pub user_edited: bool,
}

fn neutral_speed() -> f64 {
    1.0
}

impl Default for TranscriptLine {
    fn default() -> Self {
        Self {
            id: String::new(),
            section_index: 0,
            start_ms: 0,
            end_ms: 0,
            speaker_id: String::new(),
            language: String::new(),
            text: String::new(),
            translated: None,
            audio: None,
            audio_ms: None,
            flags: BTreeSet::new(),
            status: LineStatus::default(),
            raw_audio: None,
            raw_ms: None,
            speed: 1.0,
            volume_db: 0.0,
            pitch_semitones: 0.0,
            dub_start_ms: None,
            speech_hash: String::new(),
            user_edited: false,
        }
    }
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
    /// The editor changed a line of this section. A resume never runs its stages again, and a
    /// fresh run asks before replacing it (feature 004, research R16).
    #[serde(default)]
    pub edited: bool,
    /// A line of this section changed in a way `dub-mix.wav` does not show yet. The editor
    /// rebuilds the section's piece of the mix and clears this (feature 004).
    #[serde(default)]
    pub mix_stale: bool,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// What each cloud step sent, if any step used a cloud service. Never holds a key or text.
    pub cloud_usage: Vec<CloudUsage>,
    /// Id from `config/caption-templates.json` (feature 004).
    pub caption_template: String,
}

impl Default for DubData {
    fn default() -> Self {
        Self {
            source: None,
            source_language: String::new(),
            detected_language: None,
            target_language: String::new(),
            options: Options::default(),
            speakers: Vec::new(),
            lines: Vec::new(),
            run: None,
            cloud_usage: Vec::new(),
            caption_template: "classic".into(),
        }
    }
}

/// What one cloud step used in a project. A resumed run adds to the same record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CloudUsage {
    pub step: String,
    pub service_id: String,
    pub service_name: String,
    /// Characters actually sent (summed over requests, not counting retries).
    pub chars: u64,
    pub requests: u64,
    /// Labels of the keys used, in the order they were first used.
    pub keys_used: Vec<String>,
    pub switches: u64,
    pub waited_seconds: u64,
    pub run_id: String,
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

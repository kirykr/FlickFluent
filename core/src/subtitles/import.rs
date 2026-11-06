//! Importing a subtitle file into the editor: a preview of how it compares with the lines, then
//! one of three choices (`contracts/editor-commands.md`). Nothing changes until the user picks.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::read::{SubtitleCue, SubtitleFile, SubtitleFormat};
use crate::editor::ops::{section_for, Touched};
use crate::error::{AppError, EDIT_OUT_OF_RANGE};
use crate::project::dub::{DubData, LineStatus, TranscriptLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportChoice {
    /// Same number of cues as lines: put each cue's text into the line at its place.
    ReplaceText,
    /// Make the lines again from the cues.
    ReplaceLines,
    /// Give each line the text of the cue that overlaps it most.
    FillByOverlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DifferenceKind {
    /// Cue and line start or end more than a millisecond apart.
    Time,
    /// A line with no cue at its place.
    MissingCue,
    /// A cue with no line at its place.
    ExtraCue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// Position in the sorted cues and in the lines, from 1.
    pub index: usize,
    pub kind: DifferenceKind,
    pub line_start_ms: Option<u64>,
    pub cue_start_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub preview_id: String,
    pub format: SubtitleFormat,
    pub encoding: String,
    pub cue_count: usize,
    pub line_count: usize,
    /// Same number of cues and lines, every time within 1 ms.
    pub all_match: bool,
    /// The first twenty differences.
    pub differences: Vec<Difference>,
    pub choices: Vec<ImportChoice>,
}

const LISTED: usize = 20;

fn close(a: u64, b: u64) -> bool {
    a.abs_diff(b) <= 1
}

pub fn preview(lines: &[TranscriptLine], file: &SubtitleFile, preview_id: String) -> ImportPreview {
    let mut differences = Vec::new();
    let mut all_match = lines.len() == file.cues.len();
    for i in 0..lines.len().max(file.cues.len()) {
        let diff = match (lines.get(i), file.cues.get(i)) {
            (Some(l), Some(c)) if close(l.start_ms, c.start_ms) && close(l.end_ms, c.end_ms) => {
                None
            }
            (Some(l), Some(c)) => Some((DifferenceKind::Time, Some(l.start_ms), Some(c.start_ms))),
            (Some(l), None) => Some((DifferenceKind::MissingCue, Some(l.start_ms), None)),
            (None, Some(c)) => Some((DifferenceKind::ExtraCue, None, Some(c.start_ms))),
            (None, None) => None,
        };
        if let Some((kind, line_start_ms, cue_start_ms)) = diff {
            all_match = false;
            if differences.len() < LISTED {
                differences.push(Difference {
                    index: i + 1,
                    kind,
                    line_start_ms,
                    cue_start_ms,
                });
            }
        }
    }
    let mut choices = Vec::new();
    if lines.len() == file.cues.len() {
        choices.push(ImportChoice::ReplaceText);
    }
    choices.push(ImportChoice::ReplaceLines);
    choices.push(ImportChoice::FillByOverlap);
    ImportPreview {
        preview_id,
        format: file.format,
        encoding: file.encoding.clone(),
        cue_count: file.cues.len(),
        line_count: lines.len(),
        all_match,
        differences,
        choices,
    }
}

fn overlap(a: (u64, u64), b: (u64, u64)) -> u64 {
    a.1.min(b.1).saturating_sub(a.0.max(b.0))
}

/// Apply a choice to a copy of the project data.
pub fn apply(
    dub: &mut DubData,
    cues: &[SubtitleCue],
    choice: ImportChoice,
) -> Result<Touched, AppError> {
    let mut touched = Touched::default();
    match choice {
        ImportChoice::ReplaceText => {
            if cues.len() != dub.lines.len() {
                return Err(AppError::new(EDIT_OUT_OF_RANGE, "editor.error.outOfRange")
                    .with_details(serde_json::json!({ "field": "cueCount" })));
            }
            for (l, c) in dub.lines.iter_mut().zip(cues) {
                if l.translated.as_deref() != Some(c.text.as_str()) {
                    l.translated = Some(c.text.clone());
                    l.user_edited = true;
                    touched.lines.insert(l.id.clone());
                }
            }
        }
        ImportChoice::FillByOverlap => {
            for l in &mut dub.lines {
                let span = (l.start_ms, l.end_ms);
                let start = cues.partition_point(|c| c.end_ms <= l.start_ms);
                let best = cues[start..]
                    .iter()
                    .take_while(|c| c.start_ms < l.end_ms)
                    .map(|c| (overlap(span, (c.start_ms, c.end_ms)), c))
                    .filter(|(o, _)| *o > 0)
                    .max_by_key(|(o, _)| *o);
                if let Some((_, c)) = best {
                    if l.translated.as_deref() != Some(c.text.as_str()) {
                        l.translated = Some(c.text.clone());
                        l.user_edited = true;
                        touched.lines.insert(l.id.clone());
                    }
                }
            }
        }
        ImportChoice::ReplaceLines => {
            let old = std::mem::take(&mut dub.lines);
            let first = dub
                .speakers
                .first()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "spk1".into());
            let mut taken = BTreeSet::new();
            for c in cues {
                let span = (c.start_ms, c.end_ms);
                let speaker = old
                    .iter()
                    .map(|l| (overlap(span, (l.start_ms, l.end_ms)), &l.speaker_id))
                    .filter(|(o, _)| *o > 0)
                    .max_by_key(|(o, _)| *o)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_else(|| first.clone());
                let mut id;
                loop {
                    id = format!("i{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
                    if taken.insert(id.clone()) {
                        break;
                    }
                }
                let neighbour = old
                    .iter()
                    .rev()
                    .find(|l| l.start_ms <= c.start_ms)
                    .map(|l| l.section_index);
                dub.lines.push(TranscriptLine {
                    id: id.clone(),
                    section_index: section_for(dub, c.start_ms, neighbour),
                    start_ms: c.start_ms,
                    end_ms: c.end_ms.max(c.start_ms + 1),
                    speaker_id: speaker,
                    language: dub.target_language.clone(),
                    translated: Some(c.text.clone()),
                    status: LineStatus::Translated,
                    user_edited: true,
                    ..Default::default()
                });
                touched.lines.insert(id);
            }
            for l in &old {
                touched.sections.insert(l.section_index);
            }
        }
    }
    Ok(touched)
}

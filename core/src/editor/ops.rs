//! One undoable step. An op changes a copy of the project data and says what it touched; the
//! session (`session.rs`) works out the inverse by comparing the copy with the original, saves,
//! and keeps the inverse. A refused op returns before the copy is used, so it changes nothing.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::limits;
use super::runtime::EditorRuntime;
use super::search::{replace_ranges, Match, MatchField};
use crate::error::{
    AppError, EDIT_CANNOT_MERGE, EDIT_END_BEFORE_START, EDIT_NOT_FOUND, EDIT_OUT_OF_RANGE,
};
use crate::project::dub::{DubData, GenderSource, LineStatus, SpeakerGender, TranscriptLine};
use crate::subtitles::import::ImportChoice;
use crate::subtitles::read::SubtitleCue;

/// What an op touched beyond the lines the session finds by comparing.
#[derive(Debug, Default)]
pub struct Touched {
    /// Lines whose flags must be recomputed.
    pub lines: BTreeSet<String>,
    /// Sections to mark `edited` even when no line differs (a speaker's gender changed).
    pub sections: BTreeSet<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LinePatch {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub translated: Option<String>,
    pub speaker_id: Option<String>,
    pub speed: Option<f64>,
    pub volume_db: Option<f64>,
    pub pitch_semitones: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    UpdateLine {
        id: String,
        patch: LinePatch,
    },
    InsertLine {
        after_id: Option<String>,
        start_ms: u64,
        end_ms: u64,
        speaker_id: String,
        translated: String,
    },
    DeleteLines {
        ids: Vec<String>,
    },
    /// Replace exactly these matches of the translated text. Matches in the original text are
    /// ignored: the original is not editable.
    ReplaceMatches {
        matches: Vec<Match>,
        replacement: String,
    },
    /// The result of a fit: new speeds and new start times, as one step.
    ApplyFit {
        speeds: Vec<(String, f64)>,
        starts: Vec<(String, Option<u64>)>,
    },
    /// Join neighbouring lines of one speaker into the first of them.
    MergeLines {
        ids: Vec<String>,
    },
    /// Cut a line's translation at a UTF-16 offset; the time is divided in proportion.
    SplitLine {
        id: String,
        text_offset: usize,
    },
    DuplicateLine {
        id: String,
    },
    /// New translated text for many lines; only lines whose text differs change.
    BulkText {
        texts: Vec<(String, String)>,
    },
    /// The user corrects a speaker's gender (kept by a later run).
    SetSpeakerGender {
        speaker_id: String,
        gender: SpeakerGender,
    },
    /// An imported subtitle file, applied by one of the three choices.
    ImportSubtitles {
        choice: ImportChoice,
        cues: Vec<SubtitleCue>,
    },
}

fn not_found(what: &str, id: &str) -> AppError {
    AppError::new(EDIT_NOT_FOUND, "editor.error.notFound")
        .with_details(serde_json::json!({ "kind": what, "id": id }))
}

fn out_of_range(field: &str, min: f64, max: f64) -> AppError {
    AppError::new(EDIT_OUT_OF_RANGE, "editor.error.outOfRange")
        .with_details(serde_json::json!({ "field": field, "min": min, "max": max }))
}

fn end_before_start() -> AppError {
    AppError::new(EDIT_END_BEFORE_START, "editor.error.endBeforeStart")
}

pub fn find(dub: &DubData, id: &str) -> Result<usize, AppError> {
    dub.lines
        .iter()
        .position(|l| l.id == id)
        .ok_or_else(|| not_found("line", id))
}

fn check_speaker(dub: &DubData, id: &str) -> Result<(), AppError> {
    if dub.speakers.iter().any(|s| s.id == id) {
        Ok(())
    } else {
        Err(not_found("speaker", id))
    }
}

/// The section a line starting at `start_ms` belongs to: the one that holds its start.
pub(crate) fn section_for(dub: &DubData, start_ms: u64, neighbour: Option<u32>) -> u32 {
    dub.run
        .iter()
        .flat_map(|r| r.sections.iter())
        .find(|s| start_ms >= s.start_ms && start_ms < s.end_ms)
        .map(|s| s.index)
        .or(neighbour)
        .unwrap_or(0)
}

fn new_id() -> String {
    format!("e{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

impl EditOp {
    pub fn apply(&self, dub: &mut DubData, _rt: &EditorRuntime) -> Result<Touched, AppError> {
        match self {
            EditOp::UpdateLine { id, patch } => update_line(dub, id, patch),
            EditOp::InsertLine {
                after_id,
                start_ms,
                end_ms,
                speaker_id,
                translated,
            } => insert_line(
                dub,
                after_id.as_deref(),
                *start_ms,
                *end_ms,
                speaker_id,
                translated,
            ),
            EditOp::DeleteLines { ids } => delete_lines(dub, ids),
            EditOp::ReplaceMatches {
                matches,
                replacement,
            } => replace_matches(dub, matches, replacement),
            EditOp::ApplyFit { speeds, starts } => apply_fit(dub, speeds, starts),
            EditOp::MergeLines { ids } => merge_lines(dub, ids),
            EditOp::SplitLine { id, text_offset } => split_line(dub, id, *text_offset),
            EditOp::DuplicateLine { id } => duplicate_line(dub, id),
            EditOp::BulkText { texts } => bulk_text(dub, texts),
            EditOp::SetSpeakerGender { speaker_id, gender } => set_gender(dub, speaker_id, *gender),
            EditOp::ImportSubtitles { choice, cues } => {
                crate::subtitles::import::apply(dub, cues, *choice)
            }
        }
    }
}

fn update_line(dub: &mut DubData, id: &str, p: &LinePatch) -> Result<Touched, AppError> {
    let i = find(dub, id)?;
    let (start, end) = (
        p.start_ms.unwrap_or(dub.lines[i].start_ms),
        p.end_ms.unwrap_or(dub.lines[i].end_ms),
    );
    if end <= start {
        return Err(end_before_start());
    }
    if let Some(v) = p.speed {
        if !limits::speed_ok(v) {
            return Err(out_of_range("speed", limits::SPEED_MIN, limits::SPEED_MAX));
        }
    }
    if let Some(v) = p.volume_db {
        if !limits::volume_ok(v) {
            return Err(out_of_range(
                "volumeDb",
                limits::VOLUME_MIN_DB,
                limits::VOLUME_MAX_DB,
            ));
        }
    }
    if let Some(v) = p.pitch_semitones {
        if !limits::pitch_ok(v) {
            return Err(out_of_range(
                "pitchSemitones",
                limits::PITCH_MIN,
                limits::PITCH_MAX,
            ));
        }
    }
    if let Some(s) = &p.speaker_id {
        check_speaker(dub, s)?;
    }

    let l = &mut dub.lines[i];
    l.start_ms = start;
    l.end_ms = end;
    if let Some(t) = &p.translated {
        l.translated = Some(t.clone());
    }
    if let Some(s) = &p.speaker_id {
        l.speaker_id = s.clone();
    }
    if let Some(v) = p.speed {
        l.speed = v;
    }
    if let Some(v) = p.volume_db {
        l.volume_db = v;
    }
    if let Some(v) = p.pitch_semitones {
        l.pitch_semitones = v;
    }
    l.user_edited = true;
    let moved = p.start_ms.is_some();
    if moved {
        let neighbour = Some(dub.lines[i].section_index);
        let section = section_for(dub, start, neighbour);
        dub.lines[i].section_index = section;
        // Keep the table in time order. The sort is stable, so equal starts keep their order.
        dub.lines.sort_by_key(|l| l.start_ms);
    }
    Ok(Touched {
        lines: BTreeSet::from([id.to_string()]),
        sections: BTreeSet::new(),
    })
}

fn insert_line(
    dub: &mut DubData,
    after_id: Option<&str>,
    start_ms: u64,
    end_ms: u64,
    speaker_id: &str,
    translated: &str,
) -> Result<Touched, AppError> {
    if end_ms <= start_ms {
        return Err(end_before_start());
    }
    check_speaker(dub, speaker_id)?;
    let after = after_id.map(|a| find(dub, a)).transpose()?;
    let at = match after {
        Some(i) => i + 1,
        None => dub
            .lines
            .iter()
            .position(|l| l.start_ms > start_ms)
            .unwrap_or(dub.lines.len()),
    };
    let neighbour = at
        .checked_sub(1)
        .and_then(|p| dub.lines.get(p))
        .map(|l| l.section_index);
    let id = new_id();
    let line = TranscriptLine {
        id: id.clone(),
        section_index: section_for(dub, start_ms, neighbour),
        start_ms,
        end_ms,
        speaker_id: speaker_id.to_string(),
        language: dub.target_language.clone(),
        text: String::new(),
        translated: Some(translated.to_string()),
        status: LineStatus::Translated,
        user_edited: true,
        ..Default::default()
    };
    dub.lines.insert(at, line);
    Ok(Touched {
        lines: BTreeSet::from([id]),
        sections: BTreeSet::new(),
    })
}

fn delete_lines(dub: &mut DubData, ids: &[String]) -> Result<Touched, AppError> {
    for id in ids {
        find(dub, id)?;
    }
    let mut touched = Touched::default();
    // The lines next to a removed one may stop overlapping, so they are rechecked.
    for (i, l) in dub.lines.iter().enumerate() {
        if ids.contains(&l.id) {
            touched.sections.insert(l.section_index);
            if let Some(next) = dub.lines.get(i + 1) {
                touched.lines.insert(next.id.clone());
            }
        }
    }
    dub.lines.retain(|l| !ids.contains(&l.id));
    touched.lines.retain(|id| !ids.contains(id));
    Ok(touched)
}

fn replace_matches(
    dub: &mut DubData,
    matches: &[Match],
    replacement: &str,
) -> Result<Touched, AppError> {
    let mut touched = Touched::default();
    for line in &mut dub.lines {
        let ranges: Vec<(usize, usize)> = matches
            .iter()
            .filter(|m| m.line_id == line.id && m.field == MatchField::Translated)
            .map(|m| (m.start, m.end))
            .collect();
        let Some(text) = line.translated.as_deref() else {
            continue;
        };
        if ranges.is_empty() {
            continue;
        }
        let new = replace_ranges(text, &ranges, replacement);
        if new != text {
            line.translated = Some(new);
            line.user_edited = true;
            touched.lines.insert(line.id.clone());
        }
    }
    Ok(touched)
}

fn apply_fit(
    dub: &mut DubData,
    speeds: &[(String, f64)],
    starts: &[(String, Option<u64>)],
) -> Result<Touched, AppError> {
    for (_, v) in speeds {
        if !limits::speed_ok(*v) {
            return Err(out_of_range("speed", limits::SPEED_MIN, limits::SPEED_MAX));
        }
    }
    let mut touched = Touched::default();
    for (id, v) in speeds {
        let i = find(dub, id)?;
        dub.lines[i].speed = *v;
        dub.lines[i].user_edited = true;
        touched.lines.insert(id.clone());
    }
    for (id, v) in starts {
        let i = find(dub, id)?;
        dub.lines[i].dub_start_ms = *v;
        dub.lines[i].user_edited = true;
        touched.lines.insert(id.clone());
    }
    Ok(touched)
}

fn cannot_merge() -> AppError {
    AppError::new(EDIT_CANNOT_MERGE, "editor.error.cannotMerge")
}

fn merge_lines(dub: &mut DubData, ids: &[String]) -> Result<Touched, AppError> {
    if ids.len() < 2 {
        return Err(cannot_merge());
    }
    let mut at: Vec<usize> = ids
        .iter()
        .map(|id| find(dub, id))
        .collect::<Result<_, _>>()?;
    at.sort_unstable();
    at.dedup();
    // Neighbours: one run of consecutive lines, all by the same speaker.
    let consecutive = at.windows(2).all(|w| w[1] == w[0] + 1);
    let speaker = &dub.lines[at[0]].speaker_id;
    if at.len() < 2 || !consecutive || at.iter().any(|i| &dub.lines[*i].speaker_id != speaker) {
        return Err(cannot_merge());
    }
    let join = |f: &dyn Fn(&TranscriptLine) -> String| {
        at.iter()
            .map(|i| f(&dub.lines[*i]))
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let translated = join(&|l| l.translated.clone().unwrap_or_default());
    let original = join(&|l| l.text.clone());
    let end = at.iter().map(|i| dub.lines[*i].end_ms).max().unwrap_or(0);
    let start = at.iter().map(|i| dub.lines[*i].start_ms).min().unwrap_or(0);
    let mut touched = Touched::default();
    let keep = dub.lines[at[0]].id.clone();
    for i in &at {
        touched.sections.insert(dub.lines[*i].section_index);
    }
    let removed: Vec<String> = at[1..].iter().map(|i| dub.lines[*i].id.clone()).collect();
    {
        let l = &mut dub.lines[at[0]];
        l.translated = Some(translated);
        l.text = original;
        l.start_ms = start;
        l.end_ms = end;
        l.dub_start_ms = None;
        l.user_edited = true;
    }
    dub.lines.retain(|l| !removed.contains(&l.id));
    touched.lines.insert(keep);
    Ok(touched)
}

fn split_line(dub: &mut DubData, id: &str, offset: usize) -> Result<Touched, AppError> {
    let i = find(dub, id)?;
    let line = dub.lines[i].clone();
    let text = line.translated.clone().unwrap_or_default();
    let units: Vec<u16> = text.encode_utf16().collect();
    if offset == 0 || offset >= units.len() {
        return Err(out_of_range(
            "textOffset",
            1.0,
            units.len().saturating_sub(1) as f64,
        ));
    }
    let first = String::from_utf16_lossy(&units[..offset])
        .trim()
        .to_string();
    let second = String::from_utf16_lossy(&units[offset..])
        .trim()
        .to_string();
    let span = line.end_ms - line.start_ms;
    let cut = line.start_ms + span * offset as u64 / units.len() as u64;
    if cut <= line.start_ms || cut >= line.end_ms {
        return Err(out_of_range("textOffset", 1.0, units.len() as f64));
    }
    let new_id = new_id();
    {
        let l = &mut dub.lines[i];
        l.translated = Some(first);
        l.end_ms = cut;
        l.dub_start_ms = None;
        l.user_edited = true;
    }
    let tail = TranscriptLine {
        id: new_id.clone(),
        start_ms: cut,
        end_ms: line.end_ms,
        translated: Some(second),
        text: String::new(),
        audio: None,
        audio_ms: None,
        raw_audio: None,
        raw_ms: None,
        speech_hash: String::new(),
        dub_start_ms: None,
        flags: Default::default(),
        status: LineStatus::Translated,
        user_edited: true,
        ..line
    };
    dub.lines.insert(i + 1, tail);
    Ok(Touched {
        lines: BTreeSet::from([id.to_string(), new_id]),
        sections: BTreeSet::new(),
    })
}

fn duplicate_line(dub: &mut DubData, id: &str) -> Result<Touched, AppError> {
    let i = find(dub, id)?;
    let line = dub.lines[i].clone();
    let span = line.end_ms - line.start_ms;
    let new_id = new_id();
    let copy = TranscriptLine {
        id: new_id.clone(),
        start_ms: line.end_ms,
        end_ms: line.end_ms + span,
        audio: None,
        audio_ms: None,
        raw_audio: None,
        raw_ms: None,
        speech_hash: String::new(),
        dub_start_ms: None,
        flags: Default::default(),
        status: LineStatus::Translated,
        user_edited: true,
        section_index: section_for(dub, line.end_ms, Some(line.section_index)),
        ..line
    };
    dub.lines.insert(i + 1, copy);
    // The copy may now start over the next line; the flag says so and nothing is moved.
    Ok(Touched {
        lines: BTreeSet::from([new_id]),
        sections: BTreeSet::new(),
    })
}

fn bulk_text(dub: &mut DubData, texts: &[(String, String)]) -> Result<Touched, AppError> {
    // Every id must exist before anything changes.
    for (id, _) in texts {
        find(dub, id)?;
    }
    let mut touched = Touched::default();
    for (id, text) in texts {
        let i = find(dub, id)?;
        if dub.lines[i].translated.as_deref() != Some(text.as_str()) {
            dub.lines[i].translated = Some(text.clone());
            dub.lines[i].user_edited = true;
            touched.lines.insert(id.clone());
        }
    }
    Ok(touched)
}

fn set_gender(
    dub: &mut DubData,
    speaker_id: &str,
    gender: SpeakerGender,
) -> Result<Touched, AppError> {
    let s = dub
        .speakers
        .iter_mut()
        .find(|s| s.id == speaker_id)
        .ok_or_else(|| not_found("speaker", speaker_id))?;
    s.gender = gender;
    s.gender_source = GenderSource::User;
    let mut touched = Touched::default();
    for l in dub.lines.iter_mut().filter(|l| l.speaker_id == speaker_id) {
        // The voice follows the gender, so these lines need speech again.
        l.user_edited = true;
        touched.sections.insert(l.section_index);
    }
    Ok(touched)
}

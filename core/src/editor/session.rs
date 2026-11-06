//! The editing session: the working project data, the undo and redo stacks, and the save after
//! every accepted edit (research R9). Stacks live in memory and end with the session.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use super::flags;
use super::hash;
use super::ops::{EditOp, Touched};
use super::runtime::EditorRuntime;
use crate::cloud::guard::merge_usage;
use crate::error::{AppError, EDIT_NOTHING_TO_UNDO};
use crate::pipeline::mix::{MixLine, MixSection};
use crate::pipeline::plan::VoicePick;
use crate::pipeline::run::window_ms;
use crate::project::dub::{CloudUsage, DubData, LineStatus, Quality, Speaker, TranscriptLine};
use crate::subtitles::import::{self, ImportChoice, ImportPreview};
use crate::subtitles::read::SubtitleFile;

/// Writes the project data to its file. The shell supplies it, so the core does not own the
/// project lock.
pub type Saver = Box<dyn FnMut(&DubData) -> Result<(), AppError> + Send>;

const UNDO_DEPTH: usize = 200;
/// How long an import preview can be applied after it was made.
const PREVIEW_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Enough to turn the data after an edit back into the data before it.
#[derive(Debug, Clone, Default)]
struct Delta {
    /// Line ids in their order before.
    order: Vec<String>,
    /// Lines that existed before and after but differ: their old form.
    changed: Vec<TranscriptLine>,
    /// Lines that existed before and not after.
    removed: Vec<TranscriptLine>,
    /// Ids that exist after and not before.
    added: Vec<String>,
    speakers: Option<Vec<Speaker>>,
    /// `(section index, edited, mix_stale)` before, for sections that differ.
    sections: Vec<(u32, bool, bool)>,
}

impl Delta {
    fn is_empty(&self) -> bool {
        self.changed.is_empty()
            && self.removed.is_empty()
            && self.added.is_empty()
            && self.speakers.is_none()
            && self.sections.is_empty()
            && self.order.is_empty()
    }
}

/// What an edit did, for the UI: only the lines that changed or appeared.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub lines: Vec<TranscriptLine>,
    pub removed_ids: Vec<String>,
    pub changed_ids: Vec<String>,
    pub needs_post_ids: Vec<String>,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    pub lines: Vec<TranscriptLine>,
    pub speakers: Vec<Speaker>,
    pub changed_ids: Vec<String>,
    pub needs_post_ids: Vec<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub caption_template: String,
}

pub struct EditSession {
    dub: DubData,
    rt: EditorRuntime,
    undo: Vec<Delta>,
    redo: Vec<Delta>,
    saver: Saver,
    /// Counts changes to what the mix holds, so a rebuild knows if it is still current.
    epoch: u64,
    previews: HashMap<String, (std::time::Instant, SubtitleFile)>,
    preview_ttl: std::time::Duration,
}

/// Compare `before` with `after`. Returns the delta that turns `after` back into `before`, and
/// the lines of `after` that differ (new or changed) with the ids that went away.
fn diff(before: &DubData, after: &DubData) -> (Delta, Vec<TranscriptLine>, Vec<String>) {
    let old: HashMap<&str, &TranscriptLine> =
        before.lines.iter().map(|l| (l.id.as_str(), l)).collect();
    let new: HashMap<&str, &TranscriptLine> =
        after.lines.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut delta = Delta::default();
    let mut shown = Vec::new();
    for l in &after.lines {
        match old.get(l.id.as_str()) {
            None => {
                delta.added.push(l.id.clone());
                shown.push(l.clone());
            }
            Some(o) if *o != l => {
                delta.changed.push((*o).clone());
                shown.push(l.clone());
            }
            _ => {}
        }
    }
    let mut removed_ids = Vec::new();
    for l in &before.lines {
        if !new.contains_key(l.id.as_str()) {
            delta.removed.push(l.clone());
            removed_ids.push(l.id.clone());
        }
    }
    if before.speakers != after.speakers {
        delta.speakers = Some(before.speakers.clone());
    }
    let sections_of = |d: &DubData| -> Vec<(u32, bool, bool)> {
        d.run
            .iter()
            .flat_map(|r| r.sections.iter())
            .map(|s| (s.index, s.edited, s.mix_stale))
            .collect()
    };
    let (sb, sa) = (sections_of(before), sections_of(after));
    delta.sections = sb
        .iter()
        .zip(sa.iter())
        .filter(|(b, a)| b != a)
        .map(|(b, _)| *b)
        .collect();
    let order_before: Vec<String> = before.lines.iter().map(|l| l.id.clone()).collect();
    let order_after: Vec<String> = after.lines.iter().map(|l| l.id.clone()).collect();
    let touched = !(delta.changed.is_empty() && delta.removed.is_empty() && delta.added.is_empty());
    if touched || order_before != order_after {
        delta.order = order_before;
    }
    (delta, shown, removed_ids)
}

fn apply_delta(dub: &mut DubData, d: &Delta) {
    let added: BTreeSet<&str> = d.added.iter().map(String::as_str).collect();
    dub.lines.retain(|l| !added.contains(l.id.as_str()));
    let mut index: HashMap<String, usize> = dub
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id.clone(), i))
        .collect();
    for old in &d.changed {
        if let Some(i) = index.get(&old.id) {
            // What the user changed comes back; the clip on disk is whatever was made last, so
            // the line keeps it and shows as changed if it no longer fits (research R9).
            let mut restored = old.clone();
            let current = &dub.lines[*i];
            restored.audio = current.audio.clone();
            restored.audio_ms = current.audio_ms;
            restored.raw_audio = current.raw_audio.clone();
            restored.raw_ms = current.raw_ms;
            restored.speech_hash = current.speech_hash.clone();
            dub.lines[*i] = restored;
        }
    }
    for old in &d.removed {
        index.insert(old.id.clone(), dub.lines.len());
        dub.lines.push(old.clone());
    }
    if !d.order.is_empty() {
        let position: HashMap<&str, usize> = d
            .order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        dub.lines
            .sort_by_key(|l| position.get(l.id.as_str()).copied().unwrap_or(usize::MAX));
    }
    if let Some(sp) = &d.speakers {
        dub.speakers = sp.clone();
    }
    if let Some(run) = dub.run.as_mut() {
        for (index, edited, stale) in &d.sections {
            if let Some(s) = run.sections.iter_mut().find(|s| s.index == *index) {
                s.edited = *edited;
                s.mix_stale = *stale;
            }
        }
    }
}

/// Does the line differ in a way the mix shows?
fn mix_relevant(a: &TranscriptLine, b: &TranscriptLine) -> bool {
    let has_clip = a.audio.is_some() || b.audio.is_some();
    let differs = (
        a.start_ms,
        a.dub_start_ms,
        &a.audio,
        a.audio_ms,
        a.section_index,
    ) != (
        b.start_ms,
        b.dub_start_ms,
        &b.audio,
        b.audio_ms,
        b.section_index,
    );
    has_clip && differs
}

/// Mark the sections an edit touched: `edited` always, `mix_stale` when the mix would differ.
fn mark_sections(before: &DubData, after: &mut DubData, extra: &BTreeSet<u32>) {
    let old: HashMap<&str, &TranscriptLine> =
        before.lines.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut edited: BTreeSet<u32> = extra.clone();
    let mut stale: BTreeSet<u32> = BTreeSet::new();
    for l in &after.lines {
        match old.get(l.id.as_str()) {
            None => {
                edited.insert(l.section_index);
                if l.audio.is_some() {
                    stale.insert(l.section_index);
                }
            }
            Some(o) if *o != l => {
                edited.insert(l.section_index);
                edited.insert(o.section_index);
                if mix_relevant(o, l) {
                    stale.insert(l.section_index);
                    stale.insert(o.section_index);
                }
            }
            _ => {}
        }
    }
    let kept: BTreeSet<&str> = after.lines.iter().map(|l| l.id.as_str()).collect();
    for o in &before.lines {
        if !kept.contains(o.id.as_str()) {
            edited.insert(o.section_index);
            if o.audio.is_some() {
                stale.insert(o.section_index);
            }
        }
    }
    if let Some(run) = after.run.as_mut() {
        for s in &mut run.sections {
            if edited.contains(&s.index) {
                s.edited = true;
            }
            if stale.contains(&s.index) {
                s.mix_stale = true;
            }
        }
    }
}

impl EditSession {
    /// Open a session over the project data. Lines with a clip and no hash (a project from
    /// before 004) get the hash of their current inputs, and the project is saved if that
    /// filled any.
    pub fn open(rt: EditorRuntime, mut dub: DubData, mut saver: Saver) -> Result<Self, AppError> {
        let speakers = dub.speakers.clone();
        if hash::fill_missing(&rt, &speakers, &mut dub.lines) > 0 {
            saver(&dub)?;
        }
        Ok(Self {
            dub,
            rt,
            undo: Vec::new(),
            redo: Vec::new(),
            saver,
            epoch: 0,
            previews: HashMap::new(),
            preview_ttl: PREVIEW_TTL,
        })
    }

    pub fn dub(&self) -> &DubData {
        &self.dub
    }

    pub fn runtime(&self) -> &EditorRuntime {
        &self.rt
    }

    pub fn changed_ids(&self) -> Vec<String> {
        hash::changed_ids(&self.rt, &self.dub.speakers, &self.dub.lines)
    }

    pub fn needs_post_ids(&self) -> Vec<String> {
        self.dub
            .lines
            .iter()
            .filter(|l| hash::needs_post(&self.rt, &self.dub.speakers, l))
            .map(|l| l.id.clone())
            .collect()
    }

    pub fn state(&self) -> EditorState {
        EditorState {
            lines: self.dub.lines.clone(),
            speakers: self.dub.speakers.clone(),
            changed_ids: self.changed_ids(),
            needs_post_ids: self.needs_post_ids(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            caption_template: self.dub.caption_template.clone(),
        }
    }

    fn result(&self, lines: Vec<TranscriptLine>, removed_ids: Vec<String>) -> EditResult {
        EditResult {
            lines,
            removed_ids,
            changed_ids: self.changed_ids(),
            needs_post_ids: self.needs_post_ids(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    /// Run an op. A refused op, or a failed save, leaves the session as it was.
    pub fn apply(&mut self, op: EditOp) -> Result<EditResult, AppError> {
        let mut next = self.dub.clone();
        let touched = op.apply(&mut next, &self.rt)?;
        self.commit(next, touched)
    }

    /// Take `next` as the new data: recompute flags, mark sections, save, push the inverse.
    pub fn commit(&mut self, mut next: DubData, touched: Touched) -> Result<EditResult, AppError> {
        flags::refresh(&mut next.lines, &touched.lines);
        mark_sections(&self.dub, &mut next, &touched.sections);
        let (delta, shown, removed) = diff(&self.dub, &next);
        if delta.is_empty() {
            return Ok(self.result(Vec::new(), Vec::new()));
        }
        (self.saver)(&next)?;
        self.dub = next;
        self.epoch += 1;
        self.undo.push(delta);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(self.result(shown, removed))
    }

    pub fn undo(&mut self) -> Result<EditResult, AppError> {
        let delta = self
            .undo
            .pop()
            .ok_or_else(|| AppError::new(EDIT_NOTHING_TO_UNDO, "editor.error.nothingToUndo"))?;
        match self.step(&delta) {
            Ok((redo, shown, removed)) => {
                self.redo.push(redo);
                Ok(self.result(shown, removed))
            }
            Err(e) => {
                self.undo.push(delta);
                Err(e)
            }
        }
    }

    pub fn redo(&mut self) -> Result<EditResult, AppError> {
        let delta = self
            .redo
            .pop()
            .ok_or_else(|| AppError::new(EDIT_NOTHING_TO_UNDO, "editor.error.nothingToUndo"))?;
        match self.step(&delta) {
            Ok((undo, shown, removed)) => {
                self.undo.push(undo);
                Ok(self.result(shown, removed))
            }
            Err(e) => {
                self.redo.push(delta);
                Err(e)
            }
        }
    }

    /// Apply a delta and save. Returns the delta that undoes this step.
    fn step(
        &mut self,
        delta: &Delta,
    ) -> Result<(Delta, Vec<TranscriptLine>, Vec<String>), AppError> {
        let mut next = self.dub.clone();
        apply_delta(&mut next, delta);
        let touched: BTreeSet<String> = delta
            .changed
            .iter()
            .map(|l| l.id.clone())
            .chain(delta.removed.iter().map(|l| l.id.clone()))
            .collect();
        flags::refresh(&mut next.lines, &touched);
        let (inverse, shown, removed) = diff(&self.dub, &next);
        (self.saver)(&next)?;
        self.dub = next;
        self.epoch += 1;
        Ok((inverse, shown, removed))
    }
}

// --- making speech again -------------------------------------------------------------------

/// One line to make again. `post_only` means the raw clip is current and only the ffmpeg chain
/// has to run.
#[derive(Debug, Clone)]
pub struct SynthJob {
    pub id: String,
    pub text: String,
    pub speaker_id: String,
    pub section_index: u32,
    /// The hash of the line's inputs when the job was made. A clip is swapped in only if the
    /// line still has these inputs.
    pub expected_hash: String,
    /// Where the hash's raw part names the new raw clip file.
    pub raw_name: String,
    pub speed: f64,
    pub volume_db: f64,
    pub pitch_semitones: f64,
    /// The slot the clip should fit.
    pub target_ms: u64,
    pub post_only: bool,
    /// The current raw clip, relative to the work folder, for a post-only job.
    pub raw_audio: Option<String>,
    pub voice: Option<VoicePick>,
    pub gender_male: bool,
}

/// A clip that was made and is complete on disk. Paths are relative to the work folder.
#[derive(Debug, Clone)]
pub struct MadeClip {
    pub audio: String,
    pub audio_ms: u64,
    pub raw_audio: String,
    pub raw_ms: u64,
}

/// What the mix rebuild needs, taken at one moment.
pub struct MixPlan {
    pub sections: Vec<MixSection>,
    /// Indexes into `sections` whose piece of the mix is out of date.
    pub stale: Vec<usize>,
    pub epoch: u64,
    pub mute_original: bool,
}

impl EditSession {
    /// The lines to make again. `ids` absent means every changed line and every line that only
    /// needs the post chain. A line that is current is skipped.
    pub fn plan_synth(&self, ids: Option<&[String]>) -> Vec<SynthJob> {
        let mut jobs = Vec::new();
        for (i, l) in self.dub.lines.iter().enumerate() {
            if ids.is_some_and(|ids| !ids.contains(&l.id)) {
                continue;
            }
            let changed = hash::is_changed(&self.rt, &self.dub.speakers, l);
            let post = !changed && hash::needs_post(&self.rt, &self.dub.speakers, l);
            if !(changed || post) {
                continue;
            }
            let expected = hash::speech_hash(&self.rt, &self.dub.speakers, l);
            let male = self
                .dub
                .speakers
                .iter()
                .find(|s| s.id == l.speaker_id)
                .is_some_and(|s| s.gender == crate::project::dub::SpeakerGender::Male);
            jobs.push(SynthJob {
                id: l.id.clone(),
                text: hash::spoken_text(l).to_string(),
                speaker_id: l.speaker_id.clone(),
                section_index: l.section_index,
                raw_name: hash::hash8(expected.split('.').next().unwrap_or("")),
                expected_hash: expected,
                speed: l.speed,
                volume_db: l.volume_db,
                pitch_semitones: l.pitch_semitones,
                target_ms: window_ms(&self.dub.lines, i),
                post_only: post,
                raw_audio: l.raw_audio.clone().or_else(|| l.audio.clone()),
                voice: self.rt.voice_in(&self.dub.speakers, &l.speaker_id),
                gender_male: male,
            });
        }
        jobs
    }

    /// Put a made clip into its line, if the line still has the inputs the job was made from.
    /// Returns the files the line no longer uses (relative paths), or `None` when the line is
    /// gone or was edited meanwhile; the line then stays changed. Not an undo step: the clip is
    /// derived data.
    pub fn swap_clip(
        &mut self,
        job: &SynthJob,
        clip: MadeClip,
    ) -> Result<Option<(TranscriptLine, Vec<String>)>, AppError> {
        let Some(i) = self.dub.lines.iter().position(|l| l.id == job.id) else {
            return Ok(None);
        };
        let now = hash::speech_hash(&self.rt, &self.dub.speakers, &self.dub.lines[i]);
        if now != job.expected_hash {
            return Ok(None);
        }
        let mut next = self.dub.clone();
        let old = next.lines[i].clone();
        {
            let l = &mut next.lines[i];
            l.audio = Some(clip.audio.clone());
            l.audio_ms = Some(clip.audio_ms);
            l.raw_audio = Some(clip.raw_audio.clone());
            l.raw_ms = Some(clip.raw_ms);
            l.speech_hash = job.expected_hash.clone();
            l.status = LineStatus::Synthesized;
        }
        flags::refresh_fit(&mut next.lines, i);
        let section = next.lines[i].section_index;
        if let Some(run) = next.run.as_mut() {
            for s in &mut run.sections {
                if s.index == section {
                    s.edited = true;
                    s.mix_stale = true;
                }
            }
        }
        (self.saver)(&next)?;
        self.dub = next;
        self.epoch += 1;
        let mut unused = Vec::new();
        for path in [old.audio, old.raw_audio].into_iter().flatten() {
            if path != clip.audio && path != clip.raw_audio && !unused.contains(&path) {
                unused.push(path);
            }
        }
        Ok(Some((self.dub.lines[i].clone(), unused)))
    }

    /// Add what a cloud engine sent to the project's usage record.
    pub fn record_cloud_usage(
        &mut self,
        add: Vec<CloudUsage>,
        run_id: &str,
    ) -> Result<(), AppError> {
        if add.is_empty() {
            return Ok(());
        }
        let mut next = self.dub.clone();
        next.cloud_usage = merge_usage(std::mem::take(&mut next.cloud_usage), add, run_id);
        (self.saver)(&next)?;
        self.dub = next;
        Ok(())
    }

    /// The sections as the mix wants them now, and which of them are out of date.
    pub fn mix_plan(&self) -> Option<MixPlan> {
        let work = self.rt.work_folder()?;
        let run = self.dub.run.as_ref()?;
        let fast = self.dub.options.quality == Quality::Fast;
        let mut sections = Vec::new();
        let mut stale = Vec::new();
        for (n, sec) in run.sections.iter().enumerate() {
            let music = work
                .join("sections")
                .join(format!("{:04}", sec.index))
                .join("sec-music.wav");
            sections.push(MixSection {
                start_ms: sec.start_ms,
                end_ms: sec.end_ms,
                music: (!fast && music.exists()).then_some(music),
                lines: self
                    .dub
                    .lines
                    .iter()
                    .filter(|l| l.section_index == sec.index)
                    .filter_map(|l| {
                        l.audio.as_ref().map(|a| MixLine {
                            start_ms: l.dub_start_ms.unwrap_or(l.start_ms),
                            clip: work.join(a),
                        })
                    })
                    .collect(),
            });
            if sec.mix_stale {
                stale.push(n);
            }
        }
        Some(MixPlan {
            sections,
            stale,
            epoch: self.epoch,
            mute_original: self.dub.options.mute_original,
        })
    }

    /// The mix now shows these sections. They stay marked if anything changed since the plan.
    pub fn mix_rebuilt(&mut self, rebuilt: &[usize], epoch: u64) -> Result<(), AppError> {
        if epoch != self.epoch {
            return Ok(());
        }
        let mut next = self.dub.clone();
        if let Some(run) = next.run.as_mut() {
            for n in rebuilt {
                if let Some(s) = run.sections.get_mut(*n) {
                    s.mix_stale = false;
                }
            }
        }
        if next.run == self.dub.run {
            return Ok(());
        }
        (self.saver)(&next)?;
        self.dub = next;
        Ok(())
    }
}

// --- subtitle import -----------------------------------------------------------------------

impl EditSession {
    pub fn set_preview_ttl(&mut self, ttl: std::time::Duration) {
        self.preview_ttl = ttl;
    }

    /// Compare a subtitle file with the lines and keep it for `import_apply`. Changes nothing.
    pub fn import_preview(&mut self, file: SubtitleFile) -> ImportPreview {
        let now = std::time::Instant::now();
        let ttl = self.preview_ttl;
        self.previews
            .retain(|_, (made, _)| now.duration_since(*made) < ttl);
        let id = uuid::Uuid::new_v4().to_string();
        let preview = import::preview(&self.dub.lines, &file, id.clone());
        self.previews.insert(id, (now, file));
        preview
    }

    /// Apply a previewed file by one of the choices, as one undo step.
    pub fn import_apply(
        &mut self,
        preview_id: &str,
        choice: ImportChoice,
    ) -> Result<EditResult, AppError> {
        let expired = || {
            AppError::new(
                crate::error::SUBTITLE_PREVIEW_EXPIRED,
                "editor.error.subtitlePreviewExpired",
            )
        };
        let (made, file) = self.previews.get(preview_id).cloned().ok_or_else(expired)?;
        if made.elapsed() >= self.preview_ttl {
            self.previews.remove(preview_id);
            return Err(expired());
        }
        let result = self.apply(EditOp::ImportSubtitles {
            choice,
            cues: file.cues,
        })?;
        self.previews.remove(preview_id);
        Ok(result)
    }
}

impl EditSession {
    /// Choose the caption template of the project. Not an undo step.
    pub fn set_caption_template(&mut self, id: &str) -> Result<(), AppError> {
        if crate::subtitles::templates::find(id).is_none() {
            return Err(AppError::new(
                crate::error::EDIT_NOT_FOUND,
                "editor.error.notFound",
            ));
        }
        let mut next = self.dub.clone();
        next.caption_template = id.to_string();
        (self.saver)(&next)?;
        self.dub = next;
        Ok(())
    }
}

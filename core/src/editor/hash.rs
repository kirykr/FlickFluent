//! The speech input hash and the "changed" set (research R1). A line is changed when the clip it
//! has was not made from the inputs it has now. Nothing here is stored as a flag.
//!
//! `speechHash` is `<raw>.<post>`. `raw` covers what the engine is given (text, language, voice,
//! speaker, gender, engine); `post` covers speed, volume and pitch. A different `raw` means the
//! engine must run again; only a different `post` means the ffmpeg chain must run again.

use sha2::{Digest, Sha256};

use super::runtime::EditorRuntime;
use crate::pipeline::plan::ModelPlan;
use crate::pipeline::run::voice_pick;
use crate::project::dub::{Speaker, SpeakerGender, TranscriptLine};

fn digest(parts: &[&str], len: usize) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    h.finalize()
        .iter()
        .take(len / 2)
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn gender_name(g: SpeakerGender) -> &'static str {
    match g {
        SpeakerGender::Male => "male",
        SpeakerGender::Female => "female",
        SpeakerGender::Unknown => "unknown",
    }
}

/// What the line is spoken from: the translation, or the original when there is none.
pub fn spoken_text(line: &TranscriptLine) -> &str {
    line.translated.as_deref().unwrap_or(&line.text)
}

/// Who speaks a line and how: the voice (empty for a cloud step, which has none of ours) and
/// the engine id (`local:<model>` or `cloud:<service>`). A run and the editor both call this, so
/// the hash a run records is the hash the editor computes.
pub fn speech_identity(
    plan: &ModelPlan,
    speakers: &[Speaker],
    cloud_speak: Option<&str>,
    speaker_id: &str,
) -> (String, String) {
    let pick = voice_pick(plan, speakers, speaker_id);
    match cloud_speak {
        Some(service) => (String::new(), format!("cloud:{service}")),
        None => (
            pick.as_ref()
                .map(|v| format!("{}#{}#{}", v.model, v.speaker_id, v.lang))
                .unwrap_or_default(),
            format!("local:{}", pick.map(|v| v.model).unwrap_or_default()),
        ),
    }
}

/// The engine-facing part from its plain inputs.
pub fn raw_hash_of(
    line: &TranscriptLine,
    gender: SpeakerGender,
    target_language: &str,
    voice_id: &str,
    engine_id: &str,
) -> String {
    digest(
        &[
            spoken_text(line),
            target_language,
            voice_id,
            &line.speaker_id,
            gender_name(gender),
            engine_id,
        ],
        16,
    )
}

/// The whole hash from its plain inputs.
pub fn speech_hash_of(
    line: &TranscriptLine,
    gender: SpeakerGender,
    target_language: &str,
    voice_id: &str,
    engine_id: &str,
) -> String {
    format!(
        "{}.{}",
        raw_hash_of(line, gender, target_language, voice_id, engine_id),
        post_hash(line)
    )
}

/// The engine-facing part: change it and the engine has to run again.
pub fn raw_hash(rt: &EditorRuntime, speakers: &[Speaker], line: &TranscriptLine) -> String {
    let gender = speakers
        .iter()
        .find(|s| s.id == line.speaker_id)
        .map(|s| s.gender)
        .unwrap_or_default();
    let voice = rt.voice_id(speakers, &line.speaker_id);
    let engine = rt.speech_engine_in(speakers, &line.speaker_id);
    raw_hash_of(line, gender, rt.target_language(), &voice, &engine)
}

fn post_hash(line: &TranscriptLine) -> String {
    digest(
        &[
            &format!("{:.3}", line.speed),
            &format!("{:.2}", line.volume_db),
            &format!("{:.2}", line.pitch_semitones),
        ],
        8,
    )
}

/// The whole hash of the inputs a line has now.
pub fn speech_hash(rt: &EditorRuntime, speakers: &[Speaker], line: &TranscriptLine) -> String {
    format!("{}.{}", raw_hash(rt, speakers, line), post_hash(line))
}

/// Eight characters naming a clip file for a given hash.
pub fn hash8(speech_hash: &str) -> String {
    digest(&[speech_hash], 8)
}

fn split(hash: &str) -> (&str, &str) {
    hash.split_once('.').unwrap_or((hash, ""))
}

/// A clip is wanted for a line that has something to say, in a project that speaks.
pub fn wants_clip(rt: &EditorRuntime, line: &TranscriptLine) -> bool {
    rt.speaks() && !spoken_text(line).trim().is_empty()
}

/// The engine has to make this line again.
pub fn is_changed(rt: &EditorRuntime, speakers: &[Speaker], line: &TranscriptLine) -> bool {
    if !wants_clip(rt, line) {
        return false;
    }
    if line.audio.is_none() {
        return true;
    }
    if line.speech_hash.is_empty() {
        return false; // a project from before 004: the clip counts as current
    }
    split(&line.speech_hash).0 != raw_hash(rt, speakers, line)
}

/// The raw clip is current, but speed, volume or pitch differ from what the line clip was made with.
pub fn needs_post(rt: &EditorRuntime, speakers: &[Speaker], line: &TranscriptLine) -> bool {
    if !wants_clip(rt, line) || line.audio.is_none() || line.speech_hash.is_empty() {
        return false;
    }
    let (raw, post) = split(&line.speech_hash);
    raw == raw_hash(rt, speakers, line) && post != post_hash(line)
}

pub fn changed_ids(
    rt: &EditorRuntime,
    speakers: &[Speaker],
    lines: &[TranscriptLine],
) -> Vec<String> {
    lines
        .iter()
        .filter(|l| is_changed(rt, speakers, l))
        .map(|l| l.id.clone())
        .collect()
}

/// Give every line that has a clip and no hash the hash of its current inputs. Returns how many.
pub fn fill_missing(
    rt: &EditorRuntime,
    speakers: &[Speaker],
    lines: &mut [TranscriptLine],
) -> usize {
    let mut filled = 0;
    for line in lines.iter_mut() {
        if line.audio.is_some() && line.speech_hash.is_empty() {
            line.speech_hash = speech_hash(rt, speakers, line);
            filled += 1;
        }
    }
    filled
}

//! The "changed" mark is derived from a hash of what a clip was made from, never stored as a flag
//! (research R1, `data-model.md`).

mod support;

use flickfluent_core::editor::hash::{changed_ids, fill_missing, needs_post, speech_hash};
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::project::dub::{DubData, Speaker, SpeakerGender, TranscriptLine};
use serde_json::json;
use support::cloud_run::use_cloud;
use support::run_fixture::config;

fn speaker(id: &str, gender: SpeakerGender) -> Speaker {
    Speaker {
        id: id.into(),
        gender,
        ..Default::default()
    }
}

fn line(id: &str, speaker: &str, text: &str) -> TranscriptLine {
    TranscriptLine {
        id: id.into(),
        start_ms: 1000,
        end_ms: 3000,
        speaker_id: speaker.into(),
        language: "fr".into(),
        text: "texte".into(),
        translated: Some(text.into()),
        audio: Some(format!("clips/{id}.wav")),
        audio_ms: Some(1500),
        ..Default::default()
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    dub: DubData,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut dub = DubData {
            target_language: "en".into(),
            source_language: "fr".into(),
            speakers: vec![
                speaker("spk1", SpeakerGender::Male),
                speaker("spk2", SpeakerGender::Female),
            ],
            lines: vec![line("l1", "spk1", "Hello"), line("l2", "spk2", "Goodbye")],
            ..Default::default()
        };
        let rt = Self::runtime(&root, &dub);
        for i in 0..dub.lines.len() {
            let h = speech_hash(&rt, &dub.speakers, &dub.lines[i]);
            dub.lines[i].speech_hash = h;
        }
        Self { _root: root, dub }
    }

    fn runtime(root: &tempfile::TempDir, dub: &DubData) -> EditorRuntime {
        EditorRuntime::for_project(config(root.path(), "a.flick", json!({})), dub)
    }

    fn rt(&self) -> EditorRuntime {
        Self::runtime(&self._root, &self.dub)
    }

    fn changed(&self) -> Vec<String> {
        changed_ids(&self.rt(), &self.dub.speakers, &self.dub.lines)
    }
}

#[test]
fn a_fresh_clip_is_not_changed() {
    let f = Fixture::new();
    assert!(f.changed().is_empty());
}

#[test]
fn editing_the_text_marks_the_line_and_editing_back_clears_it() {
    let mut f = Fixture::new();
    f.dub.lines[0].translated = Some("Hello there".into());
    assert_eq!(f.changed(), vec!["l1"]);
    f.dub.lines[0].translated = Some("Hello".into());
    assert!(f.changed().is_empty(), "the old clip is current again");
}

#[test]
fn speaker_gender_voice_and_engine_changes_mark_lines() {
    let mut f = Fixture::new();
    f.dub.lines[0].speaker_id = "spk2".into();
    assert_eq!(f.changed(), vec!["l1"]);

    let mut f = Fixture::new();
    f.dub.speakers[0].gender = SpeakerGender::Female; // another voice, and another gender
    assert_eq!(f.changed(), vec!["l1"]);

    // The engine in the settings changes: every spoken line is changed.
    let mut f = Fixture::new();
    use_cloud(&mut f.dub, None, Some("google-gemini"));
    assert_eq!(f.changed(), vec!["l1", "l2"]);
}

#[test]
fn speed_volume_and_pitch_need_only_the_post_chain() {
    for tweak in [
        (|l: &mut TranscriptLine| l.speed = 1.2) as fn(&mut TranscriptLine),
        |l| l.volume_db = -3.0,
        |l| l.pitch_semitones = 2.0,
    ] {
        let mut f = Fixture::new();
        tweak(&mut f.dub.lines[0]);
        let rt = f.rt();
        assert!(f.changed().is_empty(), "the raw clip is still current");
        assert!(needs_post(&rt, &f.dub.speakers, &f.dub.lines[0]));
        assert!(!needs_post(&rt, &f.dub.speakers, &f.dub.lines[1]));
    }
}

#[test]
fn start_and_end_times_are_not_inputs() {
    let mut f = Fixture::new();
    f.dub.lines[0].start_ms = 500;
    f.dub.lines[0].end_ms = 4000;
    f.dub.lines[0].dub_start_ms = Some(700);
    assert!(f.changed().is_empty());
}

#[test]
fn an_old_project_line_with_a_clip_and_no_hash_is_unchanged_and_open_fills_it() {
    let mut f = Fixture::new();
    for l in &mut f.dub.lines {
        l.speech_hash.clear();
    }
    assert!(f.changed().is_empty());
    let rt = f.rt();
    assert_eq!(fill_missing(&rt, &f.dub.speakers, &mut f.dub.lines), 2);
    assert!(f.dub.lines.iter().all(|l| !l.speech_hash.is_empty()));
    assert!(f.changed().is_empty());
    assert_eq!(fill_missing(&rt, &f.dub.speakers, &mut f.dub.lines), 0);
}

#[test]
fn a_line_with_no_clip_is_changed_unless_it_has_nothing_to_say() {
    let mut f = Fixture::new();
    f.dub.lines[1].audio = None;
    f.dub.lines[1].speech_hash.clear();
    assert_eq!(f.changed(), vec!["l2"]);

    f.dub.lines[1].translated = Some(String::new());
    f.dub.lines[1].text = String::new();
    assert!(f.changed().is_empty(), "empty text has no speech");
}

#[test]
fn a_gender_set_by_the_user_marks_that_speakers_lines_and_is_kept_by_a_later_run() {
    use flickfluent_core::editor::ops::EditOp;
    use flickfluent_core::project::dub::GenderSource;
    let mut f = Fixture::new();
    let rt = f.rt();
    let mut session = flickfluent_core::editor::session::EditSession::open(
        rt,
        f.dub.clone(),
        Box::new(|_| Ok(())),
    )
    .unwrap();
    let r = session
        .apply(EditOp::SetSpeakerGender {
            speaker_id: "spk1".into(),
            gender: SpeakerGender::Female,
        })
        .unwrap();
    let spk = &session.dub().speakers[0];
    assert_eq!(
        (spk.gender, spk.gender_source),
        (SpeakerGender::Female, GenderSource::User)
    );
    assert_eq!(r.changed_ids, vec!["l1"], "only that speaker's lines");
    assert!(session.dub().lines[0].user_edited);
    session.undo().unwrap();
    assert!(session.state().changed_ids.is_empty());
    assert_eq!(
        session.dub().speakers[0].gender_source,
        GenderSource::Detected
    );
    f.dub = session.dub().clone();
}

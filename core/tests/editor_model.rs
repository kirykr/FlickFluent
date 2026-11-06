//! Feature 004 data fields: an older project loads with their defaults and saves back whole.

use flickfluent_core::project::dub::{
    read_from, write_into, DubData, GenderSource, Section, SpeakerGender, TranscriptLine,
};
use flickfluent_core::project::SUPPORTED_FORMAT_VERSION;
use serde_json::json;

/// A project as feature 003 wrote it: none of the 004 fields exist.
fn old_data() -> serde_json::Value {
    json!({
        "notes": "keep me",
        "sourceLanguage": "fr",
        "targetLanguage": "en",
        "speakers": [{ "id": "spk1", "gender": "female", "voiceId": "v1", "confidence": 0.9 }],
        "lines": [{
            "id": "l1", "sectionIndex": 0, "startMs": 1000, "endMs": 2500,
            "speakerId": "spk1", "language": "fr", "text": "Bonjour", "translated": "Hello",
            "audio": "sec0000/clips/l1.wav", "audioMs": 1200, "flags": ["tooLong"],
            "status": "synthesized"
        }],
        "run": {
            "id": "r1", "startedAt": "2026-11-05T10:00:00Z", "state": "completed",
            "settingsHash": "h", "sections": [{ "index": 0, "startMs": 0, "endMs": 5000 }],
            "stages": []
        },
        "cloudUsage": [{ "step": "speak", "serviceId": "gemini", "chars": 12 }]
    })
}

#[test]
fn an_old_project_loads_with_the_004_defaults() {
    let dub = read_from(&old_data());
    let line = &dub.lines[0];
    assert_eq!(line.speed, 1.0);
    assert_eq!(line.volume_db, 0.0);
    assert_eq!(line.pitch_semitones, 0.0);
    assert_eq!(line.dub_start_ms, None);
    assert_eq!(line.raw_audio, None);
    assert_eq!(line.raw_ms, None);
    assert!(line.speech_hash.is_empty());
    assert!(!line.user_edited);
    assert_eq!(dub.speakers[0].gender_source, GenderSource::Detected);
    assert_eq!(dub.speakers[0].gender, SpeakerGender::Female);
    assert_eq!(dub.caption_template, "classic");
    assert!(!dub.run.as_ref().unwrap().sections[0].edited);
}

#[test]
fn saving_back_keeps_every_002_and_003_field() {
    let original = old_data();
    let dub = read_from(&original);
    let mut written = original.clone();
    write_into(&mut written, &dub);

    assert_eq!(written["notes"], "keep me");
    assert_eq!(written["cloudUsage"][0]["chars"], 12);
    let line = &written["lines"][0];
    for key in [
        "id",
        "sectionIndex",
        "startMs",
        "endMs",
        "speakerId",
        "language",
        "text",
        "translated",
        "audio",
        "audioMs",
        "status",
    ] {
        assert_eq!(line[key], original["lines"][0][key], "{key}");
    }
    assert_eq!(line["flags"], json!(["tooLong"]));
    assert_eq!(written["speakers"][0]["voiceId"], "v1");
    assert_eq!(written["run"]["sections"][0]["endMs"], 5000);
    // The new fields are written with their names in camelCase.
    assert_eq!(line["speed"], 1.0);
    assert_eq!(line["volumeDb"], 0.0);
    assert_eq!(line["pitchSemitones"], 0.0);
    assert_eq!(line["speechHash"], "");
    assert_eq!(line["userEdited"], false);
    assert_eq!(written["captionTemplate"], "classic");
    assert_eq!(written["speakers"][0]["genderSource"], "detected");
}

#[test]
fn new_fields_survive_a_round_trip() {
    let mut dub = read_from(&old_data());
    dub.lines[0].speed = 1.2;
    dub.lines[0].dub_start_ms = Some(900);
    dub.lines[0].raw_audio = Some("sec0000/clips/l1.raw.wav".into());
    dub.lines[0].raw_ms = Some(1500);
    dub.lines[0].speech_hash = "abc".into();
    dub.lines[0].user_edited = true;
    dub.speakers[0].gender_source = GenderSource::User;
    dub.caption_template = "soft-box".into();
    dub.run.as_mut().unwrap().sections[0].edited = true;

    let mut data = json!({});
    write_into(&mut data, &dub);
    assert_eq!(read_from(&data), dub);
}

#[test]
fn a_default_line_is_neutral_and_the_format_version_is_unchanged() {
    let line = TranscriptLine::default();
    assert_eq!(
        (line.speed, line.volume_db, line.pitch_semitones),
        (1.0, 0.0, 0.0)
    );
    assert!(!Section::default().edited);
    assert_eq!(DubData::default().caption_template, "classic");
    assert_eq!(SUPPORTED_FORMAT_VERSION, 2);
}

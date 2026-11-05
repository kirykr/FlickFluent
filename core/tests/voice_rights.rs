//! A voice the project trains itself ships only with a complete, granted rights record. Being
//! public (on the radio, on the web) is not permission: the test rejects it by name.

use std::path::{Path, PathBuf};

use flickfluent_core::models::rights::{check, Problem, RightsRecord};
use flickfluent_core::models::{bundled_catalogue, LicenceClass};

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

fn fixture(name: &str) -> RightsRecord {
    let path = repo(&format!("core/tests/fixtures/voice-rights/{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn problems(name: &str) -> Vec<Problem> {
    check(&fixture(name))
}

#[test]
fn a_complete_signed_record_passes() {
    assert_eq!(problems("complete"), vec![]);
}

#[test]
fn a_pending_record_blocks_shipping() {
    assert_eq!(problems("pending"), vec![Problem::NotGranted]);
}

#[test]
fn permission_must_cover_both_training_and_shipping() {
    assert_eq!(problems("no-ship"), vec![Problem::CannotShip]);
}

#[test]
fn someone_must_have_granted_it_with_evidence() {
    assert_eq!(problems("no-holder"), vec![Problem::NoRightsHolder]);
}

#[test]
fn assumed_public_is_not_permission() {
    assert_eq!(problems("assumed-public"), vec![Problem::AssumedPublic]);
}

#[test]
fn every_speaker_must_have_consented_in_writing() {
    assert_eq!(
        problems("speaker-no-consent"),
        vec![Problem::SpeakerWithoutConsent]
    );
    assert_eq!(problems("no-speakers"), vec![Problem::NoSpeakers]);
}

#[test]
fn a_licence_that_does_not_allow_commercial_use_must_say_so() {
    assert_eq!(
        problems("noncommercial-claims-permissive"),
        vec![Problem::NonCommercialNotMarked]
    );
    assert_eq!(problems("noncommercial-ok"), vec![]);
}

#[test]
fn share_alike_is_a_notice_class_not_permissive() {
    assert_eq!(
        problems("share-alike-as-permissive"),
        vec![Problem::ShareAlikeNotMarked]
    );
    assert_eq!(problems("share-alike-ok"), vec![]);
}

#[test]
fn the_schema_accepts_the_fixtures_and_rejects_a_missing_field() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo("config/voice-rights.schema.json")).unwrap(),
    )
    .unwrap();
    for name in [
        "complete",
        "pending",
        "noncommercial-ok",
        "share-alike-ok",
        "assumed-public",
    ] {
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo(&format!(
                "core/tests/fixtures/voice-rights/{name}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        assert!(jsonschema::is_valid(&schema, &v), "{name}");
    }
    let mut bad: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo("core/tests/fixtures/voice-rights/complete.json")).unwrap(),
    )
    .unwrap();
    bad.as_object_mut().unwrap().remove("speakers");
    assert!(!jsonschema::is_valid(&schema, &bad));
}

/// Every catalogue entry that names a rights record must have one that passes and agrees with
/// the entry. This is the gate: a project-trained voice cannot be added without it.
#[test]
fn every_project_trained_voice_in_the_catalogue_has_a_passing_record() {
    for entry in bundled_catalogue()
        .iter()
        .filter(|e| e.rights_record.is_some())
    {
        let rel = entry.rights_record.as_deref().unwrap();
        let text = std::fs::read_to_string(repo(rel))
            .unwrap_or_else(|_| panic!("{}: rights record {rel} is missing", entry.id));
        let record: RightsRecord = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: rights record does not parse: {e}", entry.id));
        assert_eq!(
            record.voice_id, entry.id,
            "{}: record names another voice",
            entry.id
        );
        assert_eq!(
            check(&record),
            vec![],
            "{}: rights record incomplete",
            entry.id
        );
        assert_eq!(
            format!("{:?}", entry.licence_class).to_lowercase(),
            record.licence.class.to_lowercase(),
            "{}: entry and record disagree on the licence class",
            entry.id
        );
        if entry.licence_class == LicenceClass::NonCommercial {
            assert!(
                !entry.default_enabled,
                "{}: non-commercial must be opt-in",
                entry.id
            );
        }
        assert!(
            entry.licence_notice_key.is_some() || entry.licence_class == LicenceClass::Permissive
        );
    }
}

/// A record that is not granted may exist (a request in progress) but must not be referenced.
#[test]
fn records_on_disk_are_valid_json_for_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo("config/voice-rights.schema.json")).unwrap(),
    )
    .unwrap();
    for e in std::fs::read_dir(repo("config/voice-rights"))
        .unwrap()
        .flatten()
    {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "json") {
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert!(jsonschema::is_valid(&schema, &v), "{}", path.display());
        }
    }
}

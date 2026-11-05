//! The cloud service data follows the contract schema and points at texts that exist.

use serde_json::Value;

fn services() -> Value {
    let text = include_str!("../../config/cloud-services.json");
    serde_json::from_str(text).expect("cloud services are JSON")
}

fn schema() -> Value {
    let text =
        include_str!("../../specs/003-hybrid-cloud-keys/contracts/cloud-services.schema.json");
    serde_json::from_str(text).expect("schema is JSON")
}

#[test]
fn every_service_validates_against_the_schema() {
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&services())
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    assert!(errors.is_empty(), "schema errors:\n{}", errors.join("\n"));
}

#[test]
fn every_notice_key_exists_in_the_english_texts() {
    let en: Value = serde_json::from_str(include_str!("../../src/i18n/en.json")).unwrap();
    for service in services().as_array().unwrap() {
        for (step, key) in service["dataNotice"].as_object().unwrap() {
            let key = key.as_str().unwrap();
            assert!(
                key.split('.')
                    .try_fold(&en, |v, part| v.get(part))
                    .is_some(),
                "{} {step} notice key {key} is missing from src/i18n/en.json",
                service["id"]
            );
        }
    }
}

#[test]
fn ids_are_unique() {
    let all = services();
    let mut ids: Vec<&str> = all
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate service id");
}

#[test]
fn a_service_with_a_voice_list_has_both_genders() {
    for service in services().as_array().unwrap() {
        let Some(voices) = service.get("voices").and_then(Value::as_array) else {
            continue;
        };
        for gender in ["female", "male"] {
            assert!(
                voices.iter().any(|v| v["gender"] == gender),
                "{} lists no {gender} voice",
                service["id"]
            );
        }
    }
}

#[test]
fn the_shipped_data_loads_through_the_core() {
    let all = flickfluent_core::cloud::services::load().unwrap();
    assert!(all.iter().any(
        |s| s.id == "google-gemini" && s.offers(flickfluent_core::cloud::services::Step::Speak)
    ));
    let azure = flickfluent_core::cloud::services::find(&all, "azure-ai").unwrap();
    assert!(azure.needs_region && azure.voices.is_none());
    assert_eq!(
        azure.url("https://{region}.x.com", Some("westeurope")),
        "https://westeurope.x.com"
    );
}

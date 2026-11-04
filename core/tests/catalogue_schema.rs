//! Every catalogue entry follows the contract schema, and the licence rules hold.

use serde_json::Value;

fn catalogue() -> Value {
    let text = include_str!("../../config/model-catalogue.json");
    serde_json::from_str(text).expect("catalogue is JSON")
}

fn schema() -> Value {
    let text = include_str!(
        "../../specs/002-core-dub-pipeline/contracts/language-capabilities.schema.json"
    );
    serde_json::from_str(text).expect("schema is JSON")
}

#[test]
fn every_entry_validates_against_the_schema() {
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&catalogue())
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    assert!(errors.is_empty(), "schema errors:\n{}", errors.join("\n"));
}

#[test]
fn non_commercial_entries_are_opt_in_with_a_notice() {
    for entry in catalogue().as_array().unwrap() {
        if entry["licenceClass"] != "permissive" {
            assert!(
                entry["licenceNoticeKey"].is_string(),
                "{} needs a licenceNoticeKey",
                entry["id"]
            );
        }
        if entry["licenceClass"] == "nonCommercial" {
            assert_eq!(
                entry["defaultEnabled"],
                Value::Bool(false),
                "{} must be opt-in",
                entry["id"]
            );
        }
    }
}

#[test]
fn ids_are_unique_and_requires_resolve() {
    let cat = catalogue();
    let entries = cat.as_array().unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate ids");
    for e in entries {
        for r in e["requires"].as_array().into_iter().flatten() {
            assert!(
                ids.contains(&r.as_str().unwrap()),
                "{} requires unknown {}",
                e["id"],
                r
            );
        }
    }
}

#[test]
fn bundled_catalogue_loads_into_rust_types() {
    let list = flickfluent_core::models::bundled_catalogue();
    assert!(!list.is_empty());
}

use std::path::PathBuf;

fn repo_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(rel)
}

#[test]
fn spec_definition_matches_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_file(
            "specs/001-app-shell-hardware-check/contracts/spec-definition.schema.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let instance: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_file("config/spec-definition.json")).unwrap(),
    )
    .unwrap();
    assert!(jsonschema::is_valid(&schema, &instance));
}

#[test]
fn bundled_spec_loads() {
    let spec = flickfluent_core::hardware::SpecDefinition::bundled();
    assert!(spec.provisional);
    assert!(spec.full_local.cpu_cores >= spec.minimum.cpu_cores);
    assert!(spec.full_local.ram_gb >= spec.minimum.ram_gb);
    assert!(spec.full_local.free_disk_gb >= spec.minimum.free_disk_gb);
}

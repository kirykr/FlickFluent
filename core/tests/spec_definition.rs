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
    assert!(spec.full_local.cpu_cores >= spec.minimum.cpu_cores);
    assert!(spec.full_local.ram_gb >= spec.minimum.ram_gb);
    assert!(spec.full_local.free_disk_gb >= spec.minimum.free_disk_gb);
}

fn flatten(spec: &serde_json::Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for tier in ["minimum", "fullLocal"] {
        let t = &spec[tier];
        for key in ["cpuBits", "cpuCores", "ramGb", "freeDiskGb", "accelerator"] {
            out.push((
                format!("{tier}.{key}"),
                t[key].to_string().trim_matches('"').to_string(),
            ));
        }
        for os in ["windows", "macos"] {
            out.push((
                format!("{tier}.os.{os}"),
                t["os"][os].as_str().unwrap().to_string(),
            ));
        }
    }
    out
}

/// Rows `| key | value | records |` of the spec table in the benchmark doc.
fn doc_rows() -> Vec<(String, String, String)> {
    let text = std::fs::read_to_string(repo_file("docs/verification/002-benchmarks.md"))
        .expect("docs/verification/002-benchmarks.md exists");
    text.lines()
        .filter(|l| l.starts_with("| minimum.") || l.starts_with("| fullLocal."))
        .map(|l| {
            let cols: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            (
                cols[0].to_string(),
                cols.get(1).unwrap_or(&"").to_string(),
                cols.get(2).unwrap_or(&"").to_string(),
            )
        })
        .collect()
}

fn instance() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(repo_file("config/spec-definition.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn the_benchmark_doc_states_every_spec_value_as_it_is_in_the_file() {
    let rows = doc_rows();
    for (key, value) in flatten(&instance()) {
        let row = rows
            .iter()
            .find(|(k, _, _)| *k == key)
            .unwrap_or_else(|| panic!("{key} has no row in 002-benchmarks.md"));
        assert_eq!(row.1, value, "{key}: the doc and the file disagree");
    }
}

#[test]
fn a_final_spec_cites_a_saved_record_for_every_value() {
    let spec = flickfluent_core::hardware::SpecDefinition::bundled();
    if spec.provisional {
        // Not final: it must say what its numbers rest on.
        assert!(spec.basis.is_some(), "a provisional spec needs a `basis`");
        return;
    }
    assert_eq!(spec.basis.as_deref(), Some("measured"));
    let records = repo_file("docs/verification/benchmarks");
    for (key, _, cited) in doc_rows() {
        assert!(
            cited
                .split([',', ' '])
                .any(|name| name.starts_with("bench-")
                    && records.join(name.trim_matches('`')).is_file()),
            "{key} cites no record file in docs/verification/benchmarks"
        );
    }
}

#[test]
fn a_spec_that_is_not_final_may_only_be_simulated_or_one_os() {
    // `provisional: false` is allowed only with records from both operating systems.
    let spec = flickfluent_core::hardware::SpecDefinition::bundled();
    if spec.provisional {
        return;
    }
    let dir = repo_file("docs/verification/benchmarks");
    let mut oses = std::collections::BTreeSet::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("bench-") {
            if !rest.contains("-simulated-") {
                oses.insert(rest.split('-').next().unwrap().to_string());
            }
        }
    }
    assert!(
        oses.contains("macos") && oses.contains("windows"),
        "final spec needs real records from both OSes, found {oses:?}"
    );
}

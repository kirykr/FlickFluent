use flickfluent_core::hardware::{evaluate, MachineInput, SpecDefinition};

fn fixture(name: &str) -> MachineInput {
    let path = format!(
        "{}/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn message_lists_every_missing_item_with_required_and_found() {
    let report = evaluate(&fixture("low"), &SpecDefinition::bundled());
    assert_eq!(
        report.blocked_message.as_deref(),
        Some(
            "Your computer is not compatible with this app. You need 4 CPU cores (found 2), \
             8 GB RAM (found 4 GB) and 10 GB free disk space (found 5 GB) to run this app."
        )
    );
}

#[test]
fn single_missing_item_has_no_and() {
    let mut m = fixture("mid");
    m.ram_gb = Some(4.0);
    let report = evaluate(&m, &SpecDefinition::bundled());
    assert_eq!(
        report.blocked_message.as_deref(),
        Some(
            "Your computer is not compatible with this app. You need 8 GB RAM (found 4 GB) \
             to run this app."
        )
    );
}

#[test]
fn no_message_when_not_blocked() {
    let report = evaluate(&fixture("mid"), &SpecDefinition::bundled());
    assert!(report.blocked_message.is_none());
}

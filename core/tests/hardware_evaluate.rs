use flickfluent_core::hardware::{evaluate, ItemStatus, MachineInput, Mode, SpecDefinition};

fn fixture(name: &str) -> MachineInput {
    let path = format!(
        "{}/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn low_machine_is_blocked() {
    let report = evaluate(&fixture("low"), &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Blocked);
    let keys: Vec<&str> = report.missing.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys, vec!["cpuCores", "ramGb", "freeDiskGb"]);
}

#[test]
fn mid_machine_is_hybrid() {
    let report = evaluate(&fixture("mid"), &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Hybrid);
    assert!(report.missing.is_empty());
}

#[test]
fn high_machine_is_full_local() {
    let report = evaluate(&fixture("high"), &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::FullLocal);
    assert!(report.items.iter().all(|i| i.status == ItemStatus::Ok));
}

#[test]
fn unknown_value_goes_to_hybrid_not_blocked() {
    let report = evaluate(&fixture("unknown-gpu"), &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Hybrid);
    let accel = report
        .items
        .iter()
        .find(|i| i.key == "accelerator")
        .unwrap();
    assert_eq!(accel.status, ItemStatus::Unknown);
    assert_eq!(accel.detected, "unknown");
}

#[test]
fn unreadable_ram_never_panics_and_is_cautious() {
    let mut m = fixture("high");
    m.ram_gb = None;
    let report = evaluate(&m, &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Hybrid);
}

#[test]
fn eight_gb_machine_reporting_slightly_less_is_not_blocked() {
    // Windows often reports about 7.9 GB usable on an 8 GB machine.
    let mut m = fixture("mid");
    m.ram_gb = Some(7.9);
    let report = evaluate(&m, &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Hybrid);
}

#[test]
fn unsupported_os_is_blocked() {
    let mut m = fixture("high");
    m.os.as_mut().unwrap().family = "linux".into();
    let report = evaluate(&m, &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Blocked);
}

#[test]
fn old_macos_is_blocked() {
    let mut m = fixture("high");
    m.os.as_mut().unwrap().version = Some("11".into());
    let report = evaluate(&m, &SpecDefinition::bundled());
    assert_eq!(report.mode, Mode::Blocked);
}

#[test]
fn real_probe_does_not_panic() {
    let input = flickfluent_core::hardware::probe();
    let report = evaluate(&input, &SpecDefinition::bundled());
    assert!(!report.items.is_empty());
}

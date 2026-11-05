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

// --- the spec values: at, just below and just above each tier ----------------------------------
//
// The machines are built from `config/spec-definition.json`, so these tests keep holding when the
// numbers change after a benchmark pass. The words of the message are fixed.

use flickfluent_core::hardware::{Accelerator, OsInfo, SpecTier};

fn machine_of(tier: &SpecTier, accelerator: Accelerator) -> MachineInput {
    MachineInput {
        cpu_bits: Some(tier.cpu_bits),
        cpu_cores: Some(tier.cpu_cores),
        ram_gb: Some(tier.ram_gb),
        free_disk_gb: Some(tier.free_disk_gb),
        os: Some(OsInfo {
            family: "macos".into(),
            version: Some(tier.os.macos.clone()),
        }),
        accelerator: Some(accelerator),
    }
}

fn n(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

const BLOCKED: &str = "Your computer is not compatible with this app. You need ";

#[test]
fn a_machine_exactly_at_the_minimum_runs_hybrid() {
    let spec = SpecDefinition::bundled();
    let report = evaluate(&machine_of(&spec.minimum, Accelerator::None), &spec);
    assert_eq!(report.mode, Mode::Hybrid);
    assert!(report.missing.is_empty() && report.blocked_message.is_none());
}

#[test]
fn a_machine_just_below_the_minimum_is_blocked_with_the_exact_message() {
    let spec = SpecDefinition::bundled();
    let min = &spec.minimum;

    let mut m = machine_of(min, Accelerator::None);
    m.cpu_cores = Some(min.cpu_cores - 1);
    assert_eq!(
        evaluate(&m, &spec).blocked_message.unwrap(),
        format!(
            "{BLOCKED}{} CPU cores (found {}) to run this app.",
            min.cpu_cores,
            min.cpu_cores - 1
        )
    );

    // Windows may report a little less than the installed RAM, so "just below" is clearly below
    // the tolerance, not one tenth under.
    let mut m = machine_of(min, Accelerator::None);
    m.ram_gb = Some(min.ram_gb - 2.0);
    assert_eq!(
        evaluate(&m, &spec).blocked_message.unwrap(),
        format!(
            "{BLOCKED}{} GB RAM (found {} GB) to run this app.",
            n(min.ram_gb),
            n(min.ram_gb - 2.0)
        )
    );

    let mut m = machine_of(min, Accelerator::None);
    m.free_disk_gb = Some(min.free_disk_gb - 0.5);
    assert_eq!(
        evaluate(&m, &spec).blocked_message.unwrap(),
        format!(
            "{BLOCKED}{} GB free disk space (found {} GB) to run this app.",
            n(min.free_disk_gb),
            n(min.free_disk_gb - 0.5)
        )
    );
}

#[test]
fn several_shortfalls_are_listed_together_in_order() {
    let spec = SpecDefinition::bundled();
    let min = &spec.minimum;
    let mut m = machine_of(min, Accelerator::None);
    m.cpu_cores = Some(min.cpu_cores - 1);
    m.ram_gb = Some(min.ram_gb - 2.0);
    m.free_disk_gb = Some(min.free_disk_gb - 0.5);
    assert_eq!(
        evaluate(&m, &spec).blocked_message.unwrap(),
        format!(
            "{BLOCKED}{} CPU cores (found {}), {} GB RAM (found {} GB) and {} GB free disk space (found {} GB) to run this app.",
            min.cpu_cores,
            min.cpu_cores - 1,
            n(min.ram_gb),
            n(min.ram_gb - 2.0),
            n(min.free_disk_gb),
            n(min.free_disk_gb - 0.5),
        )
    );
}

#[test]
fn an_older_os_is_blocked_by_name() {
    let spec = SpecDefinition::bundled();
    let mut m = machine_of(&spec.minimum, Accelerator::None);
    m.os = Some(OsInfo {
        family: "macos".into(),
        version: Some("9".into()),
    });
    let msg = evaluate(&m, &spec).blocked_message.unwrap();
    assert_eq!(
        msg,
        format!(
            "{BLOCKED}macOS {} or newer (found macOS 9) to run this app.",
            spec.minimum.os.macos
        )
    );
}

#[test]
fn a_machine_at_and_above_the_full_local_spec_runs_full_local() {
    let spec = SpecDefinition::bundled();
    let at = machine_of(&spec.full_local, Accelerator::AppleSilicon);
    assert_eq!(evaluate(&at, &spec).mode, Mode::FullLocal);

    let mut above = at.clone();
    above.cpu_cores = Some(spec.full_local.cpu_cores * 2);
    above.ram_gb = Some(spec.full_local.ram_gb * 2.0);
    above.free_disk_gb = Some(spec.full_local.free_disk_gb * 2.0);
    assert_eq!(evaluate(&above, &spec).mode, Mode::FullLocal);
}

#[test]
fn just_below_full_local_is_hybrid_never_blocked() {
    let spec = SpecDefinition::bundled();
    let full = &spec.full_local;
    for tweak in [
        (|m: &mut MachineInput, f: &SpecTier| m.cpu_cores = Some(f.cpu_cores - 1))
            as fn(&mut MachineInput, &SpecTier),
        |m, f| m.ram_gb = Some(f.ram_gb - 2.0),
        |m, f| m.free_disk_gb = Some(f.free_disk_gb - 1.0),
        |m, _| m.accelerator = Some(Accelerator::None),
    ] {
        let mut m = machine_of(full, Accelerator::AppleSilicon);
        tweak(&mut m, full);
        let report = evaluate(&m, &spec);
        assert_eq!(report.mode, Mode::Hybrid, "{:?}", report.items);
        assert!(report.missing.is_empty() && report.blocked_message.is_none());
    }
}

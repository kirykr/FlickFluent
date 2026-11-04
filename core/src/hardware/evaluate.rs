use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::message::blocked_message;
use super::{
    Accelerator, AcceleratorRequirement, CheckItem, ItemStatus, MachineCapabilityReport,
    MachineInput, Mode, SpecDefinition,
};

/// Windows often reports a little less than the installed RAM (reserved memory), so an 8 GB
/// machine can read as 7.9 GB. Accept 93% of the stated amount.
const RAM_TOLERANCE: f64 = 0.93;

pub fn evaluate(machine: &MachineInput, spec: &SpecDefinition) -> MachineCapabilityReport {
    let items = vec![
        cpu_bits_item(machine, spec),
        numeric_item(
            "cpuCores",
            machine.cpu_cores.map(f64::from),
            spec.minimum.cpu_cores.into(),
            spec.full_local.cpu_cores.into(),
            1.0,
            |n| format!("{n} CPU cores"),
            |n| n.to_string(),
            |n| n.to_string(),
        ),
        numeric_item(
            "ramGb",
            machine.ram_gb,
            spec.minimum.ram_gb,
            spec.full_local.ram_gb,
            RAM_TOLERANCE,
            |n| format!("{n} GB RAM"),
            |n| format!("{n} GB"),
            |n| format!("{n} GB"),
        ),
        numeric_item(
            "freeDiskGb",
            machine.free_disk_gb,
            spec.minimum.free_disk_gb,
            spec.full_local.free_disk_gb,
            1.0,
            |n| format!("{n} GB free disk space"),
            |n| format!("{n} GB"),
            |n| format!("{n} GB"),
        ),
        os_item(machine, spec),
        accelerator_item(machine, spec),
    ];

    let missing: Vec<CheckItem> = items
        .iter()
        .filter(|i| i.status == ItemStatus::BelowMinimum)
        .cloned()
        .collect();
    let mode = if !missing.is_empty() {
        Mode::Blocked
    } else if items
        .iter()
        .any(|i| matches!(i.status, ItemStatus::BelowFull | ItemStatus::Unknown))
    {
        Mode::Hybrid
    } else {
        Mode::FullLocal
    };
    let blocked_message = (mode == Mode::Blocked).then(|| blocked_message(&missing));

    MachineCapabilityReport {
        items,
        mode,
        missing,
        blocked_message,
        provisional: spec.provisional,
        generated_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
    }
}

fn fmt_num(n: f64) -> String {
    if (n - n.round()).abs() < 0.05 {
        format!("{}", n.round() as i64)
    } else {
        format!("{n:.1}")
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric_item(
    key: &str,
    detected: Option<f64>,
    min: f64,
    full: f64,
    tolerance: f64,
    needed: impl Fn(&str) -> String,
    required: impl Fn(&str) -> String,
    found: impl Fn(&str) -> String,
) -> CheckItem {
    let min_s = fmt_num(min);
    let (status, detected_s) = match detected {
        None => (ItemStatus::Unknown, "unknown".to_string()),
        Some(d) => {
            let status = if d < min * tolerance {
                ItemStatus::BelowMinimum
            } else if d < full * tolerance {
                ItemStatus::BelowFull
            } else {
                ItemStatus::Ok
            };
            (status, fmt_num(d))
        }
    };
    CheckItem {
        key: key.to_string(),
        required: required(&min_s),
        detected: detected_s.clone(),
        status,
        needed_label: needed(&min_s),
        found_label: found(&detected_s),
    }
}

fn cpu_bits_item(machine: &MachineInput, spec: &SpecDefinition) -> CheckItem {
    let (status, detected) = match machine.cpu_bits {
        None => (ItemStatus::Unknown, "unknown".to_string()),
        Some(b) if b >= spec.minimum.cpu_bits => (ItemStatus::Ok, b.to_string()),
        Some(b) => (ItemStatus::BelowMinimum, b.to_string()),
    };
    CheckItem {
        key: "cpuBits".into(),
        required: spec.minimum.cpu_bits.to_string(),
        found_label: format!("{detected}-bit"),
        detected,
        status,
        needed_label: format!("a {}-bit processor", spec.minimum.cpu_bits),
    }
}

/// First run of digits in a version string: `14.5` -> 14, `11 (22631)` -> 11.
fn major_version(v: &str) -> Option<u32> {
    v.split(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?
        .parse()
        .ok()
}

fn os_item(machine: &MachineInput, spec: &SpecDefinition) -> CheckItem {
    let unsupported = |family: &str| CheckItem {
        key: "os".into(),
        required: format!(
            "Windows {} or macOS {} or newer",
            spec.minimum.os.windows, spec.minimum.os.macos
        ),
        detected: family.to_string(),
        status: ItemStatus::BelowMinimum,
        needed_label: format!(
            "Windows {} or macOS {} or newer",
            spec.minimum.os.windows, spec.minimum.os.macos
        ),
        found_label: family.to_string(),
    };
    let Some(os) = &machine.os else {
        return CheckItem {
            key: "os".into(),
            required: String::new(),
            detected: "unknown".into(),
            status: ItemStatus::Unknown,
            needed_label: String::new(),
            found_label: "unknown".into(),
        };
    };
    let (name, required) = match os.family.as_str() {
        "windows" => ("Windows", &spec.minimum.os.windows),
        "macos" => ("macOS", &spec.minimum.os.macos),
        other => return unsupported(other),
    };
    let needed_label = format!("{name} {required} or newer");
    let (status, detected) = match (
        os.version.as_deref().and_then(major_version),
        major_version(required),
    ) {
        (Some(have), Some(need)) if have >= need => (ItemStatus::Ok, format!("{name} {have}")),
        (Some(have), Some(_)) => (ItemStatus::BelowMinimum, format!("{name} {have}")),
        _ => (ItemStatus::Unknown, "unknown".to_string()),
    };
    CheckItem {
        key: "os".into(),
        required: needed_label.clone(),
        found_label: detected.clone(),
        detected,
        status,
        needed_label,
    }
}

fn accelerator_item(machine: &MachineInput, spec: &SpecDefinition) -> CheckItem {
    let needs_accelerator =
        spec.full_local.accelerator == AcceleratorRequirement::AppleSiliconOrDiscreteGpu;
    let (status, detected) = match machine.accelerator {
        None => (ItemStatus::Unknown, "unknown"),
        Some(Accelerator::AppleSilicon) => (ItemStatus::Ok, "Apple Silicon"),
        Some(Accelerator::DiscreteGpu) => (ItemStatus::Ok, "discrete GPU"),
        Some(Accelerator::None) if needs_accelerator => (ItemStatus::BelowFull, "none"),
        Some(Accelerator::None) => (ItemStatus::Ok, "none"),
    };
    CheckItem {
        key: "accelerator".into(),
        // The minimum spec needs no accelerator; the full-local spec does.
        required: "Apple Silicon or discrete GPU (full local only)".into(),
        detected: detected.to_string(),
        status,
        needed_label: "Apple Silicon or a discrete GPU".into(),
        found_label: detected.to_string(),
    }
}

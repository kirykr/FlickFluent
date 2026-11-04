//! Preflight reports missing models, disk and machine limits; `check_start` turns them into the
//! stable error codes.

use flickfluent_core::error::{MACHINE_TOO_WEAK, MODELS_MISSING, NO_FALLBACK_CHOSEN, NO_SPACE};
use flickfluent_core::models::{record_installed, CatalogueEntry, Gender, MachineClass, Step};
use flickfluent_core::pipeline::plan::plan_models;
use flickfluent_core::pipeline::preflight::{check_start, preflight, Preflight};

fn e(id: &str, step: Step, class: MachineClass, languages: &[&str], size: u64) -> CatalogueEntry {
    CatalogueEntry {
        id: id.into(),
        name: id.into(),
        step,
        machine_class: class,
        languages: languages.iter().map(|s| s.to_string()).collect(),
        size_bytes: size,
        default_enabled: true,
        ..Default::default()
    }
}

fn catalogue() -> Vec<CatalogueEntry> {
    let mut voice = e("voice-en", Step::Speak, MachineClass::Minimum, &["en"], 63);
    voice.gender = Some(Gender::Female);
    vec![
        e("runtime", Step::Runtime, MachineClass::Minimum, &[], 25),
        e(
            "whisper-small",
            Step::Transcribe,
            MachineClass::Minimum,
            &["fr"],
            486,
        ),
        e(
            "whisper-large",
            Step::Transcribe,
            MachineClass::FullLocal,
            &["fr"],
            3000,
        ),
        e(
            "m2m",
            Step::Translate,
            MachineClass::Minimum,
            &["fr", "en"],
            977,
        ),
        e("demucs", Step::Separate, MachineClass::Mid, &[], 84),
        voice,
    ]
}

fn run(class: MachineClass, fast: bool, models: &std::path::Path) -> Preflight {
    let cat = catalogue();
    let plan = plan_models(&cat, class, "macos", "arm64", "fr", "en", fast);
    preflight(
        &cat, &plan, models, models, class, "macos", "arm64", "fr", "en", fast, 120_000,
    )
}

#[test]
fn lists_what_is_missing_with_sizes_and_skips_what_is_installed() {
    let dir = tempfile::tempdir().unwrap();
    let cat = catalogue();
    let pre = run(MachineClass::Mid, false, dir.path());
    let ids: Vec<&str> = pre.missing_models.iter().map(|m| m.id.as_str()).collect();
    for want in ["runtime", "whisper-small", "m2m", "demucs", "voice-en"] {
        assert!(ids.contains(&want), "{want} in {ids:?}");
    }
    assert_eq!(pre.download_bytes, 25 + 486 + 977 + 84 + 63);

    let file = dir.path().join("x");
    std::fs::write(&file, b"x").unwrap();
    record_installed(
        dir.path(),
        cat.iter().find(|c| c.id == "runtime").unwrap(),
        &file,
    )
    .unwrap();
    let again = run(MachineClass::Mid, false, dir.path());
    assert!(!again.missing_models.iter().any(|m| m.id == "runtime"));
}

#[test]
fn a_weak_machine_is_named_and_fast_quality_avoids_it() {
    let dir = tempfile::tempdir().unwrap();
    let weak = run(MachineClass::Minimum, false, dir.path());
    assert_eq!(check_start(&weak).unwrap_err().code, MACHINE_TOO_WEAK);
    assert!(weak
        .blockers
        .iter()
        .any(|b| b.step == Step::Separate && b.reason == "machine"));
    let fast = run(MachineClass::Minimum, true, dir.path());
    assert!(fast.blockers.is_empty());
    assert_eq!(fast.warnings, vec!["dub.warn.fastNoMusic".to_string()]);
}

#[test]
fn an_unsupported_language_is_a_fallback_question_not_a_machine_limit() {
    let dir = tempfile::tempdir().unwrap();
    let cat = catalogue();
    let plan = plan_models(
        &cat,
        MachineClass::FullLocal,
        "macos",
        "arm64",
        "fr",
        "km",
        true,
    );
    let pre = preflight(
        &cat,
        &plan,
        dir.path(),
        dir.path(),
        MachineClass::FullLocal,
        "macos",
        "arm64",
        "fr",
        "km",
        true,
        1000,
    );
    assert!(pre.blockers.iter().all(|b| b.reason == "language"));
    assert_eq!(check_start(&pre).unwrap_err().code, NO_FALLBACK_CHOSEN);
}

#[test]
fn missing_models_and_a_full_disk_give_their_codes_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut pre = run(MachineClass::Mid, false, dir.path());
    assert_eq!(check_start(&pre).unwrap_err().code, MODELS_MISSING);
    pre.missing_models.clear();
    pre.disk_free_bytes = Some(10);
    let err = check_start(&pre).unwrap_err();
    assert_eq!(err.code, NO_SPACE);
    assert_eq!(err.details.unwrap()["freeBytes"], 10);
    pre.disk_free_bytes = Some(u64::MAX);
    assert!(check_start(&pre).is_ok());
}

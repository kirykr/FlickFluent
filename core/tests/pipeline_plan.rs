//! Model planning: which models a dub needs on this machine, what the disk must hold.

use flickfluent_core::hardware::{Accelerator, MachineInput};
use flickfluent_core::models::{
    CatalogueEntry, Gender, MachineClass, Platforms, Quality, Step, VoiceSpec,
};
use flickfluent_core::pipeline::plan::{disk_needed_bytes, machine_class, plan_models};

fn e(id: &str, step: Step, class: MachineClass, languages: &[&str]) -> CatalogueEntry {
    CatalogueEntry {
        id: id.into(),
        step,
        machine_class: class,
        languages: languages.iter().map(|s| s.to_string()).collect(),
        default_enabled: true,
        ..Default::default()
    }
}

fn catalogue() -> Vec<CatalogueEntry> {
    let mut voice_f = e("voice-fr-f", Step::Speak, MachineClass::Minimum, &["fr"]);
    voice_f.gender = Some(Gender::Female);
    voice_f.requires = vec!["espeak".into()];
    let mut voice_en_m = e("voice-en-m", Step::Speak, MachineClass::Minimum, &["en"]);
    voice_en_m.gender = Some(Gender::Male);
    let mut nc = e(
        "nllb",
        Step::Translate,
        MachineClass::Minimum,
        &["fr", "en", "km"],
    );
    nc.default_enabled = false;
    let mut mac_only = e("runtime-mac", Step::Runtime, MachineClass::Minimum, &[]);
    mac_only.platforms = Some(Platforms {
        macos: vec!["arm64".into()],
        windows: vec![],
    });
    let mut win_only = e("runtime-win", Step::Runtime, MachineClass::Minimum, &[]);
    win_only.platforms = Some(Platforms {
        windows: vec!["x64".into()],
        macos: vec![],
    });
    vec![
        mac_only,
        win_only,
        e(
            "whisper-small",
            Step::Transcribe,
            MachineClass::Minimum,
            &["fr", "en"],
        ),
        e(
            "whisper-large",
            Step::Transcribe,
            MachineClass::FullLocal,
            &["fr", "en"],
        ),
        e(
            "m2m-small",
            Step::Translate,
            MachineClass::Minimum,
            &["fr", "en"],
        ),
        e("m2m-big", Step::Translate, MachineClass::Mid, &["fr", "en"]),
        nc,
        e("demucs", Step::Separate, MachineClass::Minimum, &[]),
        e("diar", Step::Speaker, MachineClass::Minimum, &[]),
        e("espeak", Step::Speak, MachineClass::Minimum, &[]),
        voice_f,
        voice_en_m,
    ]
}

#[test]
fn machine_class_follows_cores_ram_and_accelerator() {
    let m = |cores, ram, acc| MachineInput {
        cpu_cores: Some(cores),
        ram_gb: Some(ram),
        accelerator: Some(acc),
        ..Default::default()
    };
    assert_eq!(
        machine_class(&m(4, 8.0, Accelerator::None)),
        MachineClass::Minimum
    );
    assert_eq!(
        machine_class(&m(8, 16.0, Accelerator::None)),
        MachineClass::Mid
    );
    assert_eq!(
        machine_class(&m(8, 16.0, Accelerator::AppleSilicon)),
        MachineClass::FullLocal
    );
    assert_eq!(
        machine_class(&MachineInput::default()),
        MachineClass::Minimum,
        "unknown is the lowest"
    );
}

#[test]
fn the_best_model_the_machine_can_run_is_chosen() {
    let cat = catalogue();
    let small = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
    );
    assert_eq!(
        (small.transcribe.as_deref(), small.translate.as_deref()),
        (Some("whisper-small"), Some("m2m-small"))
    );
    let mid = plan_models(&cat, MachineClass::Mid, "macos", "arm64", "fr", "en", false);
    assert_eq!(
        (mid.transcribe.as_deref(), mid.translate.as_deref()),
        (Some("whisper-small"), Some("m2m-big"))
    );
    let full = plan_models(
        &cat,
        MachineClass::FullLocal,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
    );
    assert_eq!(full.transcribe.as_deref(), Some("whisper-large"));
}

#[test]
fn platform_and_opt_in_rules_apply() {
    let cat = catalogue();
    assert_eq!(
        plan_models(
            &cat,
            MachineClass::Minimum,
            "macos",
            "arm64",
            "fr",
            "en",
            false
        )
        .runtime
        .as_deref(),
        Some("runtime-mac")
    );
    assert_eq!(
        plan_models(
            &cat,
            MachineClass::Minimum,
            "windows",
            "x64",
            "fr",
            "en",
            false
        )
        .runtime
        .as_deref(),
        Some("runtime-win")
    );
    assert_eq!(
        plan_models(
            &cat,
            MachineClass::Minimum,
            "windows",
            "arm64",
            "fr",
            "en",
            false
        )
        .runtime,
        None
    );
    // The non-commercial model covers Khmer but is off by default, so the pair is unsupported.
    let km = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "km",
        false,
    );
    assert_eq!(km.translate, None);
    assert!(km.unsupported.iter().any(|(s, _)| *s == Step::Translate));
}

#[test]
fn voices_follow_target_language_and_gender_with_a_generic_fallback() {
    let cat = catalogue();
    let fr = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "en",
        "fr",
        false,
    );
    assert_eq!(
        fr.voices.get(&Gender::Female).map(|v| v.model.as_str()),
        Some("voice-fr-f")
    );
    assert_eq!(
        fr.voices.get(&Gender::Male).map(|v| v.model.as_str()),
        Some("voice-fr-f"),
        "one voice serves both"
    );
    let en = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
    );
    assert_eq!(
        en.voices.get(&Gender::Male).map(|v| v.model.as_str()),
        Some("voice-en-m")
    );
    let none = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "de",
        false,
    );
    assert!(
        none.voices.is_empty()
            && none
                .unsupported
                .iter()
                .any(|(s, l)| *s == Step::Speak && l == "de")
    );
}

#[test]
fn fast_quality_skips_separation_and_same_language_needs_no_translation() {
    let cat = catalogue();
    assert_eq!(
        plan_models(
            &cat,
            MachineClass::Minimum,
            "macos",
            "arm64",
            "fr",
            "en",
            true
        )
        .separate,
        None
    );
    assert_eq!(
        plan_models(
            &cat,
            MachineClass::Minimum,
            "macos",
            "arm64",
            "fr",
            "en",
            false
        )
        .separate
        .as_deref(),
        Some("demucs")
    );
    let same = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "fr",
        false,
    );
    assert!(!same.unsupported.iter().any(|(s, _)| *s == Step::Translate));
}

#[test]
fn required_ids_include_requirements_once() {
    let cat = catalogue();
    let plan = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "en",
        "fr",
        false,
    );
    let ids = plan.required_ids(&cat);
    assert_eq!(ids.iter().filter(|i| *i == "espeak").count(), 1);
    assert!(ids.contains(&"voice-fr-f".to_string()) && ids.contains(&"whisper-small".to_string()));
}

#[test]
fn disk_estimate_grows_with_length() {
    let two_min = disk_needed_bytes(120_000);
    assert!(two_min > 60_000_000 && two_min < 100_000_000, "{two_min}");
    assert!(disk_needed_bytes(3 * 3600 * 1000) > 80 * two_min);
}

#[test]
fn auto_detect_needs_only_the_target_language_for_translation() {
    let cat = catalogue();
    let plan = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "auto",
        "en",
        false,
    );
    assert_eq!(plan.translate.as_deref(), Some("m2m-small"));
    assert!(plan.unsupported.is_empty(), "{:?}", plan.unsupported);
    // A target nobody translates into is still unsupported.
    let none = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "auto",
        "km",
        false,
    );
    assert!(none.unsupported.iter().any(|(s, _)| *s == Step::Translate));
}

fn kokoro() -> CatalogueEntry {
    let voice = |id: &str, sid: u32, gender: Gender, lang: &str, lexicon: Option<&str>| VoiceSpec {
        id: id.into(),
        name: id.into(),
        speaker_id: sid,
        gender,
        languages: vec![lang.split('-').next().unwrap().into()],
        lang: lang.into(),
        lexicon: lexicon.map(String::from),
        quality: Quality::Good,
    };
    CatalogueEntry {
        id: "kokoro".into(),
        step: Step::Speak,
        machine_class: MachineClass::Minimum,
        default_enabled: true,
        voices: vec![
            voice("ff_siwis", 30, Gender::Female, "fr", None),
            voice(
                "af_heart",
                3,
                Gender::Female,
                "en-us",
                Some("lexicon-us-en.txt"),
            ),
            voice(
                "am_michael",
                16,
                Gender::Male,
                "en-us",
                Some("lexicon-us-en.txt"),
            ),
        ],
        requires: vec!["espeak-ng-data".into()],
        ..Default::default()
    }
}

#[test]
fn a_better_voice_wins_and_carries_its_speaker_id_and_language() {
    let mut cat = catalogue(); // has Piper voices for fr (female) and en (male)
    cat.push(kokoro());
    let fr = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "en",
        "fr",
        false,
    );
    let v = &fr.voices[&Gender::Female];
    assert_eq!(
        (v.model.as_str(), v.speaker_id, v.lang.as_str()),
        ("kokoro", 30, "fr"),
        "Kokoro beats Piper"
    );
    assert_eq!(
        fr.voices[&Gender::Male].model,
        "kokoro",
        "no male French voice: the female one serves both"
    );

    let en = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "fr",
        "en",
        false,
    );
    assert_eq!(en.voices[&Gender::Male].speaker_id, 16);
    assert_eq!(en.voices[&Gender::Female].speaker_id, 3);
    assert_eq!(
        en.voices[&Gender::Female].lexicon.as_deref(),
        Some("lexicon-us-en.txt")
    );
    assert!(en.required_ids(&cat).contains(&"kokoro".to_string()));
}

#[test]
fn piper_still_covers_languages_kokoro_does_not() {
    let mut cat = catalogue();
    let mut piper_de = e("piper-de", Step::Speak, MachineClass::Minimum, &["de"]);
    piper_de.gender = Some(Gender::Male);
    cat.push(piper_de);
    cat.push(kokoro());
    let de = plan_models(
        &cat,
        MachineClass::Minimum,
        "macos",
        "arm64",
        "en",
        "de",
        false,
    );
    assert_eq!(de.voices[&Gender::Male].model, "piper-de");
    assert_eq!(
        de.voices[&Gender::Female].model,
        "piper-de",
        "the male voice also serves female speakers"
    );
}

#[test]
fn an_opted_in_model_beats_a_default_one_of_the_same_step() {
    use flickfluent_core::pipeline::plan::plan_models_full;
    use flickfluent_core::project::dub::FallbackChoices;

    let small = e(
        "m2m-small",
        Step::Translate,
        MachineClass::Minimum,
        &["fr", "en", "km"],
    );
    // A bigger default model would win on class alone.
    let big = e(
        "m2m-big",
        Step::Translate,
        MachineClass::Mid,
        &["fr", "en", "km"],
    );
    let mut nllb = e(
        "nllb",
        Step::Translate,
        MachineClass::Minimum,
        &["fr", "en", "km"],
    );
    nllb.default_enabled = false;
    let cat = vec![small, big, nllb];
    let plan = |opt_in: &[&str]| {
        let choices = FallbackChoices {
            opt_in: opt_in.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        plan_models_full(
            &cat,
            MachineClass::FullLocal,
            "macos",
            "arm64",
            "fr",
            "km",
            false,
            &[],
            &choices,
        )
        .translate
    };
    assert_eq!(plan(&[]).as_deref(), Some("m2m-big"));
    assert_eq!(plan(&["nllb"]).as_deref(), Some("nllb"));
}

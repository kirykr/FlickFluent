//! The language list and pair check are built from catalogue data only (SC-005, SC-006).

use flickfluent_core::languages::{check_pair, list, FallbackKind, StepKind};
use flickfluent_core::models::{
    CatalogueEntry, Gender, LicenceClass, MachineClass, Quality, Step, VoiceSpec,
};

fn entry(id: &str, step: Step, languages: &[&str]) -> CatalogueEntry {
    CatalogueEntry {
        id: id.into(),
        name: id.into(),
        step,
        machine_class: MachineClass::Minimum,
        languages: languages.iter().map(|s| s.to_string()).collect(),
        default_enabled: true,
        ..Default::default()
    }
}

fn catalogue() -> Vec<CatalogueEntry> {
    let mut piper = entry("piper-de", Step::Speak, &["de"]);
    piper.gender = Some(Gender::Male);
    let mut kokoro = entry("kokoro", Step::Speak, &["en", "fr"]);
    kokoro.voices = vec![
        VoiceSpec {
            id: "k-en".into(),
            name: "k".into(),
            speaker_id: 3,
            gender: Gender::Female,
            languages: vec!["en".into()],
            lang: "en-us".into(),
            lexicon: None,
            quality: Quality::Good,
        },
        VoiceSpec {
            id: "k-fr".into(),
            name: "k".into(),
            speaker_id: 30,
            gender: Gender::Female,
            languages: vec!["fr".into()],
            lang: "fr".into(),
            lexicon: None,
            quality: Quality::Good,
        },
    ];
    let mut nllb = entry("nllb", Step::Translate, &["en", "km", "fr"]);
    nllb.licence_class = LicenceClass::NonCommercial;
    nllb.default_enabled = false;
    nllb.licence_notice_key = Some("licence.nllb".into());
    vec![
        entry("whisper", Step::Transcribe, &["en", "fr", "de", "km"]),
        entry("m2m", Step::Translate, &["en", "fr", "de"]),
        nllb,
        piper,
        kokoro,
    ]
}

fn find<'a>(
    l: &'a [flickfluent_core::languages::Language],
    code: &str,
) -> &'a flickfluent_core::languages::Language {
    l.iter()
        .find(|x| x.code == code)
        .unwrap_or_else(|| panic!("{code} missing"))
}

#[test]
fn the_list_is_the_union_of_catalogue_languages_per_step() {
    let langs = list(&catalogue(), &[]);
    let codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["de", "en", "fr", "km"],
        "sorted, one row per language"
    );

    let en = find(&langs, "en");
    assert!(en.transcribe && en.translate_from && en.translate_to);
    assert_eq!((en.speak.voices, en.speak.permissive_voices), (1, 1));
    let de = find(&langs, "de");
    assert_eq!(de.speak.voices, 1);
    let km = find(&langs, "km");
    assert!(
        km.transcribe && !km.translate_from && !km.translate_to,
        "the non-commercial model is off by default"
    );
    assert_eq!(km.speak.voices, 0);
}

#[test]
fn adding_a_catalogue_entry_adds_the_language_with_no_other_change() {
    let mut cat = catalogue();
    assert!(!list(&cat, &[]).iter().any(|l| l.code == "sw"));
    cat.push(entry("whisper-sw", Step::Transcribe, &["sw"]));
    let langs = list(&cat, &[]);
    assert!(find(&langs, "sw").transcribe);
}

#[test]
fn non_commercial_entries_count_only_when_the_user_opted_in() {
    let off = list(&catalogue(), &[]);
    assert!(!find(&off, "km").translate_to);
    let on = list(&catalogue(), &["nllb".to_string()]);
    assert!(find(&on, "km").translate_to && find(&on, "km").translate_from);
}

#[test]
fn a_fully_supported_pair_has_no_fallbacks() {
    let check = check_pair(&catalogue(), "fr", "en", &[]);
    assert!(check.ok);
    assert!(check.fallbacks.is_empty());
    assert!(check.steps.iter().all(|s| s.supported));
}

#[test]
fn a_missing_voice_names_the_step_and_offers_subtitles_or_a_generic_voice() {
    // Khmer speech can be written down but nothing can translate or speak it by default.
    let check = check_pair(&catalogue(), "en", "km", &[]);
    assert!(!check.ok);
    let speak = check
        .steps
        .iter()
        .find(|s| s.step == StepKind::Speak)
        .unwrap();
    assert!(!speak.supported);
    let kinds: Vec<FallbackKind> = check.fallbacks.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&FallbackKind::SubtitlesOnly));
    assert!(kinds.contains(&FallbackKind::GenericVoice));
    assert!(
        kinds.contains(&FallbackKind::OptInNonCommercial),
        "the NLLB model could translate it"
    );
    let optin = check
        .fallbacks
        .iter()
        .find(|f| f.kind == FallbackKind::OptInNonCommercial)
        .unwrap();
    assert_eq!(optin.model_ids, vec!["nllb".to_string()]);
    assert_eq!(optin.notice_keys, vec!["licence.nllb".to_string()]);
}

#[test]
fn a_weak_pair_can_pivot_through_english() {
    // de to fr: only English connects them in this catalogue (m2m covers de, en and fr, so make
    // a model that does not).
    let mut cat = catalogue();
    cat.retain(|e| e.id != "m2m");
    cat.push(entry("de-en", Step::Translate, &["de", "en"]));
    cat.push(entry("en-fr", Step::Translate, &["en", "fr"]));
    // Each model covers both of its languages, so a direct de to fr is not possible with one model.
    let check = check_pair(&cat, "de", "fr", &[]);
    let t = check
        .steps
        .iter()
        .find(|s| s.step == StepKind::Translate)
        .unwrap();
    assert!(!t.supported);
    assert!(check
        .fallbacks
        .iter()
        .any(|f| f.kind == FallbackKind::PivotViaEnglish));
}

#[test]
fn auto_detect_and_same_language_need_no_translation_step() {
    let cat = catalogue();
    let auto = check_pair(&cat, "auto", "fr", &[]);
    assert!(auto.ok, "{:?}", auto.steps);
    let same = check_pair(&cat, "fr", "fr", &[]);
    let t = same
        .steps
        .iter()
        .find(|s| s.step == StepKind::Translate)
        .unwrap();
    assert!(t.supported, "nothing to translate");
}

#[test]
fn quality_comes_from_catalogue_data() {
    let mut cat = catalogue();
    cat[1]
        .quality_by_language
        .insert("de".into(), Quality::Weak);
    let check = check_pair(&cat, "de", "en", &[]);
    let t = check
        .steps
        .iter()
        .find(|s| s.step == StepKind::Translate)
        .unwrap();
    assert_eq!(t.quality, Some(Quality::Weak));
}

#[test]
fn the_shipped_catalogue_gives_a_real_list_and_honest_pair_checks() {
    use flickfluent_core::models::bundled_catalogue;
    let cat = bundled_catalogue();
    let langs = list(&cat, &[]);
    assert!(langs.len() >= 90, "{} languages", langs.len());
    let en = find(&langs, "en");
    assert!(en.transcribe && en.translate_to && en.speak.voices >= 2);
    let fr = find(&langs, "fr");
    assert!(fr.speak.voices >= 2, "Kokoro and Piper both speak French");

    assert!(check_pair(&cat, "auto", "fr", &[]).ok);
    assert!(check_pair(&cat, "fr", "en", &[]).ok);
    // Speech recognition and translation cover Khmer, but no voice does.
    let km = check_pair(&cat, "en", "km", &[]);
    assert!(!km.ok);
    assert!(km
        .steps
        .iter()
        .any(|s| s.step == StepKind::Speak && !s.supported));
    assert!(km
        .fallbacks
        .iter()
        .any(|f| f.kind == FallbackKind::SubtitlesOnly));
}

use flickfluent_core::error::{HOTKEY_CONFLICT, INVALID_SETTING};
use flickfluent_core::hardware::Mode;
use flickfluent_core::project::ProjectSession;
use flickfluent_core::settings::{self, hotkeys, Settings, Theme};
use serde_json::json;

#[test]
fn defaults_are_private_and_follow_the_system_theme() {
    let s = Settings::default();
    assert!(!s.analytics_enabled, "analytics must be off by default");
    assert_eq!(s.theme, Theme::System);
    assert_eq!(s.general.ui_language, "en");
    assert!(!s.hotkeys.is_empty());
    let combos: std::collections::HashSet<_> = s.hotkeys.values().collect();
    assert_eq!(
        combos.len(),
        s.hotkeys.len(),
        "default hotkeys must not clash"
    );
}

#[test]
fn save_and_load_round_trip_without_leaving_a_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    let mut s = Settings::default();
    s.apply_patch(&json!({ "theme": "dark", "analyticsEnabled": true }))
        .unwrap();
    s.last_compat_mode = Some(Mode::Hybrid);
    settings::save(&file, &s).unwrap();

    let loaded = settings::load(&file);
    assert!(loaded.notice.is_none());
    assert_eq!(loaded.settings, s);
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["settings.json"]);
}

#[test]
fn a_missing_file_gives_defaults_without_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = settings::load(&dir.path().join("settings.json"));
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.notice.is_none());
}

#[test]
fn a_corrupted_file_gives_defaults_a_notice_and_keeps_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    std::fs::write(&file, b"{ not json").unwrap();

    let loaded = settings::load(&file);
    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(loaded.notice, Some("settings.corrupted"));
    assert_eq!(
        std::fs::read(dir.path().join("settings.json.corrupt")).unwrap(),
        b"{ not json"
    );

    // The app can carry on and save normally afterwards.
    settings::save(&file, &loaded.settings).unwrap();
    assert!(settings::load(&file).notice.is_none());
}

#[test]
fn an_older_partial_file_is_filled_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    std::fs::write(&file, br#"{ "theme": "dark" }"#).unwrap();
    let s = settings::load(&file).settings;
    assert_eq!(s.theme, Theme::Dark);
    assert!(!s.analytics_enabled);
    assert_eq!(s.hotkeys, Settings::default().hotkeys);
}

#[test]
fn a_hotkey_already_in_use_is_reported_with_its_owner_and_nothing_changes() {
    let mut s = Settings::default();
    let before = s.clone();
    let err = s
        .apply_patch(&json!({ "hotkeys": { "project.save": "Mod+O" } }))
        .unwrap_err();
    assert_eq!(err.code, HOTKEY_CONFLICT);
    let d = err.details.unwrap();
    assert_eq!(d["action"], "project.open");
    assert_eq!(d["combo"], "Mod+O");
    assert_eq!(s, before);
}

#[test]
fn a_conflict_can_be_resolved_by_unassigning_the_other_action() {
    let mut s = Settings::default();
    s.apply_patch(&json!({ "hotkeys": { "project.save": "Mod+O", "project.open": null } }))
        .unwrap();
    assert_eq!(s.hotkeys["project.save"], "Mod+O");
    assert!(!s.hotkeys.contains_key("project.open"));
}

#[test]
fn hotkeys_are_normalised_and_compared_case_insensitively() {
    assert_eq!(
        hotkeys::normalize("mod+shift+s").as_deref(),
        Some("Mod+Shift+S")
    );
    assert_eq!(
        hotkeys::normalize("Shift+ctrl+f5").as_deref(),
        Some("Ctrl+Shift+F5")
    );
    assert_eq!(
        hotkeys::normalize("alt+arrowup").as_deref(),
        Some("Alt+ArrowUp")
    );
    assert_eq!(hotkeys::normalize("S"), None, "needs a modifier");
    assert_eq!(hotkeys::normalize("mod+"), None);
    assert_eq!(hotkeys::normalize("mod+banana"), None);

    let mut s = Settings::default();
    let err = s
        .apply_patch(&json!({ "hotkeys": { "project.save": "mod+o" } }))
        .unwrap_err();
    assert_eq!(err.code, HOTKEY_CONFLICT);
}

#[test]
fn invalid_patches_are_refused_and_change_nothing() {
    for patch in [
        json!({ "theme": "purple" }),
        json!({ "analyticsEnabled": "yes" }),
        json!({ "nonsense": 1 }),
        json!({ "hotkeys": { "project.fly": "Mod+F" } }),
        json!({ "hotkeys": { "project.save": "banana" } }),
        json!("not an object"),
    ] {
        let mut s = Settings::default();
        let err = s.apply_patch(&patch).unwrap_err();
        assert_eq!(err.code, INVALID_SETTING, "{patch}");
        assert_eq!(s, Settings::default(), "{patch}");
    }
}

#[test]
fn partial_patches_leave_other_settings_alone() {
    let mut s = Settings::default();
    s.apply_patch(&json!({ "general": { "defaultSaveFolder": "/films" } }))
        .unwrap();
    s.apply_patch(&json!({ "theme": "light" })).unwrap();
    assert_eq!(s.general.default_save_folder.as_deref(), Some("/films"));
    assert_eq!(s.general.ui_language, "en");
    assert_eq!(s.theme, Theme::Light);
}

#[test]
fn reset_restores_defaults_and_does_not_touch_projects() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("keep.flick");
    ProjectSession::create("Keep", &project).unwrap().close();
    let before = std::fs::read(&project).unwrap();

    let file = dir.path().join("settings.json");
    let mut s = Settings::default();
    s.apply_patch(&json!({ "theme": "dark", "analyticsEnabled": true }))
        .unwrap();
    settings::save(&file, &s).unwrap();

    let reset = Settings::default();
    settings::save(&file, &reset).unwrap();
    assert_eq!(settings::load(&file).settings, Settings::default());
    assert_eq!(std::fs::read(&project).unwrap(), before);
}

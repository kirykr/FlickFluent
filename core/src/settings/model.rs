use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::hotkeys;
use crate::cloud::choice::Engines;
use crate::error::{AppError, HOTKEY_CONFLICT, INVALID_SETTING};
use crate::hardware::Mode;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub ui_language: String,
    pub default_save_folder: Option<String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            ui_language: "en".to_string(),
            default_save_folder: None,
        }
    }
}

/// Every missing key falls back to its default, so an older file still loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub settings_version: u32,
    pub theme: Theme,
    pub general: General,
    /// Action name to combo, for example `project.save` to `Mod+S`.
    pub hotkeys: BTreeMap<String, String>,
    /// Off by default. Turning it on is the user's choice.
    pub analytics_enabled: bool,
    pub models_folder: Option<String>,
    pub last_compat_mode: Option<Mode>,
    /// Local or a cloud service for each heavy step. Local by default.
    pub engines: Engines,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: SETTINGS_VERSION,
            theme: Theme::System,
            general: General::default(),
            hotkeys: hotkeys::DEFAULTS
                .iter()
                .map(|(a, c)| ((*a).to_string(), (*c).to_string()))
                .collect(),
            analytics_enabled: false,
            models_folder: None,
            last_compat_mode: None,
            engines: Engines::default(),
        }
    }
}

fn invalid(what: &str) -> AppError {
    AppError::new(INVALID_SETTING, "settings.error.invalid")
        .with_details(serde_json::json!({ "what": what }))
}

const PATCHABLE: [&str; 8] = [
    "settingsVersion",
    "theme",
    "general",
    "hotkeys",
    "analyticsEnabled",
    "modelsFolder",
    "lastCompatMode",
    "engines",
];

impl Settings {
    /// Apply a partial update. All or nothing: if anything is invalid or a hotkey clashes, the
    /// settings stay exactly as they were.
    ///
    /// `hotkeys` is merged per action; a `null` value unassigns that action, which is how the
    /// UI resolves a clash ("use it here, remove it there").
    pub fn apply_patch(&mut self, patch: &Value) -> Result<(), AppError> {
        let Value::Object(patch) = patch else {
            return Err(invalid("patch"));
        };
        if let Some(unknown) = patch.keys().find(|k| !PATCHABLE.contains(&k.as_str())) {
            return Err(invalid(unknown));
        }

        let mut next = serde_json::to_value(&*self).map_err(|_| invalid("current"))?;
        let obj = next.as_object_mut().ok_or_else(|| invalid("current"))?;
        let mut changed_actions: Vec<String> = Vec::new();

        for (key, value) in patch {
            match key.as_str() {
                "settingsVersion" => {}
                "general" => {
                    let Value::Object(incoming) = value else {
                        return Err(invalid("general"));
                    };
                    let general = obj
                        .entry("general")
                        .or_insert_with(|| Value::Object(Map::new()));
                    let general = general.as_object_mut().ok_or_else(|| invalid("general"))?;
                    for (k, v) in incoming {
                        general.insert(k.clone(), v.clone());
                    }
                }
                "hotkeys" => {
                    let Value::Object(incoming) = value else {
                        return Err(invalid("hotkeys"));
                    };
                    let mut map = self.hotkeys.clone();
                    for (action, combo) in incoming {
                        if !hotkeys::is_known_action(action) {
                            return Err(invalid(action));
                        }
                        changed_actions.push(action.clone());
                        match combo {
                            Value::Null => {
                                map.remove(action);
                            }
                            Value::String(text) => {
                                let normal =
                                    hotkeys::normalize(text).ok_or_else(|| invalid(action))?;
                                map.insert(action.clone(), normal);
                            }
                            _ => return Err(invalid(action)),
                        }
                    }
                    obj.insert(
                        "hotkeys".into(),
                        serde_json::to_value(map).map_err(|_| invalid(key))?,
                    );
                }
                _ => {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }

        let candidate: Settings = serde_json::from_value(next).map_err(|_| invalid("value"))?;
        candidate.check_hotkeys(&changed_actions)?;
        *self = candidate;
        Ok(())
    }

    /// Two actions may not share a combo. The error names the action that already owns it.
    fn check_hotkeys(&self, changed: &[String]) -> Result<(), AppError> {
        let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
        for (action, combo) in &self.hotkeys {
            let key = combo.as_str();
            if let Some(first) = owners.insert(key, action) {
                // Report the owner that was not part of this change.
                let owner = if changed.iter().any(|c| c == first) {
                    action
                } else {
                    first
                };
                return Err(
                    AppError::new(HOTKEY_CONFLICT, "settings.error.hotkeyConflict")
                        .with_details(serde_json::json!({ "action": owner, "combo": combo })),
                );
            }
        }
        Ok(())
    }
}

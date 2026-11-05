use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use flickfluent_core::error::AppError;
use flickfluent_core::hardware::Mode;
use flickfluent_core::settings::{self, Settings};
use serde::Serialize;
use tauri::{Emitter, Manager};

struct Inner {
    settings: Settings,
    /// Shown once, then cleared.
    notice: Option<&'static str>,
    file: Option<PathBuf>,
}

pub struct SettingsState {
    inner: Mutex<Inner>,
}

#[derive(Serialize)]
pub struct SettingsView {
    settings: Settings,
    notice: Option<&'static str>,
}

impl SettingsState {
    pub fn load(app: &tauri::AppHandle) -> Self {
        let file = app
            .path()
            .app_config_dir()
            .ok()
            .map(|d| d.join("settings.json"));
        let loaded = match &file {
            Some(f) => settings::load(f),
            None => settings::Loaded {
                settings: Settings::default(),
                notice: None,
            },
        };
        Self {
            inner: Mutex::new(Inner {
                settings: loaded.settings,
                notice: loaded.notice,
                file,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Folder chosen in settings for downloaded models, if any.
    pub fn models_folder(&self) -> Option<String> {
        self.lock().settings.models_folder.clone()
    }

    /// The engine choice per step (local by default).
    pub fn engines(&self) -> flickfluent_core::cloud::choice::Engines {
        self.lock().settings.engines.clone()
    }

    /// Save a new engine for one step. The caller has checked that the choice is allowed.
    pub fn set_engine(
        &self,
        step: flickfluent_core::cloud::services::Step,
        engine: &str,
    ) -> Result<flickfluent_core::cloud::choice::Engines, AppError> {
        let mut inner = self.lock();
        let mut next = inner.settings.clone();
        let mut engines = next.engines.clone();
        engines.set(step, engine);
        next.apply_patch(&serde_json::json!({ "engines": engines }))?;
        let previous = std::mem::replace(&mut inner.settings, next);
        if let Err(e) = persist(&inner) {
            inner.settings = previous;
            return Err(e);
        }
        Ok(inner.settings.engines.clone())
    }

    /// Remember the last hardware check result. Failing to save is not worth an error here.
    pub fn record_mode(&self, mode: Mode) {
        let mut inner = self.lock();
        if inner.settings.last_compat_mode != Some(mode) {
            inner.settings.last_compat_mode = Some(mode);
            if let Some(file) = &inner.file {
                let _ = settings::save(file, &inner.settings);
            }
        }
    }
}

fn view(inner: &mut Inner) -> SettingsView {
    SettingsView {
        settings: inner.settings.clone(),
        notice: inner.notice.take(),
    }
}

fn persist(inner: &Inner) -> Result<(), AppError> {
    match &inner.file {
        Some(file) => settings::save(file, &inner.settings),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn settings_get(state: tauri::State<SettingsState>) -> SettingsView {
    view(&mut state.lock())
}

#[tauri::command]
pub fn settings_set(
    app: tauri::AppHandle,
    state: tauri::State<SettingsState>,
    patch: serde_json::Value,
) -> Result<SettingsView, AppError> {
    let mut inner = state.lock();
    // Work on a copy so a failed save leaves the in-memory settings as they were.
    let mut next = inner.settings.clone();
    next.apply_patch(&patch)?;
    let previous = std::mem::replace(&mut inner.settings, next);
    if let Err(e) = persist(&inner) {
        inner.settings = previous;
        return Err(e);
    }
    let out = view(&mut inner);
    let _ = app.emit("settings:changed", &out.settings);
    Ok(out)
}

/// Back to defaults. Projects are not touched; only the settings file changes.
#[tauri::command]
pub fn settings_reset(
    app: tauri::AppHandle,
    state: tauri::State<SettingsState>,
) -> Result<SettingsView, AppError> {
    let mut inner = state.lock();
    let previous = std::mem::take(&mut inner.settings);
    if let Err(e) = persist(&inner) {
        inner.settings = previous;
        return Err(e);
    }
    let out = view(&mut inner);
    let _ = app.emit("settings:changed", &out.settings);
    Ok(out)
}

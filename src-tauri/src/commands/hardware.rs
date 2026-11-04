use std::path::PathBuf;

use flickfluent_core::hardware::{evaluate, probe, MachineCapabilityReport, SpecDefinition};
use tauri::Manager;

fn cache_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("last_compat.json"))
}

/// Run the hardware check now. The probe can take a moment (GPU listing), so it runs off the
/// main thread. The result is cached so the app can show the last mode without re-probing.
#[tauri::command]
pub async fn run_compat_check(
    app: tauri::AppHandle,
    settings: tauri::State<'_, super::settings::SettingsState>,
) -> Result<MachineCapabilityReport, String> {
    let report =
        tauri::async_runtime::spawn_blocking(|| evaluate(&probe(), &SpecDefinition::bundled()))
            .await
            .map_err(|e| e.to_string())?;
    settings.record_mode(report.mode);
    if let Some(path) = cache_path(&app) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(&report) {
            let _ = std::fs::write(path, json);
        }
    }
    Ok(report)
}

#[tauri::command]
pub fn get_last_compat_report(app: tauri::AppHandle) -> Option<MachineCapabilityReport> {
    let text = std::fs::read_to_string(cache_path(&app)?).ok()?;
    serde_json::from_str(&text).ok()
}

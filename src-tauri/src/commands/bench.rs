//! Benchmark commands. The run is `flickfluent_core::bench`, the same code `flickfluent --bench`
//! uses; this only supplies the app's folders and sends `bench:progress`.

use std::time::{Duration, Instant};

use flickfluent_core::bench::{cli, load_records, records_dir, BenchRecord};
use flickfluent_core::error::AppError;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use super::dub::launcher;
use super::models::models_dir;
use super::settings::SettingsState;

pub const PROGRESS: &str = "bench:progress";
const PROGRESS_EVERY: Duration = Duration::from_secs(1);

fn data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))
}

/// Run a benchmark in the background. Progress arrives as `bench:progress`; the record is saved
/// and shows up in `bench_records`.
#[tauri::command]
pub fn bench_run(
    app: tauri::AppHandle,
    settings: tauri::State<SettingsState>,
    fixture_id: String,
    simulated: Option<bool>,
) -> Result<String, AppError> {
    let models = models_dir(&app, &settings)?;
    let out = records_dir(&data_dir(&app)?);
    let mut args: Vec<String> = vec![
        "--bench".into(),
        "--json".into(),
        "--fixture".into(),
        fixture_id,
        "--models".into(),
        models.to_string_lossy().into_owned(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
        "--sidecar".into(),
        launcher(&app).to_string_lossy().into_owned(),
    ];
    if simulated == Some(true) {
        // The same machine as the minimum spec: its cores and RAM.
        let spec = flickfluent_core::hardware::SpecDefinition::bundled();
        args.extend([
            "--simulate-cores".into(),
            spec.minimum.cpu_cores.to_string(),
            "--simulate-ram-gb".into(),
            spec.minimum.ram_gb.to_string(),
        ]);
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let mut last = Instant::now() - PROGRESS_EVERY;
        let sink = app.clone();
        let sink_id = id.clone();
        let result = cli::run_with(&args, &mut move |stage, progress| {
            if last.elapsed() >= PROGRESS_EVERY {
                last = Instant::now();
                let _ = sink.emit(
                    PROGRESS,
                    json!({ "runId": sink_id, "stage": stage, "progress": progress }),
                );
            }
        })
        .await;
        let _ = app.emit(
            "bench:done",
            json!({ "runId": id, "ok": result.exit_code == 0, "exitCode": result.exit_code,
                    "message": if result.exit_code == 0 { Value::Null } else { Value::String(result.stdout) } }),
        );
    });
    Ok(run_id)
}

#[tauri::command]
pub fn bench_records(app: tauri::AppHandle) -> Result<Vec<BenchRecord>, AppError> {
    Ok(load_records(&records_dir(&data_dir(&app)?)))
}

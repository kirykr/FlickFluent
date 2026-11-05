mod commands;
mod events;

use tauri::Manager;

use std::time::Duration;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::project::ProjectState::default())
        .manage(commands::models::ModelsState::default())
        .manage(commands::dub::DubState::default())
        .setup(|app| {
            app.manage(commands::settings::SettingsState::load(app.handle()));
            // Write recovery files for projects with unsaved changes (SC-005).
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(
                    flickfluent_core::project::AUTOSAVE_SECS,
                ));
                commands::project::autosave_all(&handle);
            });
            events::start_memory_sampler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hardware::run_compat_check,
            commands::hardware::get_last_compat_report,
            commands::project::project_suggest_file_name,
            commands::project::project_new,
            commands::project::project_open,
            commands::project::project_recover,
            commands::project::project_discard_recovery,
            commands::project::project_save,
            commands::project::project_set_notes,
            commands::project::project_close,
            commands::project::project_recent,
            commands::project::project_delete,
            commands::models::models_catalogue,
            commands::models::model_download_start,
            commands::models::model_download_pause,
            commands::models::model_download_resume,
            commands::models::model_download_cancel,
            commands::models::model_delete,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::settings_reset,
            commands::languages::languages_list,
            commands::languages::languages_check_pair,
            commands::dub::dub_probe_source,
            commands::dub::dub_configure,
            commands::dub::dub_preflight,
            commands::dub::dub_start,
            commands::dub::dub_cancel,
            commands::dub::dub_resume,
            commands::dub::dub_status,
            commands::dub::dub_export,
            commands::dub::dub_discard_run,
            commands::dub::bench_run,
            commands::dub::bench_records,
            commands::dub::dub_installed_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlickFluent");
}

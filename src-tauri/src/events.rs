use std::time::Duration;

use tauri::Emitter;

/// How often the memory badge is refreshed.
pub const MEMORY_SAMPLE_SECS: u64 = 2;

/// Sends `memory:sample` every two seconds for the life of the app.
pub fn start_memory_sampler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let _ = app.emit("memory:sample", flickfluent_core::memory::sample());
        std::thread::sleep(Duration::from_secs(MEMORY_SAMPLE_SECS));
    });
}

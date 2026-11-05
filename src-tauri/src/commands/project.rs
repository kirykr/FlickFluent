use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use flickfluent_core::error::{AppError, IN_USE, READ_ONLY};
use flickfluent_core::project::recent::{self, RecentEntry};
use flickfluent_core::project::{self, file_name_for, ProjectSession};
use serde::Serialize;
use tauri::{Emitter, Manager};

/// All projects open in this window, keyed by an opaque handle id.
#[derive(Default)]
pub struct ProjectState {
    sessions: Mutex<HashMap<String, ProjectSession>>,
}

impl ProjectState {
    /// Run `f` on one open project.
    pub fn with<R>(
        &self,
        handle: &str,
        f: impl FnOnce(&mut ProjectSession) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        let mut sessions = self.lock();
        let s = sessions.get_mut(handle).ok_or_else(not_found)?;
        f(s)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, ProjectSession>> {
        // A panic in another command must not lock the user out of their projects.
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHandle {
    pub id: String,
    name: String,
    path: String,
    read_only: bool,
    dirty: bool,
    /// Placeholder project content until later features add real data.
    notes: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    handle: ProjectHandle,
    recovery_available: bool,
}

pub fn handle_of(id: &str, s: &ProjectSession) -> ProjectHandle {
    ProjectHandle {
        id: id.to_string(),
        name: s.project.manifest.name.clone(),
        path: s.path.to_string_lossy().into_owned(),
        read_only: s.read_only,
        dirty: s.dirty,
        notes: s
            .project
            .data
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

fn not_found() -> AppError {
    AppError::new("NOT_FOUND", "error.notFound")
}

fn recent_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("recent.json"))
}

fn remember(app: &tauri::AppHandle, s: &ProjectSession) {
    let Some(file) = recent_file(app) else { return };
    let mut list = recent::load(&file);
    recent::push(
        &mut list,
        RecentEntry {
            path: s.path.to_string_lossy().into_owned(),
            name: s.project.manifest.name.clone(),
            opened_at: s.project.manifest.saved_at.clone(),
            available: true,
        },
    );
    let _ = recent::save(&file, &list);
}

fn register(
    app: &tauri::AppHandle,
    state: &ProjectState,
    session: ProjectSession,
) -> ProjectHandle {
    remember(app, &session);
    let id = uuid_like();
    let handle = handle_of(&id, &session);
    state.lock().insert(id, session);
    handle
}

fn uuid_like() -> String {
    // Handle ids only need to be unique within this run.
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("p{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[tauri::command]
pub fn project_suggest_file_name(name: String) -> String {
    file_name_for(&name)
}

#[tauri::command]
pub fn project_new(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    name: String,
    path: String,
) -> Result<ProjectHandle, AppError> {
    let session = ProjectSession::create(&name, Path::new(&path))?;
    Ok(register(&app, &state, session))
}

#[tauri::command]
pub fn project_open(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    path: String,
) -> Result<OpenResult, AppError> {
    // Already open in this window: switch to it instead of opening a read-only second copy.
    if let Some((id, s)) = state
        .lock()
        .iter()
        .find(|(_, s)| s.path == Path::new(&path))
    {
        return Ok(OpenResult {
            handle: handle_of(id, s),
            recovery_available: false,
        });
    }
    let outcome = ProjectSession::open(Path::new(&path))?;
    let recovery_available = outcome.recovery_available;
    Ok(OpenResult {
        handle: register(&app, &state, outcome.session),
        recovery_available,
    })
}

/// Restore from the recovery file or newest backup. Replaces any session already open for the
/// same file without deleting the recovery file it is about to read.
#[tauri::command]
pub fn project_recover(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    path: String,
) -> Result<ProjectHandle, AppError> {
    let p = PathBuf::from(&path);
    let stale: Vec<String> = state
        .lock()
        .iter()
        .filter(|(_, s)| s.path == p)
        .map(|(id, _)| id.clone())
        .collect();
    for id in stale {
        state.lock().remove(&id); // dropped, not closed: closing could delete the recovery file
    }
    let session = ProjectSession::open_recovered(&p)?;
    Ok(register(&app, &state, session))
}

/// The user chose not to restore: remove the recovery file.
#[tauri::command]
pub fn project_discard_recovery(path: String) {
    project::recover::clear_recovery(Path::new(&path));
}

#[tauri::command]
pub fn project_save(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    handle: String,
    path: Option<String>,
) -> Result<ProjectHandle, AppError> {
    let mut sessions = state.lock();
    let s = sessions.get_mut(&handle).ok_or_else(not_found)?;
    match path {
        Some(p) => s.save_as(Path::new(&p))?,
        None => s.save()?,
    }
    remember(&app, s);
    let out = handle_of(&handle, s);
    let _ = app.emit(
        "project:dirty",
        serde_json::json!({ "handle": handle, "dirty": false }),
    );
    Ok(out)
}

#[tauri::command]
pub fn project_set_notes(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    handle: String,
    notes: String,
) -> Result<ProjectHandle, AppError> {
    let mut sessions = state.lock();
    let s = sessions.get_mut(&handle).ok_or_else(not_found)?;
    if s.read_only {
        return Err(AppError::new(READ_ONLY, "error.readOnly"));
    }
    let mut data = s.project.data.clone();
    data["notes"] = serde_json::Value::String(notes);
    s.set_data(data);
    let _ = app.emit(
        "project:dirty",
        serde_json::json!({ "handle": handle, "dirty": true }),
    );
    Ok(handle_of(&handle, s))
}

#[tauri::command]
pub fn project_close(state: tauri::State<ProjectState>, handle: String) {
    if let Some(s) = state.lock().remove(&handle) {
        s.close();
    }
}

#[tauri::command]
pub fn project_recent(app: tauri::AppHandle) -> Vec<RecentEntry> {
    recent_file(&app)
        .map(|f| recent::with_availability(recent::load(&f)))
        .unwrap_or_default()
}

/// Delete a project file and everything next to it. The UI asks the user first.
#[tauri::command]
pub fn project_delete(
    app: tauri::AppHandle,
    state: tauri::State<ProjectState>,
    path: String,
) -> Result<(), AppError> {
    let p = PathBuf::from(&path);
    if state.lock().values().any(|s| s.path == p) {
        return Err(AppError::new(IN_USE, "error.inUse"));
    }
    let side = |suffix: &str| {
        let mut s = p.as_os_str().to_owned();
        s.push(suffix);
        PathBuf::from(s)
    };
    let mut doomed = vec![p.clone(), side(".recover"), side(".lock"), side(".tmp")];
    doomed.extend((1..=project::BACKUPS).map(|n| side(&format!(".bak.{n}"))));
    for f in doomed {
        match std::fs::remove_file(&f) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::io(e)),
        }
    }
    // The dub's working files go with the project, so a new project with the same name starts clean.
    flickfluent_core::pipeline::state::WorkFolder::remove(&p)?;
    if let Some(file) = recent_file(&app) {
        let mut list = recent::load(&file);
        recent::remove(&mut list, &path);
        let _ = recent::save(&file, &list);
    }
    Ok(())
}

/// Called every `AUTOSAVE_SECS` from a background thread.
pub fn autosave_all(app: &tauri::AppHandle) {
    let state = app.state::<ProjectState>();
    for s in state.lock().values_mut() {
        let _ = s.autosave_tick();
    }
}

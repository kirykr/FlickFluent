//! Editor commands (feature 004). They drive `flickfluent_core::editor`; every rule is in the
//! core. See `specs/004-transcript-editor-timeline/contracts/editor-commands.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use flickfluent_core::editor::fit::{fit, FitMode, FitReport};
use flickfluent_core::editor::ops::{EditOp, LinePatch};
use flickfluent_core::editor::resynth::{prepare, run_prepared, ResynthEvent, ResynthState};
use flickfluent_core::editor::runtime::EditorRuntime;
use flickfluent_core::editor::search::{find, Field, Match, SearchOptions};
use flickfluent_core::editor::session::{EditResult, EditSession, EditorState, Saver};
use flickfluent_core::error::{AppError, IN_USE};
use flickfluent_core::pipeline::run::CancelToken;
use flickfluent_core::pipeline::state::WorkFolder;
use flickfluent_core::project::dub::{RunState, SpeakerGender};
use flickfluent_core::subtitles::import::{ImportChoice, ImportPreview};
use flickfluent_core::subtitles::read::read_file;
use flickfluent_core::subtitles::templates::{self, CaptionTemplate};
use flickfluent_core::subtitles::write::{export_file, Format, Source};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

use super::dub::{build_config, dub_of, store, with_settings_engines, DubState};
use super::models::models_dir;
use super::project::ProjectState;
use super::settings::SettingsState;

/// One editing session per open project, by the project handle. The session is shared so a
/// re-synthesis can run while edits keep arriving (its lock is held only for short steps).
#[derive(Default)]
pub struct EditorSessions {
    map: Mutex<HashMap<String, Arc<Mutex<EditSession>>>>,
    /// The work now running for a project (re-synthesis, import or export): its id and its stop.
    active: Mutex<HashMap<String, (String, CancelToken)>>,
}

/// The one event channel of the editor. Payloads are in `contracts/editor-commands.md`.
pub const EVENT: &str = "editor:event";

impl EditorSessions {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<Mutex<EditSession>>>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn insert(&self, handle: &str, session: EditSession) -> Arc<Mutex<EditSession>> {
        let shared = Arc::new(Mutex::new(session));
        self.lock().insert(handle.to_string(), shared.clone());
        shared
    }

    pub fn get(&self, handle: &str) -> Result<Arc<Mutex<EditSession>>, AppError> {
        self.lock().get(handle).cloned().ok_or_else(|| {
            AppError::new(
                flickfluent_core::error::EDIT_NOT_FOUND,
                "editor.error.notFound",
            )
        })
    }

    /// Drop the session of a project that closed, or whose data a dub run is about to replace.
    pub fn close(&self, handle: &str) {
        self.lock().remove(handle);
        if let Some((_, cancel)) = self.running().remove(handle) {
            cancel.cancel();
        }
    }

    fn running(&self) -> MutexGuard<'_, HashMap<String, (String, CancelToken)>> {
        self.active.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Note that work started. Fails if the project already has some running.
    fn begin(&self, handle: &str, run_id: &str, cancel: &CancelToken) -> Result<(), AppError> {
        let mut active = self.running();
        if active.contains_key(handle) {
            return Err(AppError::new(IN_USE, "error.inUse"));
        }
        active.insert(handle.to_string(), (run_id.to_string(), cancel.clone()));
        Ok(())
    }

    fn end(&self, handle: &str, run_id: &str) {
        let mut active = self.running();
        if active.get(handle).is_some_and(|(id, _)| id == run_id) {
            active.remove(handle);
        }
    }
}

fn with_session<R>(
    sessions: &EditorSessions,
    handle: &str,
    f: impl FnOnce(&mut EditSession) -> Result<R, AppError>,
) -> Result<R, AppError> {
    let shared = sessions.get(handle)?;
    let mut session = shared.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut session)
}

/// Writes the session's data into the project and saves the file, so the file always matches.
fn saver(app: &tauri::AppHandle, handle: &str) -> Saver {
    let app = app.clone();
    let handle = handle.to_string();
    Box::new(move |dub| {
        let projects = app.state::<ProjectState>();
        store(&projects, &handle, dub)?;
        projects.with(&handle, |s| s.save())
    })
}

#[tauri::command]
pub fn editor_open(
    app: tauri::AppHandle,
    projects: tauri::State<ProjectState>,
    dubs: tauri::State<DubState>,
    sessions: tauri::State<EditorSessions>,
    settings: tauri::State<SettingsState>,
    cloud: tauri::State<super::cloud::CloudState>,
    handle: String,
) -> Result<EditorState, AppError> {
    if dubs.running_for(&handle) {
        return Err(AppError::new(IN_USE, "error.inUse"));
    }
    let models = models_dir(&app, &settings)?;
    let (mut dub, path, project_id) = dub_of(&projects, &handle)?;
    let work = WorkFolder::open_for(&path, &project_id);
    // The project may have been closed before it saved a finished run's lines: read them back.
    if dub.lines.is_empty() {
        if let (Some(w), Some(run)) = (
            &work,
            work.as_ref().and_then(|w| w.load_run().ok().flatten()),
        ) {
            if run.state == RunState::Completed {
                dub.lines = w.load_lines(&run);
            }
        }
    }
    // The engines come from the project's run, else from today's settings. Only the runtime sees
    // the settings; the project keeps the choice its run recorded.
    let engines_from = with_settings_engines(dub.clone(), &settings);
    let folder = WorkFolder::folder_for(&path);
    let cfg = build_config(&app, &models, &path, &project_id, &folder, cloud.runtime());
    let rt = EditorRuntime::for_project(cfg, &engines_from);
    let session = EditSession::open(rt, dub, saver(&app, &handle))?;
    let state = session.state();
    sessions.insert(&handle, session);
    Ok(state)
}

#[tauri::command]
pub fn editor_state(
    sessions: tauri::State<EditorSessions>,
    handle: String,
) -> Result<EditorState, AppError> {
    with_session(&sessions, &handle, |s| Ok(s.state()))
}

/// `{ id, startMs?, endMs?, translated?, speakerId?, speed?, volumeDb?, pitchSemitones? }`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInput {
    id: String,
    #[serde(flatten)]
    patch: LinePatch,
}

#[tauri::command]
pub fn editor_update_line(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    edit: UpdateInput,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::UpdateLine {
            id: edit.id,
            patch: edit.patch,
        })
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertInput {
    after_id: Option<String>,
    start_ms: u64,
    end_ms: u64,
    speaker_id: String,
    translated: String,
}

#[tauri::command]
pub fn editor_insert_line(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    edit: InsertInput,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::InsertLine {
            after_id: edit.after_id,
            start_ms: edit.start_ms,
            end_ms: edit.end_ms,
            speaker_id: edit.speaker_id,
            translated: edit.translated,
        })
    })
}

#[tauri::command]
pub fn editor_delete_lines(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    ids: Vec<String>,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| s.apply(EditOp::DeleteLines { ids }))
}

#[tauri::command]
pub fn editor_undo(
    sessions: tauri::State<EditorSessions>,
    handle: String,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| s.undo())
}

#[tauri::command]
pub fn editor_redo(
    sessions: tauri::State<EditorSessions>,
    handle: String,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| s.redo())
}

// --- search and replace ----------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SearchResult {
    matches: Vec<Match>,
}

#[tauri::command]
pub fn editor_search(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    query: String,
    field: Field,
    match_case: Option<bool>,
    whole_word: Option<bool>,
) -> Result<SearchResult, AppError> {
    with_session(&sessions, &handle, |s| {
        Ok(SearchResult {
            matches: find(
                &s.dub().lines,
                &SearchOptions {
                    query,
                    field,
                    match_case: match_case.unwrap_or(false),
                    whole_word: whole_word.unwrap_or(false),
                },
            ),
        })
    })
}

#[tauri::command]
pub fn editor_replace(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    matches: Vec<Match>,
    replacement: String,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::ReplaceMatches {
            matches,
            replacement,
        })
    })
}

// --- making speech again ---------------------------------------------------------------------

fn emit(app: &tauri::AppHandle, payload: Value) {
    let _ = app.emit(EVENT, payload);
}

/// Start re-synthesis of `ids` (absent: every changed line). Problems that stop it before it
/// starts come back here; progress and the end arrive as `editor:event`.
#[tauri::command]
pub fn editor_resynthesize(
    app: tauri::AppHandle,
    sessions: tauri::State<EditorSessions>,
    handle: String,
    ids: Option<Vec<String>>,
) -> Result<String, AppError> {
    let shared = sessions.get(&handle)?;
    let prepared = prepare(&shared, ids)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancel = CancelToken::default();
    sessions.begin(&handle, &run_id, &cancel)?;

    let task_app = app.clone();
    let (task_id, task_handle) = (run_id.clone(), handle.clone());
    tauri::async_runtime::spawn(async move {
        let sink_app = task_app.clone();
        let sink_id = task_id.clone();
        let out = run_prepared(&shared, prepared, cancel, &mut move |e| match e {
            ResynthEvent::Progress {
                done,
                total,
                eta_seconds,
            } => emit(
                &sink_app,
                json!({
                    "type": "progress", "runId": sink_id, "kind": "resynthesize",
                    "done": done, "total": total, "etaSeconds": eta_seconds,
                }),
            ),
            ResynthEvent::LineDone(line) => emit(
                &sink_app,
                json!({ "type": "lineDone", "runId": sink_id, "line": line }),
            ),
            ResynthEvent::Cloud(note) => emit(
                &sink_app,
                json!({
                    "type": "cloud", "runId": sink_id, "kind": note.kind, "step": note.step,
                    "serviceId": note.service_id, "keyLabel": note.key_label,
                    "reason": note.reason, "waitSeconds": note.wait_seconds,
                }),
            ),
        })
        .await;
        let state = match out.state {
            ResynthState::Completed => "completed",
            ResynthState::Stopped => "stopped",
            ResynthState::Failed => "failed",
        };
        task_app
            .state::<EditorSessions>()
            .end(&task_handle, &task_id);
        emit(
            &task_app,
            json!({
                "type": "finished", "runId": task_id, "state": state,
                "error": out.error, "remainingChangedIds": out.remaining_changed_ids,
            }),
        );
    });
    Ok(run_id)
}

/// Stop the work now running for this project. What was finished is kept.
#[tauri::command]
pub fn editor_stop(sessions: tauri::State<EditorSessions>, handle: String) {
    if let Some((_, cancel)) = sessions.running().get(&handle) {
        cancel.cancel();
    }
}

// --- fitting ---------------------------------------------------------------------------------

/// Smart Fit or End Sync. `ids` absent means every flagged line (Smart Fit) or every line
/// (End Sync). No engine call and no cloud characters: only the post chain and the mix.
#[tauri::command]
pub async fn editor_fit(
    app: tauri::AppHandle,
    sessions: tauri::State<'_, EditorSessions>,
    handle: String,
    mode: FitMode,
    ids: Option<Vec<String>>,
) -> Result<FitReport, AppError> {
    let shared = sessions.get(&handle)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancel = CancelToken::default();
    sessions.begin(&handle, &run_id, &cancel)?;
    let sink = app.clone();
    let id = run_id.clone();
    let out = fit(&shared, mode, ids, cancel, &mut move |e| {
        if let ResynthEvent::LineDone(line) = e {
            emit(
                &sink,
                json!({ "type": "lineDone", "runId": id, "line": line }),
            );
        }
    })
    .await;
    sessions.end(&handle, &run_id);
    out
}

// --- subtitles -------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCount {
    cue_count: usize,
}

/// Write the lines as SRT or WebVTT where the user chose. WebVTT carries the project's caption
/// template; SRT has no style.
#[tauri::command]
pub fn subtitles_export(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    source: Source,
    format: Format,
    path: String,
) -> Result<ExportCount, AppError> {
    with_session(&sessions, &handle, |s| {
        let template = templates::find(&s.dub().caption_template);
        let cue_count = export_file(
            &s.dub().lines,
            source,
            format,
            std::path::Path::new(&path),
            template.as_ref(),
        )?;
        Ok(ExportCount { cue_count })
    })
}

/// Read a subtitle file and say how it compares with the lines. Changes nothing.
#[tauri::command]
pub fn subtitles_import_preview(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    path: String,
) -> Result<ImportPreview, AppError> {
    let file = read_file(std::path::Path::new(&path))?;
    with_session(&sessions, &handle, |s| Ok(s.import_preview(file)))
}

#[tauri::command]
pub fn subtitles_import_apply(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    preview_id: String,
    choice: ImportChoice,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| s.import_apply(&preview_id, choice))
}

#[tauri::command]
pub fn caption_templates_list() -> Result<Vec<CaptionTemplate>, AppError> {
    templates::bundled()
}

#[tauri::command]
pub fn caption_template_set(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    id: String,
) -> Result<(), AppError> {
    with_session(&sessions, &handle, |s| s.set_caption_template(&id))
}

// --- bulk tools ------------------------------------------------------------------------------

#[tauri::command]
pub fn editor_merge_lines(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    ids: Vec<String>,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| s.apply(EditOp::MergeLines { ids }))
}

#[tauri::command]
pub fn editor_split_line(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    id: String,
    text_offset: usize,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::SplitLine { id, text_offset })
    })
}

#[tauri::command]
pub fn editor_duplicate_line(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    id: String,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::DuplicateLine { id })
    })
}

#[derive(Debug, Deserialize)]
pub struct BulkRow {
    id: String,
    translated: String,
}

#[tauri::command]
pub fn editor_bulk_text(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    texts: Vec<BulkRow>,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::BulkText {
            texts: texts.into_iter().map(|r| (r.id, r.translated)).collect(),
        })
    })
}

#[tauri::command]
pub fn editor_set_speaker_gender(
    sessions: tauri::State<EditorSessions>,
    handle: String,
    speaker_id: String,
    gender: SpeakerGender,
) -> Result<EditResult, AppError> {
    with_session(&sessions, &handle, |s| {
        s.apply(EditOp::SetSpeakerGender { speaker_id, gender })
    })
}

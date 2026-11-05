//! The work folder beside a project and the crash-safe `run-state.json` inside it.
//! See `specs/002-core-dub-pipeline/contracts/project-file-format-v2.md`.

use std::path::{Path, PathBuf};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::fsutil::write_atomic;
use crate::project::dub::{PipelineRun, RunState, Stage, StageStatus};

const RUN_STATE: &str = "run-state.json";
const WORK_MANIFEST: &str = "work-manifest.json";

/// Hash of the parts that decide a stage's output. A change in any part gives a new hash.
pub fn input_hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone)]
pub struct WorkFolder {
    dir: PathBuf,
}

impl WorkFolder {
    /// `<project file name without extension>.flick-work` beside the project file.
    pub fn folder_for(project_path: &Path) -> PathBuf {
        let stem = project_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
        project_path.with_file_name(format!("{stem}-work"))
    }

    /// Create the folder if needed and write `work-manifest.json`.
    pub fn create(project_path: &Path, project_id: &str) -> Result<Self, AppError> {
        let dir = Self::folder_for(project_path);
        std::fs::create_dir_all(&dir).map_err(AppError::io)?;
        let manifest = json!({ "projectId": project_id, "formatVersion": 2 });
        write_atomic(
            &dir.join(WORK_MANIFEST),
            &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
        )?;
        Ok(Self { dir })
    }

    /// Open an existing folder without creating it.
    pub fn open(project_path: &Path) -> Option<Self> {
        let dir = Self::folder_for(project_path);
        dir.is_dir().then_some(Self { dir })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn section_dir(&self, index: u32) -> PathBuf {
        self.dir.join("sections").join(format!("{index:04}"))
    }

    pub fn save_run(&self, run: &PipelineRun) -> Result<(), AppError> {
        let bytes = serde_json::to_vec_pretty(run).map_err(|e| AppError::io(e.into()))?;
        write_atomic(&self.dir.join(RUN_STATE), &bytes)
    }

    /// `None` when there is no run, or the file cannot be read. A leftover `.tmp` from a killed
    /// write is ignored because only the renamed file counts.
    pub fn load_run(&self) -> Result<Option<PipelineRun>, AppError> {
        match std::fs::read(self.dir.join(RUN_STATE)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AppError::io(e)),
        }
    }

    /// The lines of every section, read back from the section folders. Used when the project
    /// was closed before it saved the lines of a finished run.
    pub fn load_lines(&self, run: &PipelineRun) -> Vec<crate::project::dub::TranscriptLine> {
        let mut lines: Vec<crate::project::dub::TranscriptLine> = run
            .sections
            .iter()
            .filter_map(|s| std::fs::read(self.section_dir(s.index).join("lines.json")).ok())
            .filter_map(|b| {
                serde_json::from_slice::<Vec<crate::project::dub::TranscriptLine>>(&b).ok()
            })
            .flatten()
            .collect();
        lines.sort_by_key(|l| (l.start_ms, l.id.clone()));
        lines
    }

    /// A run still marked running or cancelling when the app starts was cut short.
    pub fn is_interrupted(&self) -> bool {
        matches!(
            self.load_run(),
            Ok(Some(PipelineRun {
                state: RunState::Running | RunState::Cancelling,
                ..
            }))
        )
    }

    /// Resume may skip a section stage only if it is recorded as done, the recorded hash matches
    /// `hash`, and every output file exists and parses (`.json` files must be valid JSON).
    pub fn can_skip(
        &self,
        run: &PipelineRun,
        section: u32,
        stage: Stage,
        hash: &str,
        outputs: &[&Path],
    ) -> bool {
        let Some(sec) = run.sections.iter().find(|s| s.index == section) else {
            return false;
        };
        if sec.status.get(&stage) != Some(&StageStatus::Done)
            || sec.input_hash.get(&stage).map(String::as_str) != Some(hash)
        {
            return false;
        }
        outputs.iter().all(|p| output_is_good(p))
    }
}

fn output_is_good(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    if path.extension().is_some_and(|e| e == "json") {
        return serde_json::from_slice::<serde_json::Value>(&bytes).is_ok();
    }
    !bytes.is_empty()
}

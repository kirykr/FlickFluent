//! Projects: a single portable `.flick` file with backups, crash recovery and a one-writer lock.

pub mod dub;
mod format;
mod lock;
pub mod recent;
pub mod recover;
mod save;

pub use format::{Manifest, Project, SUPPORTED_FORMAT_VERSION};
pub use save::BACKUPS;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{AppError, DAMAGED, READ_ONLY};
use lock::{LockGuard, LockResult};

/// Unsaved changes are written to the recovery file this often.
pub const AUTOSAVE_SECS: u64 = 30;

/// `<file>` + suffix, for the `.lock`, `.bak.N`, `.recover` and `.tmp` companions.
pub(crate) fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// A safe file name for a project name. Keeps non-Latin letters; replaces characters that
/// Windows or macOS refuse.
pub fn file_name_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "\\/:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_string();
    let base = if cleaned.is_empty() {
        "Untitled".to_string()
    } else {
        cleaned
    };
    if base.to_lowercase().ends_with(".flick") {
        base
    } else {
        format!("{base}.flick")
    }
}

/// One open project: the data, where it lives, whether it has unsaved changes, and its lock.
pub struct ProjectSession {
    pub project: Project,
    pub path: PathBuf,
    pub dirty: bool,
    /// Another process has the project open, so this one may look but not save.
    pub read_only: bool,
    /// This session wrote or restored the recovery file, so closing it may delete it. A session
    /// that merely opened the project must leave a pending recovery file for the user to decide.
    owns_recovery: bool,
    lock: Option<LockGuard>,
}

pub struct OpenOutcome {
    pub session: ProjectSession,
    /// A newer recovery file exists (the app probably closed unexpectedly).
    pub recovery_available: bool,
}

fn read_only_error() -> AppError {
    AppError::new(READ_ONLY, "error.readOnly")
}

impl ProjectSession {
    /// Create the file straight away, so a new project always has a place on disk.
    pub fn create(name: &str, path: &Path) -> Result<Self, AppError> {
        let lock = match lock::acquire(path)? {
            LockResult::Acquired(g) => g,
            LockResult::Held => return Err(read_only_error()),
        };
        let mut project = Project::new(name);
        save::save_with_backups(&mut project, path)?;
        Ok(Self {
            project,
            path: path.to_path_buf(),
            dirty: false,
            read_only: false,
            owns_recovery: false,
            lock: Some(lock),
        })
    }

    pub fn open(path: &Path) -> Result<OpenOutcome, AppError> {
        let project = Project::read(path).map_err(|e| {
            if e.code == DAMAGED {
                let available = recover::any_recovery_source(path);
                e.with_details(json!({ "backupAvailable": available }))
            } else {
                e
            }
        })?;
        let (lock, read_only) = match lock::acquire(path)? {
            LockResult::Acquired(g) => (Some(g), false),
            LockResult::Held => (None, true),
        };
        Ok(OpenOutcome {
            recovery_available: !read_only && recover::recovery_available(path),
            session: Self {
                project,
                path: path.to_path_buf(),
                dirty: false,
                read_only,
                owns_recovery: false,
                lock,
            },
        })
    }

    /// Open from the recovery file or the newest good backup. The result is marked dirty so the
    /// user saves it on purpose.
    pub fn open_recovered(path: &Path) -> Result<Self, AppError> {
        let project = recover::recover(path)?;
        let lock = match lock::acquire(path)? {
            LockResult::Acquired(g) => g,
            LockResult::Held => return Err(read_only_error()),
        };
        Ok(Self {
            project,
            path: path.to_path_buf(),
            dirty: true,
            read_only: false,
            owns_recovery: true,
            lock: Some(lock),
        })
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Replace the feature data and mark the project changed.
    pub fn set_data(&mut self, data: Value) {
        self.project.data = data;
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<(), AppError> {
        if self.read_only {
            return Err(read_only_error());
        }
        save::save_with_backups(&mut self.project, &self.path)?;
        self.dirty = false;
        Ok(())
    }

    /// Save to a new file and keep working there.
    pub fn save_as(&mut self, new_path: &Path) -> Result<(), AppError> {
        if self.read_only {
            return Err(read_only_error());
        }
        let new_lock = match lock::acquire(new_path)? {
            LockResult::Acquired(g) => g,
            LockResult::Held => return Err(read_only_error()),
        };
        save::save_with_backups(&mut self.project, new_path)?;
        recover::clear_recovery(&self.path);
        self.path = new_path.to_path_buf();
        self.lock = Some(new_lock); // the old lock is released here
        self.dirty = false;
        Ok(())
    }

    /// Called every `AUTOSAVE_SECS`. Writes the recovery file only if there is something to lose.
    pub fn autosave_tick(&mut self) -> Result<bool, AppError> {
        if !self.dirty || self.read_only {
            return Ok(false);
        }
        recover::write_recovery(&self.project, &self.path)?;
        self.owns_recovery = true;
        Ok(true)
    }

    /// Close on purpose: the recovery file is no longer needed and the lock is released.
    pub fn close(self) {
        if self.owns_recovery && !self.read_only {
            recover::clear_recovery(&self.path);
        }
    }
}

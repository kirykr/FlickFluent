//! Crash recovery. While a project has unsaved changes the app writes `<file>.recover` every
//! `AUTOSAVE_SECS` seconds, so a crash loses at most that much work (SC-005).

use std::path::{Path, PathBuf};

use super::format::Project;
use super::save::BACKUPS;
use super::sidecar;
use crate::error::{AppError, DAMAGED};

pub fn recovery_path(path: &Path) -> PathBuf {
    sidecar(path, ".recover")
}

pub fn backup_path(path: &Path, n: u32) -> PathBuf {
    sidecar(path, &format!(".bak.{n}"))
}

pub fn write_recovery(project: &Project, path: &Path) -> Result<(), AppError> {
    project.write_atomic(&recovery_path(path))
}

pub fn clear_recovery(path: &Path) {
    let _ = std::fs::remove_file(recovery_path(path));
}

/// A recovery file is only worth offering when it is newer than the saved file.
pub fn recovery_available(path: &Path) -> bool {
    let recover = recovery_path(path);
    let Ok(recover_time) = std::fs::metadata(&recover).and_then(|m| m.modified()) else {
        return false;
    };
    match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(saved_time) => recover_time > saved_time,
        Err(_) => true,
    }
}

/// True when something can be restored: a recovery file or at least one backup.
pub fn any_recovery_source(path: &Path) -> bool {
    recovery_path(path).exists() || (1..=BACKUPS).any(|n| backup_path(path, n).exists())
}

/// Restore from the newest readable source: the recovery file first, then `.bak.1` to `.bak.3`.
pub fn recover(path: &Path) -> Result<Project, AppError> {
    std::iter::once(recovery_path(path))
        .chain((1..=BACKUPS).map(|n| backup_path(path, n)))
        .filter(|p| p.exists())
        .find_map(|p| Project::read(&p).ok())
        .ok_or_else(|| AppError::new(DAMAGED, "error.damaged"))
}

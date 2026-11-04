use std::path::Path;

use super::format::{now, Project};
use super::recover::{backup_path, clear_recovery};
use crate::error::AppError;

/// How many previous versions are kept next to the project file.
pub const BACKUPS: u32 = 3;

/// Save with rolling backups: the old file becomes `.bak.1`, `.bak.1` becomes `.bak.2`, and so on.
pub fn save_with_backups(project: &mut Project, path: &Path) -> Result<(), AppError> {
    project.manifest.saved_at = now();
    rotate_backups(path)?;
    project.write_atomic(path)?;
    clear_recovery(path);
    Ok(())
}

fn rotate_backups(path: &Path) -> Result<(), AppError> {
    // Never push a damaged file into the backup chain; that would push out the good copies.
    if !path.exists() || Project::read(path).is_err() {
        return Ok(());
    }
    for n in (1..BACKUPS).rev() {
        let from = backup_path(path, n);
        if from.exists() {
            std::fs::rename(&from, backup_path(path, n + 1)).map_err(AppError::io)?;
        }
    }
    std::fs::copy(path, backup_path(path, 1)).map_err(AppError::io)?;
    Ok(())
}

//! Where models live on disk, which ones are installed, and how much space is free.
//! Layout: `<models folder>/<id>/<file>` and `<file>.part` while downloading, plus `index.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::Disks;

use super::CatalogueEntry;
use crate::error::{AppError, NO_SPACE};
use crate::fsutil::write_atomic;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub id: String,
    pub path: String,
    pub size_bytes: u64,
}

fn file_name(entry: &CatalogueEntry) -> String {
    if let Some(name) = &entry.file_name {
        return name.clone();
    }
    let path = entry.url.split(['?', '#']).next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    if last.is_empty() {
        "model.bin".to_string()
    } else {
        last.to_string()
    }
}

pub fn final_path(dir: &Path, entry: &CatalogueEntry) -> PathBuf {
    dir.join(&entry.id).join(file_name(entry))
}

pub fn part_path(dir: &Path, entry: &CatalogueEntry) -> PathBuf {
    let mut p = final_path(dir, entry).into_os_string();
    p.push(".part");
    PathBuf::from(p)
}

/// How many bytes of an unfinished download are already on disk.
pub fn part_bytes(dir: &Path, entry: &CatalogueEntry) -> u64 {
    std::fs::metadata(part_path(dir, entry))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Free space on the drive that holds `dir`. The folder may not exist yet, so the nearest
/// existing parent is used. `None` when it cannot be read.
pub fn free_space(dir: &Path) -> Option<u64> {
    let mut probe = dir;
    while !probe.exists() {
        probe = probe.parent()?;
    }
    let probe = probe.canonicalize().ok()?;
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| probe.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}

/// Refuse before downloading if the disk is too full. An unreadable value does not block.
pub fn preflight_space(needed: u64, dir: &Path) -> Result<(), AppError> {
    match free_space(dir) {
        Some(free) if free < needed => Err(AppError::new(NO_SPACE, "models.error.noSpace")
            .with_details(serde_json::json!({ "neededBytes": needed, "freeBytes": free }))),
        _ => Ok(()),
    }
}

fn index_path(dir: &Path) -> PathBuf {
    dir.join("index.json")
}

/// A missing or damaged index means nothing is recorded as installed.
pub fn installed(dir: &Path) -> Vec<InstalledModel> {
    std::fs::read_to_string(index_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_index(dir: &Path, list: &[InstalledModel]) -> Result<(), AppError> {
    let json = serde_json::to_vec_pretty(list).map_err(|e| AppError::io(e.into()))?;
    write_atomic(&index_path(dir), &json)
}

fn tree_size(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| tree_size(&e.path()))
            .sum(),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// `path` is the installed file, or the folder for a model with several files, the runtime or
/// ffmpeg.
pub fn record_installed(dir: &Path, entry: &CatalogueEntry, path: &Path) -> Result<(), AppError> {
    let size_bytes = tree_size(path);
    let mut list = installed(dir);
    list.retain(|m| m.id != entry.id);
    list.push(InstalledModel {
        id: entry.id.clone(),
        path: path.to_string_lossy().into_owned(),
        size_bytes,
    });
    save_index(dir, &list)
}

/// Remove the model's files and its index entry. Files that are already gone are fine.
pub fn delete_model(dir: &Path, entry: &CatalogueEntry) -> Result<(), AppError> {
    for p in [final_path(dir, entry), part_path(dir, entry)] {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::io(e)),
        }
    }
    // A model with several files lives in its own folder; the runtime and ffmpeg unpack to
    // shared folders named after their step.
    let mut folders = vec![dir.join(&entry.id)];
    match entry.step {
        super::Step::Runtime => folders.push(dir.join("runtime")),
        super::Step::Ffmpeg => folders.push(dir.join("ffmpeg")),
        _ => {}
    }
    for folder in folders {
        match std::fs::remove_dir_all(&folder) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::io(e)),
        }
    }
    let mut list = installed(dir);
    list.retain(|m| m.id != entry.id);
    save_index(dir, &list)
}

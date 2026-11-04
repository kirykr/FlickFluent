//! Install any catalogue entry: a single file, a model with several files, the Python runtime or
//! ffmpeg. One entry point so the app and the tests install things the same way.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use super::runtime::{install as install_runtime, lock_for, runtime_dir, unpack_tar_gz};
use super::{download, record_installed, CatalogueEntry, Control, Outcome, Phase, Progress, Step};
use crate::error::AppError;

/// What the install needs to know about this computer and the app.
pub struct Target<'a> {
    pub models_dir: &'a Path,
    pub os: &'a str,
    pub arch: &'a str,
    /// Arguments that start the sidecar from the runtime (the launcher script and its folders).
    pub sidecar_args: Vec<OsString>,
}

/// Unpack a `.zip` into `dest`, refusing entries that would land outside it.
pub fn unpack_zip(archive: &Path, dest: &Path) -> Result<(), AppError> {
    let file = std::fs::File::open(archive).map_err(AppError::io)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?;
    std::fs::create_dir_all(dest).map_err(AppError::io)?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?;
        let rel = entry.enclosed_name().ok_or_else(|| {
            AppError::io(std::io::Error::other(format!(
                "archive entry outside the target folder: {}",
                entry.name()
            )))
        })?;
        if !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(AppError::io(std::io::Error::other(
                "archive entry outside the target folder",
            )));
        }
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(AppError::io)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::io)?;
        }
        let mut f = std::fs::File::create(&out).map_err(AppError::io)?;
        std::io::copy(&mut entry, &mut f).map_err(AppError::io)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

/// The folder that holds the ffmpeg program, searched a few levels down.
fn find_bin_dir(root: &Path, depth: u32) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if root.join(exe).is_file() {
        return Some(root.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .find_map(|e| find_bin_dir(&e.path(), depth - 1))
}

/// Archives hold a top folder with `bin/` inside (and licence files beside it). Move `bin/` to
/// `<models>/ffmpeg/bin`, keep the licence files next to it.
fn place_ffmpeg(staging: &Path, models_dir: &Path) -> Result<PathBuf, AppError> {
    let bin = find_bin_dir(staging, 3)
        .ok_or_else(|| AppError::io(std::io::Error::other("no ffmpeg program in the archive")))?;
    let root = models_dir.join("ffmpeg");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(AppError::io)?;
    std::fs::rename(&bin, root.join("bin")).map_err(AppError::io)?;
    if let Some(top) = bin.parent() {
        for e in std::fs::read_dir(top).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if e.path().is_file() && (name.starts_with("license") || name.starts_with("copying")) {
                let _ = std::fs::copy(e.path(), root.join(e.file_name()));
            }
        }
    }
    let _ = std::fs::remove_dir_all(staging);
    Ok(root)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?
}

/// Download, verify and place `entry`, then record it as installed. Pause and cancel work per
/// file. For several files, progress counts bytes across all of them.
pub async fn install_entry(
    entry: &CatalogueEntry,
    target: &Target<'_>,
    control: &Control,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<Outcome, AppError> {
    let dir = target.models_dir;
    let placed = match entry.step {
        Step::Runtime => {
            let lock = lock_for(target.os, target.arch);
            match install_runtime(
                entry,
                dir,
                control,
                on_progress,
                lock,
                target.sidecar_args.clone(),
                Duration::from_secs(120),
            )
            .await?
            {
                Outcome::Completed(p) => p,
                stopped => return Ok(stopped),
            }
        }
        Step::Ffmpeg => {
            let archive = match download(entry, dir, control, on_progress).await? {
                Outcome::Completed(p) => p,
                stopped => return Ok(stopped),
            };
            let staging = dir.join("ffmpeg.staging");
            let (from, to, models) = (archive.clone(), staging.clone(), dir.to_path_buf());
            let name = archive.to_string_lossy().to_lowercase();
            let placed = blocking(move || {
                let _ = std::fs::remove_dir_all(&to);
                if name.ends_with(".zip") {
                    unpack_zip(&from, &to)?;
                } else {
                    unpack_tar_gz(&from, &to)?;
                }
                place_ffmpeg(&to, &models)
            })
            .await;
            let _ = std::fs::remove_dir_all(&staging);
            let placed = placed?;
            let _ = std::fs::remove_file(&archive);
            placed
        }
        _ => {
            let pieces = entry.pieces();
            let total: u64 = pieces.iter().map(|p| p.size_bytes).sum();
            let mut done_before = 0u64;
            let mut last = PathBuf::new();
            for piece in &pieces {
                let base = done_before;
                let outcome = download(piece, dir, control, &mut |p: Progress| {
                    on_progress(Progress {
                        bytes_done: base + p.bytes_done,
                        size_bytes: total,
                        phase: p.phase,
                    });
                })
                .await?;
                match outcome {
                    Outcome::Completed(p) => last = p,
                    stopped => return Ok(stopped),
                }
                done_before += piece.size_bytes;
            }
            if entry.files.is_empty() {
                last
            } else {
                dir.join(&entry.id)
            }
        }
    };
    record_installed(dir, entry, &placed)?;
    on_progress(Progress {
        bytes_done: entry.size_bytes,
        size_bytes: entry.size_bytes,
        phase: Phase::Verifying,
    });
    Ok(Outcome::Completed(placed))
}

pub fn is_runtime_installed(models_dir: &Path) -> bool {
    runtime_dir(models_dir).join("requirements.lock").exists()
        || super::runtime::python_exe(models_dir).is_ok()
}

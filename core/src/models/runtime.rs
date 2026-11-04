//! The Python runtime pack: an interpreter archive from the catalogue, unpacked into the models
//! folder. Python is never in the installer (installer size budget), so it is a model-style
//! download with the size shown first.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use flate2::read::GzDecoder;

use super::{download, Control, Outcome, Progress};
use crate::error::{AppError, RUNTIME_MISSING};
use crate::models::CatalogueEntry;
use crate::sidecar::{Sidecar, SidecarConfig};

/// Where the runtime lives once unpacked: `<models>/runtime/`.
pub fn runtime_dir(models_dir: &Path) -> PathBuf {
    models_dir.join("runtime")
}

/// The interpreter inside an unpacked pack. python-build-standalone puts it in `python/bin` or
/// `python/` (Windows).
pub fn python_exe(models_dir: &Path) -> Result<PathBuf, AppError> {
    let root = runtime_dir(models_dir);
    let candidates = if cfg!(windows) {
        [
            root.join("python").join("python.exe"),
            root.join("python.exe"),
        ]
    } else {
        [
            root.join("python").join("bin").join("python3"),
            root.join("bin").join("python3"),
        ]
    };
    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| AppError::new(RUNTIME_MISSING, "dub.error.runtimeMissing"))
}

/// Unpack a `.tar.gz` into `dest`. Entries that would land outside `dest` (absolute paths or
/// `..`) are refused, so a bad archive cannot write elsewhere.
pub fn unpack_tar_gz(archive: &Path, dest: &Path) -> Result<(), AppError> {
    let file = std::fs::File::open(archive).map_err(AppError::io)?;
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    std::fs::create_dir_all(dest).map_err(AppError::io)?;
    for entry in tar.entries().map_err(AppError::io)? {
        let mut entry = entry.map_err(AppError::io)?;
        let rel = entry.path().map_err(AppError::io)?.into_owned();
        let safe = rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !safe {
            return Err(AppError::io(std::io::Error::other(format!(
                "archive entry outside the target folder: {}",
                rel.display()
            ))));
        }
        entry.unpack_in(dest).map_err(AppError::io)?;
    }
    Ok(())
}

/// The pinned, hashed list of Python packages for a platform (`sidecar/locks/`). `None` when no
/// complete set of packages exists for it, so the engine cannot run there.
pub fn lock_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "arm64") => Some(include_str!("../../../sidecar/locks/macos-arm64.txt")),
        ("macos", "x64") => Some(include_str!("../../../sidecar/locks/macos-x64.txt")),
        ("windows", "x64") => Some(include_str!("../../../sidecar/locks/windows-x64.txt")),
        _ => None,
    }
}

/// Install the packages of `lock` into the unpacked runtime. Every package is pinned with a hash
/// and only wheels are used, so nothing is built or swapped on the way.
async fn install_packages(models_dir: &Path, lock: &str) -> Result<(), AppError> {
    let python = python_exe(models_dir)?;
    let file = runtime_dir(models_dir).join("requirements.lock");
    std::fs::write(&file, lock).map_err(AppError::io)?;
    let out = tokio::process::Command::new(&python)
        .args([
            "-m",
            "pip",
            "install",
            "--require-hashes",
            "--only-binary=:all:",
            "--no-deps",
        ])
        .args(["--disable-pip-version-check", "--no-input", "-r"])
        .arg(&file)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(AppError::io)?;
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = text
        .lines()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Err(AppError::new(RUNTIME_MISSING, "dub.error.runtimeMissing")
        .with_details(serde_json::json!({ "pip": tail })))
}

/// Download the pack, verify it, unpack it to `<models>/runtime/`, install `requirements` (the
/// hashed package list, when there is one), then start the sidecar from it and complete the
/// `hello` handshake. `health_args` are the arguments given to the interpreter to start the
/// sidecar. On any failure after unpacking, the runtime folder is removed so the next attempt
/// starts clean.
///
/// The size is `entry.size_bytes`; the caller shows it before calling (FR-014).
pub async fn install(
    entry: &CatalogueEntry,
    models_dir: &Path,
    control: &Control,
    on_progress: &mut (dyn FnMut(Progress) + Send),
    requirements: Option<&str>,
    health_args: Vec<OsString>,
    start_timeout: Duration,
) -> Result<Outcome, AppError> {
    let archive = match download(entry, models_dir, control, on_progress).await? {
        Outcome::Completed(path) => path,
        stopped => return Ok(stopped),
    };

    let root = runtime_dir(models_dir);
    let staging = models_dir.join("runtime.staging");
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&root);
    let (from, to) = (archive.clone(), staging.clone());
    let unpacked = tokio::task::spawn_blocking(move || unpack_tar_gz(&from, &to))
        .await
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?;
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    std::fs::rename(&staging, &root).map_err(AppError::io)?;
    let _ = std::fs::remove_file(&archive);

    if let Some(lock) = requirements {
        if let Err(e) = install_packages(models_dir, lock).await {
            let _ = std::fs::remove_dir_all(&root);
            return Err(e);
        }
    }
    if let Err(e) = health_check(models_dir, health_args, start_timeout).await {
        let _ = std::fs::remove_dir_all(&root);
        return Err(e);
    }
    Ok(Outcome::Completed(root))
}

async fn health_check(
    models_dir: &Path,
    args: Vec<OsString>,
    start_timeout: Duration,
) -> Result<(), AppError> {
    let program = python_exe(models_dir)?;
    // `Sidecar::start` returns only after the handshake, so a started sidecar is a healthy one.
    let sidecar = Sidecar::start(SidecarConfig {
        program,
        args,
        hang_timeout: start_timeout,
        start_timeout,
    })
    .await?;
    drop(sidecar);
    Ok(())
}

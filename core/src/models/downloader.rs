//! Resumable download: streams to `<file>.part`, resumes with an HTTP Range request, verifies
//! the SHA-256, then renames into place. A file that fails verification is never installed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use super::storage::{final_path, part_bytes, part_path, preflight_space};
use super::verify::verify_file;
use super::CatalogueEntry;
use crate::error::{AppError, HASH_MISMATCH, OFFLINE, SERVER_ERROR};

const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

/// Lets another task pause or cancel a running download.
#[derive(Clone, Default)]
pub struct Control(Arc<AtomicU8>);

impl Control {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stop the transfer and keep the `.part` file so it can be resumed.
    pub fn pause(&self) {
        self.0.store(PAUSE, Ordering::SeqCst);
    }

    /// Stop the transfer and delete the `.part` file.
    pub fn cancel(&self) {
        self.0.store(CANCEL, Ordering::SeqCst);
    }

    fn state(&self) -> u8 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Downloading,
    Verifying,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub bytes_done: u64,
    pub size_bytes: u64,
    pub phase: Phase,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed(PathBuf),
    Paused,
    Cancelled,
}

fn network_error(e: reqwest::Error) -> AppError {
    AppError::new(OFFLINE, "models.error.offline")
        .with_details(serde_json::json!({ "detail": e.to_string() }))
}

/// Download (or resume) a model into `models_dir`.
///
/// Errors: `NO_SPACE` before any request is made, `OFFLINE` when the network fails (progress is
/// kept for a later resume), `SERVER_ERROR` for an unexpected HTTP status, `HASH_MISMATCH` when
/// the finished file is not the expected one (the file is deleted).
pub async fn download(
    entry: &CatalogueEntry,
    models_dir: &Path,
    control: &Control,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<Outcome, AppError> {
    let target = final_path(models_dir, entry);
    let part = part_path(models_dir, entry);
    std::fs::create_dir_all(target.parent().unwrap_or(models_dir)).map_err(AppError::io)?;

    let mut have = part_bytes(models_dir, entry);
    if have > entry.size_bytes {
        // Not a prefix of this file, so it cannot be resumed.
        let _ = std::fs::remove_file(&part);
        have = 0;
    }
    if have < entry.size_bytes {
        preflight_space(entry.size_bytes - have, models_dir)?;
        if let Some(stopped) = transfer(entry, &part, have, control, on_progress).await? {
            return Ok(stopped);
        }
    }

    on_progress(Progress {
        bytes_done: entry.size_bytes,
        size_bytes: entry.size_bytes,
        phase: Phase::Verifying,
    });
    let (check, expected) = (part.clone(), entry.sha256.clone());
    let verified = tokio::task::spawn_blocking(move || verify_file(&check, &expected))
        .await
        .map_err(|e| AppError::io(std::io::Error::other(e.to_string())))?;
    if let Err(e) = verified {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &target).map_err(AppError::io)?;
    Ok(Outcome::Completed(target))
}

/// Returns `Some(outcome)` if stopped by the user, `None` when all bytes have arrived.
async fn transfer(
    entry: &CatalogueEntry,
    part: &Path,
    mut have: u64,
    control: &Control,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<Option<Outcome>, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(network_error)?;

    let mut attempt = 0;
    let (mut response, resumed) = loop {
        let mut request = client.get(&entry.url);
        if have > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let response = request.send().await.map_err(network_error)?;
        match response.status().as_u16() {
            206 => break (response, true),
            // The server ignored Range and is sending the whole file: start again from zero.
            200 => {
                have = 0;
                break (response, false);
            }
            // Our partial file is not usable: drop it and ask for everything once.
            416 if have > 0 && attempt == 0 => {
                let _ = std::fs::remove_file(part);
                have = 0;
                attempt += 1;
            }
            status => {
                return Err(AppError::new(SERVER_ERROR, "models.error.server")
                    .with_details(serde_json::json!({ "status": status })))
            }
        }
    };

    let mut file = if resumed {
        std::fs::OpenOptions::new().append(true).open(part)
    } else {
        std::fs::File::create(part)
    }
    .map_err(AppError::io)?;

    loop {
        match control.state() {
            PAUSE => {
                file.flush().map_err(AppError::io)?;
                return Ok(Some(Outcome::Paused));
            }
            CANCEL => {
                drop(file);
                let _ = std::fs::remove_file(part);
                return Ok(Some(Outcome::Cancelled));
            }
            _ => {}
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk).map_err(AppError::io)?;
                have += chunk.len() as u64;
                on_progress(Progress {
                    bytes_done: have,
                    size_bytes: entry.size_bytes,
                    phase: Phase::Downloading,
                });
            }
            Ok(None) => break,
            Err(e) => {
                let _ = file.flush();
                return Err(network_error(e));
            }
        }
    }
    file.sync_all().map_err(AppError::io)?;
    drop(file);

    match have.cmp(&entry.size_bytes) {
        std::cmp::Ordering::Equal => Ok(None),
        // The connection ended early without an error. Keep what arrived and let the user resume.
        std::cmp::Ordering::Less => Err(AppError::new(OFFLINE, "models.error.offline")
            .with_details(serde_json::json!({ "detail": "connection closed early" }))),
        std::cmp::Ordering::Greater => {
            let _ = std::fs::remove_file(part);
            Err(AppError::new(HASH_MISMATCH, "models.error.hashMismatch"))
        }
    }
}

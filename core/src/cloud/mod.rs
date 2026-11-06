//! Optional cloud engines for translation and speech, with the user's own keys.
//! See `specs/003-hybrid-cloud-keys`. Nothing secret is written outside the OS secure storage.

pub mod adapter;
pub mod adapters;
pub mod audio;
pub mod choice;
pub mod clock;
pub mod consent;
pub mod engine;
pub mod estimate;
pub mod guard;
pub mod http;
pub mod keys;
pub mod keystore;
pub mod rotation;
pub mod runtime;
pub mod secret;
pub mod services;

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppError;

/// Non-secret list of keys (label, service, region, last four characters, order, state).
pub fn cloud_keys_path(app_data: &Path) -> PathBuf {
    app_data.join("cloud-keys.json")
}

/// The user's agreements to send data, per service and step. Never inside a project.
pub fn cloud_consent_path(app_data: &Path) -> PathBuf {
    app_data.join("cloud-consent.json")
}

/// Cached live language and voice lists.
pub fn voices_cache_dir(app_data: &Path) -> PathBuf {
    app_data.join("cloud-cache")
}

/// Read a JSON file; a file that does not exist reads as the default.
pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, AppError> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
            AppError::new(crate::error::DAMAGED, "error.damaged")
                .with_details(serde_json::json!({ "detail": e.to_string() }))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(AppError::io(e)),
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| {
        AppError::new(crate::error::IO_ERROR, "error.io")
            .with_details(serde_json::json!({ "detail": e.to_string() }))
    })?;
    crate::fsutil::write_atomic(path, &bytes)
}

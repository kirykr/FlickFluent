use std::collections::HashMap;
use std::sync::Mutex;

use super::secret::Secret;
use crate::error::{AppError, KEY_STORE_UNAVAILABLE};

/// Where the secret part of a key lives. Nothing else about a key is secret.
pub trait KeyStore: Send + Sync {
    fn set(&self, key_id: &str, secret: &Secret) -> Result<(), AppError>;
    /// `Ok(None)` when no entry exists.
    fn get(&self, key_id: &str) -> Result<Option<Secret>, AppError>;
    /// Deleting an entry that does not exist is `Ok`.
    fn delete(&self, key_id: &str) -> Result<(), AppError>;
}

fn unavailable() -> AppError {
    AppError::new(KEY_STORE_UNAVAILABLE, "dub.error.keyStoreUnavailable")
}

/// Service name used for every entry in the operating system's secure storage.
pub const SERVICE_NAME: &str = "app.flickfluent.desktop";

fn account(key_id: &str) -> String {
    format!("cloud-key:{key_id}")
}

/// The operating system's secure storage: the Keychain on macOS, Credential Manager on Windows.
/// Call [`OsKeyStore::install`] once at start-up.
pub struct OsKeyStore;

impl OsKeyStore {
    /// Choose the platform store as the default for `keyring-core`. Fails (and nothing is saved
    /// anywhere else) on a platform with no store or when the store cannot be opened.
    pub fn install() -> Result<Self, AppError> {
        #[cfg(target_os = "macos")]
        let store =
            apple_native_keyring_store::keychain::Store::new().map_err(|_| unavailable())?;
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new().map_err(|_| unavailable())?;
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            keyring_core::set_default_store(store);
            Ok(Self)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Err(unavailable())
    }

    fn entry(key_id: &str) -> Result<keyring_core::Entry, AppError> {
        keyring_core::Entry::new(SERVICE_NAME, &account(key_id)).map_err(|_| unavailable())
    }
}

impl KeyStore for OsKeyStore {
    fn set(&self, key_id: &str, secret: &Secret) -> Result<(), AppError> {
        Self::entry(key_id)?
            .set_password(secret.expose())
            .map_err(|_| unavailable())
    }

    fn get(&self, key_id: &str) -> Result<Option<Secret>, AppError> {
        match Self::entry(key_id)?.get_password() {
            Ok(v) => Ok(Some(Secret::new(v))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(unavailable()),
        }
    }

    fn delete(&self, key_id: &str) -> Result<(), AppError> {
        match Self::entry(key_id)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(_) => Err(unavailable()),
        }
    }
}

/// Keeps secrets in memory only. Used by tests and for keys the user keeps for one session.
#[derive(Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, String>>,
    fail: bool,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// A store where every call fails, for tests of the "secure storage is locked" path.
    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }
}

impl KeyStore for MemoryStore {
    fn set(&self, key_id: &str, secret: &Secret) -> Result<(), AppError> {
        if self.fail {
            return Err(unavailable());
        }
        self.entries
            .lock()
            .unwrap()
            .insert(key_id.to_string(), secret.expose().to_string());
        Ok(())
    }

    fn get(&self, key_id: &str) -> Result<Option<Secret>, AppError> {
        if self.fail {
            return Err(unavailable());
        }
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(key_id)
            .map(|v| Secret::new(v.clone())))
    }

    fn delete(&self, key_id: &str) -> Result<(), AppError> {
        if self.fail {
            return Err(unavailable());
        }
        self.entries.lock().unwrap().remove(key_id);
        Ok(())
    }
}

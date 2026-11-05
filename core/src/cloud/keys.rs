use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;

use super::adapter::{Auth, CloudAdapter, FailureKind, Languages};
use super::keystore::{KeyStore, MemoryStore};
use super::secret::Secret;
use super::services::Service;
use super::{cloud_keys_path, read_json, voices_cache_dir, write_json};
use crate::error::{AppError, KEY_DUPLICATE, KEY_INVALID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyState {
    Untested,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Storage {
    Os,
    Session,
}

/// Everything about a key except the secret. Safe to show, save and send to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub id: String,
    pub service_id: String,
    pub label: String,
    pub region: Option<String>,
    pub last4: String,
    /// SHA-256 of the secret, only to refuse the same key twice. Not reversible, never shown.
    pub fingerprint: String,
    pub order: u32,
    pub state: KeyState,
    pub verified_at: Option<String>,
    pub storage: Storage,
}

#[derive(Default, Serialize, Deserialize)]
struct KeyFile {
    keys: Vec<KeyInfo>,
}

const MAX_KEY_CHARS: usize = 512;
const MAX_LABEL_CHARS: usize = 40;

fn invalid() -> AppError {
    AppError::new(KEY_INVALID, "dub.error.keyInvalid")
}

fn fingerprint(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

/// Why a "Verify connection" failed, in the words the screen has texts for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerifyReason {
    Rejected,
    NoCredit,
    Offline,
    Unreachable,
    Unexpected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    pub ok: bool,
    pub languages: Option<Languages>,
    pub reason: Option<VerifyReason>,
    /// The service's own words, already scrubbed of the key.
    pub message: Option<String>,
}

/// The user's keys: rows in `cloud-keys.json`, secrets in the key store, and session-only keys
/// held in memory until the app exits.
pub struct KeyBook {
    app_data: PathBuf,
    store: Arc<dyn KeyStore>,
    services: Vec<Service>,
    session_rows: Mutex<Vec<KeyInfo>>,
    session_store: MemoryStore,
    write_lock: Mutex<()>,
}

impl KeyBook {
    pub fn new(app_data: PathBuf, store: Arc<dyn KeyStore>, services: Vec<Service>) -> Self {
        Self {
            app_data,
            store,
            services,
            session_rows: Mutex::new(Vec::new()),
            session_store: MemoryStore::new(),
            write_lock: Mutex::new(()),
        }
    }

    pub fn services(&self) -> &[Service] {
        &self.services
    }

    fn read_file(&self) -> Result<KeyFile, AppError> {
        read_json(&cloud_keys_path(&self.app_data))
    }

    fn write_file(&self, file: &KeyFile) -> Result<(), AppError> {
        write_json(&cloud_keys_path(&self.app_data), file)
    }

    /// Every key, saved and session-only, by service and then in the user's order.
    pub fn list(&self) -> Result<Vec<KeyInfo>, AppError> {
        let mut all = self.read_file()?.keys;
        all.extend(self.session_rows.lock().unwrap().iter().cloned());
        all.sort_by(|a, b| (&a.service_id, a.order).cmp(&(&b.service_id, b.order)));
        Ok(all)
    }

    pub fn for_service(&self, service_id: &str) -> Result<Vec<KeyInfo>, AppError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|k| k.service_id == service_id)
            .collect())
    }

    pub fn info(&self, id: &str) -> Result<Option<KeyInfo>, AppError> {
        Ok(self.list()?.into_iter().find(|k| k.id == id))
    }

    /// The secret of one key, for building a request. `Ok(None)` when it is gone.
    pub fn secret(&self, id: &str) -> Result<Option<Secret>, AppError> {
        match self.info(id)? {
            Some(k) if k.storage == Storage::Session => self.session_store.get(id),
            Some(_) => self.store.get(id),
            None => Ok(None),
        }
    }

    /// Add a key. The text is trimmed of spaces and line breaks first. When the secure storage
    /// fails the key is not saved anywhere, unless the user asked for `session_only`.
    pub fn add(
        &self,
        service_id: &str,
        label: &str,
        secret_text: String,
        region: Option<String>,
        session_only: bool,
    ) -> Result<KeyInfo, AppError> {
        let service = self
            .services
            .iter()
            .find(|s| s.id == service_id)
            .ok_or_else(invalid)?;
        let text: String = secret_text.chars().filter(|c| !c.is_whitespace()).collect();
        let label = label.trim().to_string();
        let region = region
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());
        if text.is_empty()
            || text.chars().count() > MAX_KEY_CHARS
            || label.is_empty()
            || label.chars().count() > MAX_LABEL_CHARS
            || (service.needs_region && region.is_none())
        {
            return Err(invalid());
        }
        let secret = Secret::new(text);
        let print = fingerprint(secret.expose());

        let _guard = self.write_lock.lock().unwrap();
        let mut file = self.read_file()?;
        let existing: Vec<KeyInfo> = file
            .keys
            .iter()
            .cloned()
            .chain(self.session_rows.lock().unwrap().iter().cloned())
            .filter(|k| k.service_id == service_id)
            .collect();
        if existing
            .iter()
            .any(|k| k.fingerprint == print || k.label == label)
        {
            return Err(AppError::new(KEY_DUPLICATE, "dub.error.keyDuplicate"));
        }
        let key = KeyInfo {
            id: uuid::Uuid::new_v4().to_string(),
            service_id: service_id.to_string(),
            label,
            region: if service.needs_region { region } else { None },
            last4: {
                let chars: Vec<char> = secret.expose().chars().collect();
                chars[chars.len().saturating_sub(4)..].iter().collect()
            },
            fingerprint: print,
            order: existing.iter().map(|k| k.order + 1).max().unwrap_or(0),
            state: KeyState::Untested,
            verified_at: None,
            storage: if session_only {
                Storage::Session
            } else {
                Storage::Os
            },
        };
        if session_only {
            self.session_store.set(&key.id, &secret)?;
            self.session_rows.lock().unwrap().push(key.clone());
        } else {
            self.store.set(&key.id, &secret)?;
            file.keys.push(key.clone());
            if let Err(e) = self.write_file(&file) {
                let _ = self.store.delete(&key.id);
                return Err(e);
            }
        }
        Ok(key)
    }

    /// Delete the secret first, then the row. If the secret cannot be deleted the row stays, so
    /// the user can try again and no secret is left behind without a name.
    pub fn remove(&self, id: &str) -> Result<(), AppError> {
        let _guard = self.write_lock.lock().unwrap();
        let mut file = self.read_file()?;
        if file.keys.iter().any(|k| k.id == id) {
            self.store.delete(id)?;
            file.keys.retain(|k| k.id != id);
            return self.write_file(&file);
        }
        self.session_store.delete(id)?;
        self.session_rows.lock().unwrap().retain(|k| k.id != id);
        Ok(())
    }

    /// Put the keys of one service in this order (the first is tried first). Every id must be a
    /// key of that service.
    pub fn reorder(&self, service_id: &str, ids: &[String]) -> Result<(), AppError> {
        let _guard = self.write_lock.lock().unwrap();
        let mut file = self.read_file()?;
        let mut session = self.session_rows.lock().unwrap();
        let belongs = |k: &KeyInfo, id: &String| k.id == *id && k.service_id == service_id;
        for id in ids {
            if !file
                .keys
                .iter()
                .chain(session.iter())
                .any(|k| belongs(k, id))
            {
                return Err(invalid());
            }
        }
        let mut next = 0u32;
        for id in ids {
            for k in file.keys.iter_mut().chain(session.iter_mut()) {
                if belongs(k, id) {
                    k.order = next;
                }
            }
            next += 1;
        }
        // Keys not named keep their relative order after the named ones.
        let mut rest: Vec<&mut KeyInfo> = file
            .keys
            .iter_mut()
            .chain(session.iter_mut())
            .filter(|k| k.service_id == service_id && !ids.contains(&k.id))
            .collect();
        rest.sort_by_key(|k| k.order);
        for k in rest {
            k.order = next;
            next += 1;
        }
        self.write_file(&file)
    }

    /// Record the result of a "Verify connection".
    pub fn set_state(&self, id: &str, state: KeyState) -> Result<(), AppError> {
        let _guard = self.write_lock.lock().unwrap();
        let stamp = (state == KeyState::Verified).then(now_rfc3339);
        let mut file = self.read_file()?;
        if let Some(k) = file.keys.iter_mut().find(|k| k.id == id) {
            k.state = state;
            k.verified_at = stamp;
            return self.write_file(&file);
        }
        if let Some(k) = self
            .session_rows
            .lock()
            .unwrap()
            .iter_mut()
            .find(|k| k.id == id)
        {
            k.state = state;
            k.verified_at = stamp;
        }
        Ok(())
    }

    /// Ask the service whether the key works and record the answer. Only a refusal (`Rejected`,
    /// `NoCredit`) changes a saved key's state; being offline or a service problem leaves it as it
    /// was, because the key may be fine. A success stores the live language list in the cache.
    pub async fn verify(
        &self,
        id: &str,
        adapter: &dyn CloudAdapter,
    ) -> Result<VerifyResult, AppError> {
        let info = self.info(id)?.ok_or_else(invalid)?;
        let secret = self.secret(id)?.ok_or_else(invalid)?;
        let auth = Auth {
            key: &secret,
            region: info.region.as_deref(),
        };
        match adapter.check(&auth).await {
            Ok(languages) => {
                self.set_state(id, KeyState::Verified)?;
                let cache = voices_cache_dir(&self.app_data)
                    .join(format!("{}-languages.json", info.service_id));
                // The cache is a convenience; a failed write must not fail the verification.
                let _ = write_json(&cache, &languages);
                Ok(VerifyResult {
                    ok: true,
                    languages: Some(languages),
                    reason: None,
                    message: None,
                })
            }
            Err(f) => {
                let reason = match f.kind {
                    FailureKind::Rejected => VerifyReason::Rejected,
                    FailureKind::NoCredit => VerifyReason::NoCredit,
                    FailureKind::Offline => VerifyReason::Offline,
                    FailureKind::Unexpected => VerifyReason::Unexpected,
                    FailureKind::RateLimited | FailureKind::Unavailable => {
                        VerifyReason::Unreachable
                    }
                };
                if matches!(reason, VerifyReason::Rejected | VerifyReason::NoCredit) {
                    self.set_state(id, KeyState::Rejected)?;
                }
                Ok(VerifyResult {
                    ok: false,
                    languages: None,
                    reason: Some(reason),
                    message: Some(f.message),
                })
            }
        }
    }

    /// The live language list saved by the last successful verification of a service, if any.
    pub fn cached_languages(&self, service_id: &str) -> Option<Languages> {
        let path = voices_cache_dir(&self.app_data).join(format!("{service_id}-languages.json"));
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }
}

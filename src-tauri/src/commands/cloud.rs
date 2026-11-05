use std::path::PathBuf;
use std::sync::Arc;

use flickfluent_core::cloud::adapters::adapter_for;
use flickfluent_core::cloud::choice::{validate_engine, Engines};
use flickfluent_core::cloud::consent::{ConsentBook, ConsentRecord, NOTICE_VERSION};
use flickfluent_core::cloud::keys::{KeyBook, KeyInfo, KeyState, Storage, VerifyResult};
use flickfluent_core::cloud::keystore::{KeyStore, MemoryStore, OsKeyStore};
use flickfluent_core::cloud::runtime::CloudRuntime;
use flickfluent_core::cloud::services::{self, Step};
use flickfluent_core::error::{AppError, INVALID_SETTING};
use flickfluent_core::project::dub::CloudUsage;
use serde::Serialize;
use tauri::{Manager, State};

use super::dub::dub_of;
use super::project::ProjectState;
use super::settings::SettingsState;

/// The user's cloud keys. The secret part of a key lives in the operating system's secure
/// storage and goes through this layer only as the argument of `cloud_key_add`.
pub struct CloudState {
    book: Arc<KeyBook>,
    consent: Arc<ConsentBook>,
    runtime: Arc<CloudRuntime>,
}

impl CloudState {
    pub fn load(app: &tauri::AppHandle) -> Self {
        let dir: PathBuf = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("flickfluent"));
        // If the secure storage cannot be opened, saving a key fails with
        // `KEY_STORE_UNAVAILABLE` and the user may keep it for the session only.
        let store: Arc<dyn KeyStore> = match OsKeyStore::install() {
            Ok(os) => Arc::new(os),
            Err(_) => Arc::new(MemoryStore::failing()),
        };
        let all = services::load().expect("config/cloud-services.json is valid");
        let book = Arc::new(KeyBook::new(dir.clone(), store, all));
        let consent = Arc::new(ConsentBook::new(dir.clone()));
        let runtime = Arc::new(CloudRuntime::real(book.clone(), consent.clone(), dir));
        Self {
            book,
            consent,
            runtime,
        }
    }

    /// What each service offers, for the language list and the pair check. A service counts as
    /// set up only when it has a verified key; its live language list (from the last verified
    /// check) replaces the one in the data file.
    pub fn offers(&self) -> Vec<flickfluent_core::languages::CloudOffer> {
        self.book
            .services()
            .iter()
            .map(|s| {
                let live = self.book.cached_languages(&s.id);
                flickfluent_core::languages::CloudOffer {
                    service_id: s.id.clone(),
                    service_name: s.name.clone(),
                    translate: live
                        .as_ref()
                        .map_or_else(|| s.languages.translate.clone(), |l| l.translate.clone()),
                    speak: live
                        .as_ref()
                        .map_or_else(|| s.languages.speak.clone(), |l| l.speak.clone()),
                    has_verified_key: self
                        .book
                        .for_service(&s.id)
                        .map(|keys| keys.iter().any(|k| k.state == KeyState::Verified))
                        .unwrap_or(false),
                }
            })
            .collect()
    }

    /// What a run needs to use a cloud service.
    pub fn runtime(&self) -> Arc<CloudRuntime> {
        self.runtime.clone()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudServiceView {
    id: String,
    name: String,
    steps: Vec<Step>,
    needs_region: bool,
    terms_url: String,
    languages: flickfluent_core::cloud::adapter::Languages,
}

/// A key as the screen sees it: no secret and no fingerprint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    id: String,
    service_id: String,
    label: String,
    region: Option<String>,
    last4: String,
    order: u32,
    state: KeyState,
    verified_at: Option<String>,
    storage: Storage,
}

impl From<KeyInfo> for KeyView {
    fn from(k: KeyInfo) -> Self {
        Self {
            id: k.id,
            service_id: k.service_id,
            label: k.label,
            region: k.region,
            last4: k.last4,
            order: k.order,
            state: k.state,
            verified_at: k.verified_at,
            storage: k.storage,
        }
    }
}

#[tauri::command]
pub fn cloud_services(state: State<'_, CloudState>) -> Vec<CloudServiceView> {
    state
        .book
        .services()
        .iter()
        .map(|s| CloudServiceView {
            id: s.id.clone(),
            name: s.name.clone(),
            steps: s.steps.clone(),
            needs_region: s.needs_region,
            terms_url: s.terms_url.clone(),
            languages: state.book.cached_languages(&s.id).unwrap_or(
                flickfluent_core::cloud::adapter::Languages {
                    translate: s.languages.translate.clone(),
                    speak: s.languages.speak.clone(),
                },
            ),
        })
        .collect()
}

#[tauri::command]
pub fn cloud_keys_list(state: State<'_, CloudState>) -> Result<Vec<KeyView>, AppError> {
    Ok(state.book.list()?.into_iter().map(KeyView::from).collect())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn cloud_key_add(
    state: State<'_, CloudState>,
    service_id: String,
    label: String,
    secret: String,
    region: Option<String>,
    session_only: Option<bool>,
) -> Result<KeyView, AppError> {
    state
        .book
        .add(
            &service_id,
            &label,
            secret,
            region,
            session_only.unwrap_or(false),
        )
        .map(KeyView::from)
}

#[tauri::command]
pub async fn cloud_key_verify(
    state: State<'_, CloudState>,
    key_id: String,
) -> Result<VerifyResult, AppError> {
    let info = state
        .book
        .info(&key_id)?
        .ok_or_else(|| AppError::new(INVALID_SETTING, "error.invalidSetting"))?;
    let service = services::find(state.book.services(), &info.service_id)
        .ok_or_else(|| AppError::new(INVALID_SETTING, "error.invalidSetting"))?;
    let adapter = adapter_for(service)
        .ok_or_else(|| AppError::new(INVALID_SETTING, "error.invalidSetting"))?;
    state.book.verify(&key_id, adapter.as_ref()).await
}

#[tauri::command]
pub fn cloud_key_remove(state: State<'_, CloudState>, key_id: String) -> Result<(), AppError> {
    state.book.remove(&key_id)
}

#[tauri::command]
pub fn cloud_keys_reorder(
    state: State<'_, CloudState>,
    service_id: String,
    key_ids: Vec<String>,
) -> Result<Vec<KeyView>, AppError> {
    state.book.reorder(&service_id, &key_ids)?;
    Ok(state
        .book
        .for_service(&service_id)?
        .into_iter()
        .map(KeyView::from)
        .collect())
}

// ---- engines, consent and usage ---------------------------------------------------------------

#[tauri::command]
pub fn cloud_engines_get(settings: State<'_, SettingsState>) -> Engines {
    settings.engines()
}

#[tauri::command]
pub fn cloud_engines_set(
    state: State<'_, CloudState>,
    settings: State<'_, SettingsState>,
    step: Step,
    engine: String,
) -> Result<Engines, AppError> {
    validate_engine(&state.book, step, &engine)?;
    settings.set_engine(step, &engine)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoticeView {
    service_name: String,
    step: Step,
    /// i18n keys: what is sent, then what is not, then the bill.
    sends: Vec<String>,
    does_not_send: Vec<String>,
    terms_url: String,
    estimated_chars: u64,
    notice_version: u32,
}

/// What the consent dialog shows. Only i18n keys and the service's name and link: no free text
/// from a vendor.
#[tauri::command]
pub fn cloud_notice(
    state: State<'_, CloudState>,
    service_id: String,
    step: Step,
    estimated_chars: u64,
) -> Result<NoticeView, AppError> {
    let service = services::find(state.book.services(), &service_id)
        .filter(|s| s.offers(step))
        .ok_or_else(|| AppError::new(INVALID_SETTING, "settings.error.invalid"))?;
    let key = match step {
        Step::Translate => service.data_notice.translate.clone(),
        Step::Speak => service.data_notice.speak.clone(),
    };
    Ok(NoticeView {
        service_name: service.name.clone(),
        step,
        sends: key.into_iter().collect(),
        does_not_send: vec![
            "cloud.notice.doesNotSend".into(),
            "cloud.notice.bill".into(),
        ],
        terms_url: service.terms_url.clone(),
        estimated_chars,
        notice_version: NOTICE_VERSION,
    })
}

#[tauri::command]
pub fn cloud_consent_list(state: State<'_, CloudState>) -> Result<Vec<ConsentRecord>, AppError> {
    state.consent.list()
}

#[tauri::command]
pub fn cloud_consent_grant(
    state: State<'_, CloudState>,
    service_id: String,
    step: Step,
    estimated_chars: u64,
) -> Result<ConsentRecord, AppError> {
    services::find(state.book.services(), &service_id)
        .filter(|s| s.offers(step))
        .ok_or_else(|| AppError::new(INVALID_SETTING, "settings.error.invalid"))?;
    state.consent.grant(&service_id, step, estimated_chars)
}

#[tauri::command]
pub fn cloud_consent_withdraw(
    state: State<'_, CloudState>,
    service_id: String,
    step: Step,
) -> Result<(), AppError> {
    state.consent.withdraw(&service_id, step)
}

/// What the cloud steps of a project's last run used.
#[tauri::command]
pub fn cloud_usage(
    projects: State<'_, ProjectState>,
    handle: String,
) -> Result<Vec<CloudUsage>, AppError> {
    Ok(dub_of(&projects, &handle)?.0.cloud_usage)
}

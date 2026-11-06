use serde::Serialize;

/// Error shape sent to the UI. `code` is stable; `message_key` is an i18n key.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code}: {message_key}")]
pub struct AppError {
    pub code: &'static str,
    pub message_key: String,
    pub details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(code: &'static str, message_key: impl Into<String>) -> Self {
        Self {
            code,
            message_key: message_key.into(),
            details: None,
        }
    }

    pub fn io(e: std::io::Error) -> Self {
        Self::new(IO_ERROR, "error.io").with_details(serde_json::json!({ "detail": e.to_string() }))
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

pub const FORMAT_TOO_NEW: &str = "FORMAT_TOO_NEW";
pub const DAMAGED: &str = "DAMAGED";
pub const READ_ONLY: &str = "READ_ONLY";
pub const HOTKEY_CONFLICT: &str = "HOTKEY_CONFLICT";
pub const NO_SPACE: &str = "NO_SPACE";
pub const OFFLINE: &str = "OFFLINE";
pub const HASH_MISMATCH: &str = "HASH_MISMATCH";
pub const IO_ERROR: &str = "IO_ERROR";
pub const IN_USE: &str = "IN_USE";
pub const INVALID_SETTING: &str = "INVALID_SETTING";
pub const SERVER_ERROR: &str = "SERVER_ERROR";
pub const INVALID_PROJECT: &str = "INVALID_PROJECT";
pub const SIDECAR_START_FAILED: &str = "SIDECAR_START_FAILED";
pub const SIDECAR_CRASHED: &str = "SIDECAR_CRASHED";
pub const CANCELLED: &str = "CANCELLED";
pub const PATH_NOT_ALLOWED: &str = "PATH_NOT_ALLOWED";
pub const STAGE_FAILED: &str = "STAGE_FAILED";
pub const UNREADABLE_FILE: &str = "UNREADABLE_FILE";
pub const NO_AUDIO_TRACK: &str = "NO_AUDIO_TRACK";
pub const RUNTIME_MISSING: &str = "RUNTIME_MISSING";
pub const NO_SPEECH_FOUND: &str = "NO_SPEECH_FOUND";
pub const SAME_LANGUAGE: &str = "SAME_LANGUAGE";
pub const NO_FALLBACK_CHOSEN: &str = "NO_FALLBACK_CHOSEN";
pub const MODELS_MISSING: &str = "MODELS_MISSING";
pub const MACHINE_TOO_WEAK: &str = "MACHINE_TOO_WEAK";
pub const SOURCE_CHANGED: &str = "SOURCE_CHANGED";
pub const NOTHING_TO_RESUME: &str = "NOTHING_TO_RESUME";
pub const FILE_EXISTS: &str = "FILE_EXISTS";
pub const RUN_NOT_COMPLETE: &str = "RUN_NOT_COMPLETE";
pub const NO_DUB_AUDIO: &str = "NO_DUB_AUDIO";
pub const CLOUD_CONSENT_REQUIRED: &str = "CLOUD_CONSENT_REQUIRED";
pub const CLOUD_NO_KEY: &str = "CLOUD_NO_KEY";
pub const CLOUD_KEYS_EXHAUSTED: &str = "CLOUD_KEYS_EXHAUSTED";
pub const CLOUD_AUTH_FAILED: &str = "CLOUD_AUTH_FAILED";
pub const CLOUD_OFFLINE: &str = "CLOUD_OFFLINE";
pub const CLOUD_BAD_REPLY: &str = "CLOUD_BAD_REPLY";
pub const KEY_INVALID: &str = "KEY_INVALID";
pub const KEY_DUPLICATE: &str = "KEY_DUPLICATE";
pub const KEY_STORE_UNAVAILABLE: &str = "KEY_STORE_UNAVAILABLE";
pub const EDIT_END_BEFORE_START: &str = "EDIT_END_BEFORE_START";
pub const EDIT_OUT_OF_RANGE: &str = "EDIT_OUT_OF_RANGE";
pub const EDIT_CANNOT_MERGE: &str = "EDIT_CANNOT_MERGE";
pub const EDIT_NOTHING_TO_UNDO: &str = "EDIT_NOTHING_TO_UNDO";
pub const EDIT_NOT_FOUND: &str = "EDIT_NOT_FOUND";
pub const EDIT_WORK_FOLDER_MISSING: &str = "EDIT_WORK_FOLDER_MISSING";
pub const SUBTITLE_PARSE_FAILED: &str = "SUBTITLE_PARSE_FAILED";
pub const SUBTITLE_ENCODING_UNKNOWN: &str = "SUBTITLE_ENCODING_UNKNOWN";
pub const SUBTITLE_PREVIEW_EXPIRED: &str = "SUBTITLE_PREVIEW_EXPIRED";
pub const DUB_EDITS_WOULD_BE_LOST: &str = "DUB_EDITS_WOULD_BE_LOST";

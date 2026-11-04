use std::path::{Path, PathBuf};

use super::Settings;
use crate::error::AppError;
use crate::fsutil::write_atomic;

pub struct Loaded {
    pub settings: Settings,
    /// Message key to show once, for example when the file had to be replaced.
    pub notice: Option<&'static str>,
}

/// Load settings. A missing file means first run. An unreadable or corrupted file is moved aside
/// (`settings.json.corrupt`) and defaults are used, so the app always starts.
pub fn load(path: &Path) -> Loaded {
    let defaults = |notice| Loaded {
        settings: Settings::default(),
        notice,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return defaults(None),
        Err(_) => return defaults(Some("settings.corrupted")),
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => Loaded {
            settings,
            notice: None,
        },
        Err(_) => {
            let mut aside = path.as_os_str().to_owned();
            aside.push(".corrupt");
            let _ = std::fs::rename(path, PathBuf::from(aside));
            defaults(Some("settings.corrupted"))
        }
    }
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), AppError> {
    let json = serde_json::to_vec_pretty(settings).map_err(|e| AppError::io(e.into()))?;
    write_atomic(path, &json)
}

use serde::{Deserialize, Serialize};

use super::keys::{KeyBook, KeyState};
use super::services::{self, Step};
use crate::error::{AppError, CLOUD_NO_KEY, INVALID_SETTING};

/// The engine id that means "run on this computer".
pub const LOCAL: &str = "local";

/// Which engine does each heavy step: `local` or a cloud service id. The default is local for
/// both, and nothing ever switches a step to a cloud service by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Engines {
    pub translate: String,
    pub speak: String,
}

impl Default for Engines {
    fn default() -> Self {
        Self {
            translate: LOCAL.to_string(),
            speak: LOCAL.to_string(),
        }
    }
}

impl Engines {
    pub fn get(&self, step: Step) -> &str {
        match step {
            Step::Translate => &self.translate,
            Step::Speak => &self.speak,
        }
    }

    pub fn set(&mut self, step: Step, engine: &str) {
        match step {
            Step::Translate => self.translate = engine.to_string(),
            Step::Speak => self.speak = engine.to_string(),
        }
    }

    /// The cloud service id for a step, or `None` when the step runs locally.
    pub fn cloud(&self, step: Step) -> Option<&str> {
        Some(self.get(step)).filter(|e| *e != LOCAL)
    }
}

/// May this step be set to this engine? `local` always; a service only if it offers the step
/// and the user has a verified key for it.
pub fn validate_engine(book: &KeyBook, step: Step, engine: &str) -> Result<(), AppError> {
    if engine == LOCAL {
        return Ok(());
    }
    let service = services::find(book.services(), engine)
        .filter(|s| s.offers(step))
        .ok_or_else(|| AppError::new(INVALID_SETTING, "settings.error.invalid"))?;
    let verified = book
        .for_service(&service.id)?
        .iter()
        .any(|k| k.state == KeyState::Verified);
    if verified {
        Ok(())
    } else {
        Err(AppError::new(CLOUD_NO_KEY, "dub.error.cloudNoKey")
            .with_details(serde_json::json!({ "service": service.name })))
    }
}

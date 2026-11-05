use std::path::PathBuf;
use std::sync::Arc;

use super::adapter::CloudAdapter;
use super::clock::{Clock, RealClock, Sleeper};
use super::consent::ConsentBook;
use super::engine::CloudEngine;
use super::keys::KeyBook;
use super::services::{Service, Step};
use crate::error::AppError;

/// Gives the adapter for a service in the data file, or `None` when none is written for it.
pub type AdapterFactory = Arc<dyn Fn(&Service) -> Option<Arc<dyn CloudAdapter>> + Send + Sync>;

/// Everything a run needs to use a cloud service: the user's keys, their consent, the adapters
/// and the clock. `RunConfig::cloud` is `None` when the shell has none, which means every step
/// is local.
pub struct CloudRuntime {
    pub book: Arc<KeyBook>,
    pub consent: Arc<ConsentBook>,
    pub app_data: PathBuf,
    /// The adapter for a service. Tests return one pointed at a fake server.
    pub adapters: AdapterFactory,
    pub clock: Arc<dyn Clock>,
    pub sleeper: Arc<dyn Sleeper>,
}

impl CloudRuntime {
    /// The real adapters and real time.
    pub fn real(book: Arc<KeyBook>, consent: Arc<ConsentBook>, app_data: PathBuf) -> Self {
        let clock = Arc::new(RealClock::new());
        Self {
            book,
            consent,
            app_data,
            adapters: Arc::new(|s| super::adapters::adapter_for(s).map(Arc::from)),
            clock: clock.clone(),
            sleeper: clock,
        }
    }

    /// An engine for one step on one service, built from the user's verified keys.
    pub fn engine(&self, service_id: &str, step: Step) -> Result<CloudEngine, AppError> {
        let service = super::services::find(self.book.services(), service_id).ok_or_else(|| {
            AppError::new(crate::error::INVALID_SETTING, "settings.error.invalid")
        })?;
        let adapter = (self.adapters)(service).ok_or_else(|| {
            AppError::new(crate::error::INVALID_SETTING, "settings.error.invalid")
        })?;
        CloudEngine::from_book(
            &self.book,
            &self.app_data,
            service_id,
            step,
            adapter,
            self.clock.clone(),
            self.sleeper.clone(),
        )
    }
}

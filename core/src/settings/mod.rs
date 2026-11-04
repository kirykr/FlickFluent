//! User settings: theme, hotkeys, analytics choice and a few paths. Stored as one JSON file.

pub mod hotkeys;
mod model;
mod store;

pub use model::{General, Settings, Theme, SETTINGS_VERSION};
pub use store::{load, save, Loaded};

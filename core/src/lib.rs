//! FlickFluent core: all logic, no UI. The Tauri shell only wires this to the window.

pub mod cli;
pub mod error;
pub mod fsutil;
pub mod hardware;
pub mod memory;
pub mod models;
pub mod pipeline;
pub mod project;
pub mod settings;
pub mod sidecar;

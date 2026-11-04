//! The dub pipeline: sections, run state, stages. Orchestration only; model work happens in the
//! sidecar.

pub mod errors;
pub mod ffmpeg;
pub mod mix;
pub mod plan;
pub mod preflight;
pub mod run;
pub mod sections;
pub mod stages;
pub mod state;

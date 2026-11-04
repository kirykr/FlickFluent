//! Memory use for the status-bar badge: this app's own memory and the whole computer's.

mod sampler;

pub use sampler::{level, sample, Level, MemorySample, WARNING_PERCENT};

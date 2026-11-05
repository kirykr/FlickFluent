//! Pacing and estimates for `dub:progress`: at most one event a second, at least one every five
//! seconds while a stage runs, and an ETA from the sections already finished.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Progress events leave no more often than this.
pub const MIN_GAP: Duration = Duration::from_secs(1);
/// A stage that reports nothing still gets an event after this long. Under the contract's five
/// seconds so a late timer tick cannot break it.
pub const HEARTBEAT: Duration = Duration::from_secs(4);
/// How many finished sections the ETA averages over.
const WINDOW: usize = 5;

/// Decides when a progress event may leave. Asking and getting `true` counts as sending.
#[derive(Debug, Default)]
pub struct Pacer {
    last: Option<Instant>,
}

impl Pacer {
    /// The worker reported progress: send unless one went out less than a second ago.
    pub fn progress_due(&mut self, now: Instant) -> bool {
        self.due(now, MIN_GAP)
    }

    /// Nothing was reported: send a repeat of the last known progress once it is overdue.
    pub fn heartbeat_due(&mut self, now: Instant) -> bool {
        self.due(now, HEARTBEAT)
    }

    fn due(&mut self, now: Instant, gap: Duration) -> bool {
        let ok = self
            .last
            .is_none_or(|t| now.saturating_duration_since(t) >= gap);
        if ok {
            self.last = Some(now);
        }
        ok
    }
}

/// Remaining time from a rolling mean of finished sections.
#[derive(Debug, Default)]
pub struct Eta {
    window: VecDeque<Duration>,
    finished: usize,
}

impl Eta {
    /// A section went through every stage in `took`.
    pub fn section_done(&mut self, took: Duration) {
        if self.window.len() == WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(took);
        self.finished += 1;
    }

    /// Sections done before this run began (a resume) shorten the remainder but add no timing.
    pub fn already_done(&mut self, count: usize) {
        self.finished += count;
    }

    /// Remaining sections times the mean of the window. `None` until one section has finished.
    pub fn seconds(&self, section_count: usize) -> Option<u64> {
        if self.window.is_empty() {
            return None;
        }
        let mean = self.window.iter().sum::<Duration>().as_secs_f64() / self.window.len() as f64;
        let left = section_count.saturating_sub(self.finished) as f64;
        Some((mean * left).round() as u64)
    }
}

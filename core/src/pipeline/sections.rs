//! Plan sections: about 5 minutes each, cut at the quietest point within 20 s of the target.

use crate::project::dub::Section;

/// Target section length.
pub const TARGET_MS: u64 = 5 * 60 * 1000;
/// How far from the target a cut may move to land on a silence.
pub const WINDOW_MS: u64 = 20 * 1000;
/// A tail shorter than this is merged into the section before it.
const MIN_TAIL_MS: u64 = 10 * 1000;

/// A quiet stretch found by ffmpeg `silencedetect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Silence {
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Silence {
    fn mid(&self) -> u64 {
        (self.start_ms + self.end_ms) / 2
    }
}

/// Cut `total_ms` into sections. `silences` must be sorted by start.
pub fn plan(total_ms: u64, silences: &[Silence]) -> Vec<Section> {
    let mut cuts = Vec::new();
    let mut start = 0u64;
    while total_ms - start > TARGET_MS + MIN_TAIL_MS {
        let target = start + TARGET_MS;
        let cut = silences
            .iter()
            .map(Silence::mid)
            .filter(|m| m.abs_diff(target) <= WINDOW_MS && *m > start)
            .min_by_key(|m| m.abs_diff(target))
            .unwrap_or(target);
        cuts.push(cut);
        start = cut;
    }
    let mut sections = Vec::new();
    let mut from = 0;
    for (i, to) in cuts.into_iter().chain([total_ms]).enumerate() {
        sections.push(Section {
            index: i as u32,
            start_ms: from,
            end_ms: to,
            ..Default::default()
        });
        from = to;
    }
    sections
}

/// Parse the `silence_start` and `silence_end` lines that ffmpeg `silencedetect` prints.
pub fn parse_silencedetect(log: &str) -> Vec<Silence> {
    let mut out = Vec::new();
    let mut open: Option<f64> = None;
    for line in log.lines() {
        if let Some(v) = value_after(line, "silence_start:") {
            open = Some(v);
        } else if let Some(end) = value_after(line, "silence_end:") {
            if let Some(start) = open.take() {
                out.push(Silence {
                    start_ms: (start.max(0.0) * 1000.0) as u64,
                    end_ms: (end * 1000.0) as u64,
                });
            }
        }
    }
    out
}

fn value_after(line: &str, key: &str) -> Option<f64> {
    let rest = line.split(key).nth(1)?;
    rest.split_whitespace().next()?.parse().ok()
}

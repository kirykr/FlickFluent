//! Flags the editor recomputes after a change: `overlap` for a line and its neighbours, and
//! `tooLong` / `tooShort` for a line whose clip or times changed. Same rules as the run
//! (`pipeline::run::flags_for`, `window_ms`), so an edited line is judged as a made one.

use std::collections::BTreeSet;

use super::hash::spoken_text;
use crate::pipeline::run::{flags_for, window_ms};
use crate::project::dub::{LineFlag, TranscriptLine};

/// Recompute the flags of the lines in `dirty` (and `overlap` for the line after each).
pub fn refresh(lines: &mut [TranscriptLine], dirty: &BTreeSet<String>) {
    let mut recheck_overlap = BTreeSet::new();
    for (i, l) in lines.iter().enumerate() {
        if dirty.contains(&l.id) {
            recheck_overlap.insert(i);
            if i + 1 < lines.len() {
                recheck_overlap.insert(i + 1);
            }
        }
    }
    for i in recheck_overlap {
        let previous_end = i.checked_sub(1).map(|p| lines[p].end_ms);
        let overlaps = previous_end.is_some_and(|end| lines[i].start_ms < end);
        set(&mut lines[i], LineFlag::Overlap, overlaps);
    }
    for i in 0..lines.len() {
        if !dirty.contains(&lines[i].id) {
            continue;
        }
        let empty = spoken_text(&lines[i]).trim().is_empty();
        set(&mut lines[i], LineFlag::EmptyText, empty);
        refresh_fit(lines, i);
    }
}

/// `tooLong` against the line's window, `tooShort` against its slot, from the current clip length.
pub fn refresh_fit(lines: &mut [TranscriptLine], i: usize) {
    let window = window_ms(lines, i);
    let l = &mut lines[i];
    let confidence = if l.flags.contains(&LineFlag::LowConfidence) {
        0.0
    } else {
        1.0
    };
    let long = flags_for(l.audio_ms, l.start_ms, l.start_ms + window, confidence);
    let short = flags_for(l.audio_ms, l.start_ms, l.end_ms, confidence);
    set(l, LineFlag::TooLong, long.contains(&LineFlag::TooLong));
    set(l, LineFlag::TooShort, short.contains(&LineFlag::TooShort));
}

fn set(line: &mut TranscriptLine, flag: LineFlag, on: bool) {
    if on {
        line.flags.insert(flag);
    } else {
        line.flags.remove(&flag);
    }
}

//! Plain-text search over the lines (research R10). No regular expressions: they add a way to
//! freeze the app and are not in the spec. Offsets are UTF-16 code units, which is how the
//! window counts the characters of a string, so the UI can mark a match without converting.

use serde::{Deserialize, Serialize};

use crate::project::dub::TranscriptLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    Original,
    Translated,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchField {
    Original,
    Translated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    pub query: String,
    pub field: Field,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub whole_word: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub line_id: String,
    pub field: MatchField,
    /// UTF-16 offsets into the field's text, `start` inclusive, `end` exclusive.
    pub start: usize,
    pub end: usize,
}

fn fold(c: char, match_case: bool) -> char {
    if match_case {
        c
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every place `query` occurs in `text`, as UTF-16 `(start, end)`.
fn find_in(text: &str, query: &[char], match_case: bool, whole_word: bool) -> Vec<(usize, usize)> {
    let hay: Vec<(char, usize)> = {
        let mut at = 0;
        text.chars()
            .map(|c| {
                let here = at;
                at += c.len_utf16();
                (c, here)
            })
            .collect()
    };
    let total: usize = text.encode_utf16().count();
    let mut out = Vec::new();
    if query.is_empty() || hay.len() < query.len() {
        return out;
    }
    let mut i = 0;
    while i + query.len() <= hay.len() {
        let hit = query
            .iter()
            .enumerate()
            .all(|(k, q)| fold(hay[i + k].0, match_case) == *q);
        if hit {
            let before_ok = i == 0 || !is_word(hay[i - 1].0);
            let after = i + query.len();
            let after_ok = after == hay.len() || !is_word(hay[after].0);
            if !whole_word || (before_ok && after_ok) {
                let end = hay.get(after).map(|h| h.1).unwrap_or(total);
                out.push((hay[i].1, end));
                i = after; // matches do not overlap
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn find(lines: &[TranscriptLine], opts: &SearchOptions) -> Vec<Match> {
    let query: Vec<char> = opts
        .query
        .chars()
        .map(|c| fold(c, opts.match_case))
        .collect();
    let mut out = Vec::new();
    for line in lines {
        let mut scan = |field: MatchField, text: &str| {
            for (start, end) in find_in(text, &query, opts.match_case, opts.whole_word) {
                out.push(Match {
                    line_id: line.id.clone(),
                    field,
                    start,
                    end,
                });
            }
        };
        if matches!(opts.field, Field::Original | Field::Both) {
            scan(MatchField::Original, &line.text);
        }
        if matches!(opts.field, Field::Translated | Field::Both) {
            if let Some(t) = &line.translated {
                scan(MatchField::Translated, t);
            }
        }
    }
    out
}

/// Replace the given UTF-16 ranges of `text`, last first so earlier offsets stay true. A range
/// that is outside the text, or cuts a character in two, is skipped.
pub fn replace_ranges(text: &str, ranges: &[(usize, usize)], replacement: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut sorted: Vec<(usize, usize)> = ranges.to_vec();
    sorted.sort();
    sorted.dedup();
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let repl: Vec<u16> = replacement.encode_utf16().collect();
    let mut at = 0;
    for (s, e) in sorted {
        if s < at || e < s || e > units.len() {
            continue; // overlaps the previous range, or lies outside the text
        }
        out.extend_from_slice(&units[at..s]);
        out.extend_from_slice(&repl);
        at = e;
    }
    out.extend_from_slice(&units[at..]);
    String::from_utf16_lossy(&out)
}

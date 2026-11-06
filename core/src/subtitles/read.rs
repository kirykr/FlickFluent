//! Read SRT and WebVTT (`contracts/subtitle-formats.md`): the encoding, then the cues. Tags and
//! positioning marks are dropped; a cue's rows are joined with a space; cues come back sorted.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, SUBTITLE_ENCODING_UNKNOWN, SUBTITLE_PARSE_FAILED};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleFormat {
    Srt,
    Vtt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleFile {
    pub format: SubtitleFormat,
    /// The name of the encoding that was used to read the file.
    pub encoding: String,
    pub cues: Vec<SubtitleCue>,
}

fn failed(cue: usize, line: usize, reason: &str) -> AppError {
    AppError::new(SUBTITLE_PARSE_FAILED, "editor.error.subtitleParseFailed").with_details(json!({
        "cue": cue,
        "line": line,
        "reason": reason,
        "detail": if cue > 0 { format!("cue {cue} (line {line}): {reason}") } else { reason.to_string() },
    }))
}

/// The text of the file: a byte order mark decides, then UTF-8, then a guess that is named.
fn decode(bytes: &[u8]) -> Result<(String, String), AppError> {
    if let Some((enc, skip)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, bad) = enc.decode(&bytes[skip..]);
        if bad {
            return Err(AppError::new(
                SUBTITLE_ENCODING_UNKNOWN,
                "editor.error.subtitleEncodingUnknown",
            ));
        }
        return Ok((text.into_owned(), enc.name().to_string()));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_string(), "UTF-8".to_string()));
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let enc = detector.guess(None, chardetng::Utf8Detection::Deny);
    let (text, _, bad) = enc.decode(bytes);
    if bad {
        return Err(AppError::new(
            SUBTITLE_ENCODING_UNKNOWN,
            "editor.error.subtitleEncodingUnknown",
        ));
    }
    Ok((text.into_owned(), enc.name().to_string()))
}

fn time_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(\d+):)?(\d{1,2}):(\d{2})[,.](\d{1,3})$").expect("valid pattern")
    })
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]*>|\{\\[^}]*\}").expect("valid pattern"))
}

fn parse_time(s: &str) -> Option<u64> {
    let c = time_re().captures(s.trim())?;
    let num = |i: usize| {
        c.get(i)
            .map_or(0, |m| m.as_str().parse::<u64>().unwrap_or(0))
    };
    let frac = c.get(4)?.as_str();
    let ms: u64 = format!("{frac:0<3}").parse().ok()?;
    Some(((num(1) * 60 + num(2)) * 60 + num(3)) * 1000 + ms)
}

fn clean(text: &str, vtt: bool) -> String {
    let no_tags = tag_re().replace_all(text, "");
    let decoded = if vtt {
        no_tags
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
    } else {
        no_tags.into_owned()
    };
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse the bytes of a subtitle file.
pub fn parse_bytes(bytes: &[u8]) -> Result<SubtitleFile, AppError> {
    let (text, encoding) = decode(bytes)?;
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let format = if text.trim_start().starts_with("WEBVTT") {
        SubtitleFormat::Vtt
    } else {
        SubtitleFormat::Srt
    };
    let vtt = format == SubtitleFormat::Vtt;

    let mut cues = Vec::new();
    let mut line_no = 0usize;
    let mut block: Vec<(usize, &str)> = Vec::new();
    let mut blocks: Vec<Vec<(usize, &str)>> = Vec::new();
    for raw in text.lines() {
        line_no += 1;
        if raw.trim().is_empty() {
            if !block.is_empty() {
                blocks.push(std::mem::take(&mut block));
            }
        } else {
            block.push((line_no, raw));
        }
    }
    if !block.is_empty() {
        blocks.push(block);
    }

    for (n, b) in blocks.iter().enumerate() {
        let first = b[0].1.trim_start_matches('\u{feff}').trim();
        if vtt && n == 0 {
            continue; // the header
        }
        if vtt
            && ["NOTE", "STYLE", "REGION"].iter().any(|k| {
                first == *k
                    || first.starts_with(&format!("{k} "))
                    || first.starts_with(&format!("{k}\t"))
            })
        {
            continue;
        }
        let Some(at) = b.iter().position(|(_, l)| l.contains("-->")) else {
            continue; // an index with nothing under it, or text that is not a cue
        };
        let cue_no = cues.len() + 1;
        let (line, stamp) = b[at];
        let (a, rest) = stamp.split_once("-->").expect("checked");
        let b_part = rest.split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (parse_time(a), parse_time(b_part)) else {
            return Err(failed(cue_no, line, "the time is not readable"));
        };
        if end < start {
            return Err(failed(cue_no, line, "the end is before the start"));
        }
        let body: Vec<&str> = b[at + 1..].iter().map(|(_, l)| *l).collect();
        cues.push(SubtitleCue {
            start_ms: start,
            end_ms: end,
            text: clean(&body.join(" "), vtt),
        });
    }
    if cues.is_empty() {
        return Err(failed(0, 0, "no cue was found in this file"));
    }
    cues.sort_by_key(|c| c.start_ms);
    Ok(SubtitleFile {
        format,
        encoding,
        cues,
    })
}

/// Read a subtitle file from disk.
pub fn read_file(path: &std::path::Path) -> Result<SubtitleFile, AppError> {
    parse_bytes(&std::fs::read(path).map_err(AppError::io)?)
}

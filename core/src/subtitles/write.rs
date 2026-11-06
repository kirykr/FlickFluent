//! SRT and WebVTT writers (`contracts/subtitle-formats.md`). Times are the transcript's
//! `startMs` and `endMs`, never `dubStartMs`. Lines with no text are skipped.

use serde::{Deserialize, Serialize};

use super::templates::{css_colour, CaptionTemplate, Position};
use crate::project::dub::TranscriptLine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Original,
    Translated,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Srt,
    Vtt,
}

pub fn srt_time(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

pub fn vtt_time(ms: u64) -> String {
    srt_time(ms).replace(',', ".")
}

fn one_row(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text of a cue, or `None` when the line has nothing to show. A translation that is empty
/// falls back to the original, as the run's own subtitles do.
fn cue_text(l: &TranscriptLine, source: Source) -> Option<String> {
    let original = one_row(&l.text);
    let translated = l.translated.as_deref().map(one_row).unwrap_or_default();
    let text = match source {
        Source::Original => original,
        Source::Translated if translated.is_empty() => original,
        Source::Translated => translated,
        Source::Both if translated.is_empty() => original,
        Source::Both if original.is_empty() => translated,
        Source::Both => format!("{original}\n{translated}"),
    };
    (!text.is_empty()).then_some(text)
}

pub fn srt_text(lines: &[TranscriptLine], source: Source) -> String {
    let mut out = String::new();
    let mut n = 0;
    for l in lines {
        let Some(text) = cue_text(l, source) else {
            continue;
        };
        n += 1;
        out.push_str(&format!(
            "{n}\n{} --> {}\n{text}\n\n",
            srt_time(l.start_ms),
            srt_time(l.end_ms)
        ));
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn style_block(t: &CaptionTemplate) -> String {
    let mut css = vec![
        format!("  font-family: {};", t.font_family),
        format!("  color: {};", css_colour(&t.color)),
        format!("  font-size: {}vh;", t.size_percent),
        format!(
            "  background-color: {};",
            t.background
                .as_deref()
                .map(css_colour)
                .unwrap_or_else(|| "transparent".into())
        ),
    ];
    if t.outline_width > 0.0 {
        let (w, c) = (t.outline_width, css_colour(&t.outline_color));
        css.push(format!(
            "  text-shadow: -{w}px -{w}px 0 {c}, {w}px -{w}px 0 {c}, -{w}px {w}px 0 {c}, {w}px {w}px 0 {c};"
        ));
    }
    format!("STYLE\n::cue {{\n{}\n}}\n\n", css.join("\n"))
}

pub fn vtt_text(lines: &[TranscriptLine], source: Source, template: &CaptionTemplate) -> String {
    let row = match template.position {
        Position::Bottom => 100.0 - template.margin_percent,
        Position::Top => template.margin_percent,
    };
    let mut out = format!("WEBVTT\n\n{}", style_block(template));
    for l in lines {
        let Some(text) = cue_text(l, source) else {
            continue;
        };
        out.push_str(&format!(
            "{} --> {} line:{row}% position:50% align:center\n{}\n\n",
            vtt_time(l.start_ms),
            vtt_time(l.end_ms),
            escape(&text)
        ));
    }
    out
}

/// How many cues a file of these lines would have.
pub fn cue_count(lines: &[TranscriptLine], source: Source) -> usize {
    lines
        .iter()
        .filter(|l| cue_text(l, source).is_some())
        .count()
}

/// Write the lines as a subtitle file, through a temporary file so a stop leaves nothing half
/// written. WebVTT uses `template` (the classic one when none is given). Returns the cue count.
pub fn export_file(
    lines: &[TranscriptLine],
    source: Source,
    format: Format,
    path: &std::path::Path,
    template: Option<&CaptionTemplate>,
) -> Result<usize, crate::error::AppError> {
    let text = match format {
        Format::Srt => srt_text(lines, source),
        Format::Vtt => {
            let fallback;
            let t = match template {
                Some(t) => t,
                None => {
                    fallback = super::templates::find("classic").ok_or_else(|| {
                        crate::error::AppError::new(
                            crate::error::INVALID_SETTING,
                            "settings.error.invalid",
                        )
                    })?;
                    &fallback
                }
            };
            vtt_text(lines, source, t)
        }
    };
    crate::fsutil::write_atomic(path, text.as_bytes())?;
    Ok(cue_count(lines, source))
}

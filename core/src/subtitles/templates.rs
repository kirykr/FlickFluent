//! Caption templates from `config/caption-templates.json` (`contracts/caption-templates.md`).
//! The file is checked when it is loaded; adding a template is a data change only.

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, INVALID_SETTING};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Bottom,
    Top,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionTemplate {
    pub id: String,
    pub name_key: String,
    pub font_family: String,
    /// Text height as a share of the video height.
    pub size_percent: f64,
    pub color: String,
    pub outline_color: String,
    pub outline_width: f64,
    pub background: Option<String>,
    pub position: Position,
    pub margin_percent: f64,
}

/// Fonts every list must end up with, so Khmer shows on both systems (research R19).
const KHMER: [&str; 3] = ["Noto Sans Khmer", "Khmer UI", "Khmer Sangam MN"];

fn bad(detail: String) -> AppError {
    AppError::new(INVALID_SETTING, "settings.error.invalid")
        .with_details(serde_json::json!({ "detail": detail }))
}

/// Read a template file and refuse anything the contract forbids.
pub fn validate(json: &str) -> Result<Vec<CaptionTemplate>, AppError> {
    let all: Vec<CaptionTemplate> =
        serde_json::from_str(json).map_err(|e| bad(format!("caption templates: {e}")))?;
    let colour = Regex::new(r"^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$").expect("valid pattern");
    for t in &all {
        let id = &t.id;
        if t.id.is_empty() || t.name_key.is_empty() {
            return Err(bad(format!("{id}: id and nameKey are required")));
        }
        for name in KHMER {
            if !t.font_family.contains(name) {
                return Err(bad(format!("{id}: fontFamily must name {name}")));
            }
        }
        for (field, value) in [
            ("color", Some(&t.color)),
            ("outlineColor", Some(&t.outline_color)),
            ("background", t.background.as_ref()),
        ] {
            if let Some(v) = value {
                if !colour.is_match(v) {
                    return Err(bad(format!("{id}: {field} must be #rrggbb or #rrggbbaa")));
                }
            }
        }
        if !(2.0..=12.0).contains(&t.size_percent) {
            return Err(bad(format!("{id}: sizePercent must be 2 to 12")));
        }
        if t.outline_width < 0.0 || !(0.0..=50.0).contains(&t.margin_percent) {
            return Err(bad(format!(
                "{id}: outlineWidth or marginPercent is out of range"
            )));
        }
    }
    Ok(all)
}

/// The templates that ship with the app, in the order shown to the user.
pub fn bundled() -> Result<Vec<CaptionTemplate>, AppError> {
    validate(include_str!("../../../config/caption-templates.json"))
}

pub fn find(id: &str) -> Option<CaptionTemplate> {
    bundled().ok()?.into_iter().find(|t| t.id == id)
}

/// `#rrggbbaa` as `rgba(r, g, b, a)` for CSS; `#rrggbb` stays as it is.
pub fn css_colour(c: &str) -> String {
    if c.len() == 9 {
        let n = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).unwrap_or(0);
        format!(
            "rgba({}, {}, {}, {:.2})",
            n(1),
            n(3),
            n(5),
            n(7) as f64 / 255.0
        )
    } else {
        c.to_string()
    }
}

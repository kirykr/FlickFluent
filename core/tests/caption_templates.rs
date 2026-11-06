//! The caption template file (`contracts/caption-templates.md`) is checked when it is loaded.

use std::path::Path;

use flickfluent_core::subtitles::templates::{bundled, validate};
use serde_json::{json, Value};

fn good() -> Value {
    json!({
        "id": "t", "nameKey": "editor.captions.classic",
        "fontFamily": "Inter, \"Noto Sans Khmer\", \"Khmer UI\", \"Khmer Sangam MN\", sans-serif",
        "sizePercent": 5, "color": "#ffffff", "outlineColor": "#000000", "outlineWidth": 2,
        "background": null, "position": "bottom", "marginPercent": 6
    })
}

fn check(v: Value) -> bool {
    validate(&serde_json::to_string(&json!([v])).unwrap()).is_ok()
}

#[test]
fn the_shipped_file_has_three_templates_that_validate() {
    let all = bundled().unwrap();
    assert_eq!(
        all.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        vec!["classic", "soft-box", "top-note"]
    );
    assert_eq!(all[1].background.as_deref(), Some("#000000b3"));
    assert_eq!(all[2].color, "#ffd84a");
}

#[test]
fn every_name_key_exists_in_the_language_file() {
    let en: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/en.json"))
            .unwrap(),
    )
    .unwrap();
    for t in bundled().unwrap() {
        let found = t.name_key.split('.').try_fold(&en, |n, p| n.get(p));
        assert!(found.is_some_and(Value::is_string), "{}", t.name_key);
    }
}

#[test]
fn every_font_list_names_the_khmer_fallbacks() {
    for t in bundled().unwrap() {
        for name in ["Noto Sans Khmer", "Khmer UI", "Khmer Sangam MN"] {
            assert!(t.font_family.contains(name), "{}: {name}", t.id);
        }
    }
    let mut v = good();
    v["fontFamily"] = json!("Inter, sans-serif");
    assert!(!check(v));
}

#[test]
fn a_missing_field_a_bad_colour_or_a_size_out_of_range_is_refused() {
    assert!(check(good()));
    let mut v = good();
    v.as_object_mut().unwrap().remove("position");
    assert!(!check(v));
    for (field, bad) in [
        ("color", json!("white")),
        ("color", json!("#fff")),
        ("outlineColor", json!("#12345g")),
        ("background", json!("#12345")),
        ("sizePercent", json!(1.9)),
        ("sizePercent", json!(12.5)),
        ("position", json!("middle")),
    ] {
        let mut v = good();
        v[field] = bad.clone();
        assert!(!check(v), "{field} = {bad}");
    }
    let mut v = good();
    v["background"] = json!("#000000b3");
    v["color"] = json!("#ffffffff");
    assert!(check(v), "eight-digit colours are fine");
}

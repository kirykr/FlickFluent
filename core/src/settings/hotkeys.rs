//! Hotkey combos as text, like `Mod+Shift+S`. `Mod` is Ctrl on Windows and Cmd on macOS, so one
//! default works on both platforms.

/// Actions that can have a hotkey, with their defaults.
pub const DEFAULTS: [(&str, &str); 4] = [
    ("project.new", "Mod+N"),
    ("project.open", "Mod+O"),
    ("project.save", "Mod+S"),
    ("project.close", "Mod+W"),
];

const MODIFIERS: [&str; 4] = ["Mod", "Ctrl", "Alt", "Shift"];
const NAMED_KEYS: [&str; 10] = [
    "Enter",
    "Escape",
    "Tab",
    "Space",
    "Backspace",
    "Delete",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
];

pub fn is_known_action(action: &str) -> bool {
    DEFAULTS.iter().any(|(a, _)| *a == action)
}

/// Canonical form: modifiers in the order Mod, Ctrl, Alt, Shift, then the key. `None` if the
/// text is not a usable combo (no modifier, unknown key, empty part).
pub fn normalize(combo: &str) -> Option<String> {
    let parts: Vec<&str> = combo.split('+').map(str::trim).collect();
    let (key, mods) = parts.split_last()?;
    if mods.is_empty() || key.is_empty() {
        return None;
    }

    let mut found = [false; 4];
    for m in mods {
        let i = MODIFIERS.iter().position(|c| c.eq_ignore_ascii_case(m))?;
        found[i] = true;
    }

    let key = canonical_key(key)?;
    let mut out: Vec<String> = MODIFIERS
        .iter()
        .zip(found)
        .filter(|(_, on)| *on)
        .map(|(m, _)| (*m).to_string())
        .collect();
    out.push(key);
    Some(out.join("+"))
}

fn canonical_key(key: &str) -> Option<String> {
    if key.chars().count() == 1 {
        return Some(key.to_uppercase());
    }
    if let Some(named) = NAMED_KEYS.iter().find(|n| n.eq_ignore_ascii_case(key)) {
        return Some((*named).to_string());
    }
    // F1 to F12
    let digits = key.strip_prefix(['f', 'F'])?;
    let n: u8 = digits.parse().ok()?;
    (1..=12).contains(&n).then(|| format!("F{n}"))
}

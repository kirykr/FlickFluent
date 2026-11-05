use base64::Engine;
use zeroize::Zeroizing;

/// A key the user owns. It has no `Serialize`, and `Debug` and `Display` print `[secret]`, so it
/// cannot reach a file, a log or an error message by accident. The memory is cleared on drop.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// The value, only for building a request header or storing it in the OS secure storage.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[secret]")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[secret]")
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Replace every form of each secret in `text` (as is, URL-encoded, base64) with `[key]`.
/// Use it on any text that came from a service before it is shown or logged.
pub fn scrub(text: &str, secrets: &[&Secret]) -> String {
    let mut forms: Vec<String> = Vec::new();
    for s in secrets {
        let plain = s.expose();
        if plain.is_empty() {
            continue;
        }
        forms.push(plain.to_string());
        forms.push(percent_encode(plain));
        forms.push(base64::engine::general_purpose::STANDARD.encode(plain));
    }
    // Longest first, so a form that contains another is replaced whole.
    forms.sort_by_key(|f| std::cmp::Reverse(f.len()));
    forms.dedup();
    let mut out = text.to_string();
    for f in forms {
        out = out.replace(&f, "[key]");
    }
    out
}

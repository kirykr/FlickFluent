//! A key is held in a wrapper that never prints itself, and `scrub` removes every form of it.

use base64::Engine;
use flickfluent_core::cloud::secret::{scrub, Secret};

const KEY: &str = "AIza-Secret/key+1=2 3";

#[test]
fn formatting_never_shows_the_value() {
    let s = Secret::new(KEY.to_string());
    assert_eq!(format!("{s}"), "[secret]");
    assert_eq!(format!("{s:?}"), "[secret]");
    assert_eq!(format!("{:?}", Some(&s)), "Some([secret])");
}

#[test]
fn expose_gives_the_value_for_a_header() {
    assert_eq!(Secret::new(KEY.to_string()).expose(), KEY);
}

#[test]
fn scrub_replaces_the_plain_url_encoded_and_base64_forms() {
    let s = Secret::new(KEY.to_string());
    let url_form = "AIza-Secret%2Fkey%2B1%3D2%203";
    let b64 = base64::engine::general_purpose::STANDARD.encode(KEY);
    let text = format!("bad {KEY} then {url_form} then {b64} and {KEY} again");
    let out = scrub(&text, &[&s]);
    assert_eq!(out, "bad [key] then [key] then [key] and [key] again");
    assert!(!out.contains("Secret"));
}

#[test]
fn scrub_handles_several_secrets_and_empty_ones() {
    let a = Secret::new("alpha-1".to_string());
    let b = Secret::new("beta-2".to_string());
    let empty = Secret::new(String::new());
    assert_eq!(
        scrub("alpha-1 beta-2 gamma", &[&a, &b, &empty]),
        "[key] [key] gamma"
    );
}

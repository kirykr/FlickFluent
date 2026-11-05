//! The amount shown before a cloud run is an estimate from the video's length (research R6).

use flickfluent_core::cloud::estimate::{estimate_chars, round_for_display, CHARS_PER_SECOND};

#[test]
fn the_estimate_is_duration_seconds_times_fifteen() {
    assert_eq!(CHARS_PER_SECOND, 15);
    assert_eq!(estimate_chars(60_000), 900);
    assert_eq!(estimate_chars(1_000), 15);
    assert_eq!(estimate_chars(0), 0);
    assert_eq!(estimate_chars(90 * 60 * 1000), 81_000);
}

#[test]
fn display_rounds_to_the_nearest_hundred() {
    assert_eq!(round_for_display(1_234), 1_200);
    assert_eq!(round_for_display(1_250), 1_300);
    assert_eq!(round_for_display(81_000), 81_000);
    assert_eq!(round_for_display(49), 0);
}

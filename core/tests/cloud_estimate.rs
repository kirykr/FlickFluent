//! The amount shown before a cloud run is an estimate from the video's length (research R6).

use flickfluent_core::cloud::estimate::{estimate_chars, round_for_display, CHARS_PER_SECOND};

#[test]
fn the_estimate_is_duration_seconds_times_ten() {
    assert_eq!(CHARS_PER_SECOND, 10);
    assert_eq!(estimate_chars(60_000), 600);
    assert_eq!(estimate_chars(1_000), 10);
    assert_eq!(estimate_chars(0), 0);
    assert_eq!(estimate_chars(90 * 60 * 1000), 54_000);
}

#[test]
fn display_rounds_to_the_nearest_hundred() {
    assert_eq!(round_for_display(1_234), 1_200);
    assert_eq!(round_for_display(1_250), 1_300);
    assert_eq!(round_for_display(54_000), 54_000);
    assert_eq!(round_for_display(49), 0);
}

#[test]
fn a_projection_scales_what_was_sent_to_the_whole_video_and_never_goes_below_it() {
    use flickfluent_core::cloud::estimate::project_chars;
    assert_eq!(project_chars(300, 20_000, 120_000), 1_800);
    assert_eq!(project_chars(300, 0, 120_000), 300);
    assert_eq!(project_chars(300, 120_000, 20_000), 300);
}

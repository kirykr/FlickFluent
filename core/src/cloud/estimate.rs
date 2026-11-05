/// Characters of text a cloud step sends per second of video. The transcript does not exist
/// before the run starts, so the amount shown beforehand is an estimate (research R6); the real
/// count is recorded after the run. Checked against the French fixtures in task T066.
pub const CHARS_PER_SECOND: u64 = 15;

/// About how many characters one cloud step will send for a video of this length.
pub fn estimate_chars(duration_ms: u64) -> u64 {
    duration_ms * CHARS_PER_SECOND / 1000
}

/// The amount as shown to the user: "about N" with N to the nearest hundred.
pub fn round_for_display(chars: u64) -> u64 {
    (chars + 50) / 100 * 100
}

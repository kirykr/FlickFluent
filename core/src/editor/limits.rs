//! The numbers the editor enforces, in one place. See `specs/004-transcript-editor-timeline/data-model.md`.

/// Speed range of the post chain. The top equals what the cloud engine may speed a clip up by.
pub const SPEED_MIN: f64 = 0.85;
pub const SPEED_MAX: f64 = crate::cloud::engine::MAX_SPEEDUP;
pub const VOLUME_MIN_DB: f64 = -12.0;
pub const VOLUME_MAX_DB: f64 = 6.0;
pub const PITCH_MIN: f64 = -4.0;
pub const PITCH_MAX: f64 = 4.0;
/// A moved clip stops this far short of the one before it.
pub const NEIGHBOUR_MARGIN_MS: u64 = 80;

pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

pub fn in_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

pub fn speed_ok(v: f64) -> bool {
    in_range(v, SPEED_MIN, SPEED_MAX)
}

pub fn volume_ok(v: f64) -> bool {
    in_range(v, VOLUME_MIN_DB, VOLUME_MAX_DB)
}

pub fn pitch_ok(v: f64) -> bool {
    in_range(v, PITCH_MIN, PITCH_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_match_the_spec() {
        assert_eq!(SPEED_MAX, 1.3);
        assert!(speed_ok(0.85) && speed_ok(1.3) && !speed_ok(0.84) && !speed_ok(1.31));
        assert!(volume_ok(-12.0) && volume_ok(6.0) && !volume_ok(6.5));
        assert!(pitch_ok(-4.0) && pitch_ok(4.0) && !pitch_ok(f64::NAN));
        assert_eq!(clamp(2.0, SPEED_MIN, SPEED_MAX), 1.3);
    }
}

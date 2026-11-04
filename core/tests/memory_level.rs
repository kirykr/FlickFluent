use flickfluent_core::memory::{level, sample, Level, WARNING_PERCENT};

const GB: u64 = 1024 * 1024 * 1024;

#[test]
fn warning_starts_at_85_percent_of_system_memory() {
    assert_eq!(WARNING_PERCENT, 85);
    assert_eq!(level(84 * GB, 100 * GB), Level::Normal);
    assert_eq!(level(8_499, 10_000), Level::Normal);
    assert_eq!(level(8_500, 10_000), Level::Warning);
    assert_eq!(level(100 * GB, 100 * GB), Level::Warning);
}

#[test]
fn unreadable_memory_is_normal_not_a_crash() {
    assert_eq!(level(0, 0), Level::Normal);
    assert_eq!(level(5, 0), Level::Normal);
}

#[test]
fn a_real_sample_is_sensible() {
    let s = sample();
    assert!(s.system_total_bytes > 0);
    assert!(s.system_used_bytes <= s.system_total_bytes);
    assert!(s.app_bytes > 0, "this test process uses some memory");
    assert_eq!(s.level, level(s.system_used_bytes, s.system_total_bytes));
}

#[test]
fn the_sample_serialises_with_the_names_the_ui_expects() {
    let json = serde_json::to_value(sample()).unwrap();
    for key in ["appBytes", "systemUsedBytes", "systemTotalBytes", "level"] {
        assert!(json.get(key).is_some(), "missing {key}");
    }
    assert!(matches!(json["level"].as_str(), Some("normal" | "warning")));
}

//! The section planner cuts about every 5 minutes at the quietest point nearby.

use flickfluent_core::pipeline::sections::{plan, Silence, TARGET_MS, WINDOW_MS};

fn silence(mid_ms: u64) -> Silence {
    Silence {
        start_ms: mid_ms.saturating_sub(200),
        end_ms: mid_ms + 200,
    }
}

#[test]
fn short_file_is_one_section() {
    let s = plan(120_000, &[]);
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].start_ms, s[0].end_ms), (0, 120_000));
}

#[test]
fn cuts_at_the_silence_nearest_the_target_within_the_window() {
    let silences = [silence(TARGET_MS - 8_000), silence(TARGET_MS + 3_000)];
    let s = plan(700_000, &silences);
    assert_eq!(s[0].end_ms, TARGET_MS + 3_000, "nearest to target wins");
    assert_eq!(s[1].start_ms, s[0].end_ms);
}

#[test]
fn no_silence_in_window_cuts_at_the_target() {
    let s = plan(700_000, &[silence(TARGET_MS + WINDOW_MS + 5_000)]);
    assert_eq!(s[0].end_ms, TARGET_MS);
}

#[test]
fn sections_cover_the_file_without_gap_or_overlap() {
    let silences: Vec<Silence> = (1..40).map(|i| silence(i * 97_000)).collect();
    let total = 3 * 60 * 60 * 1000;
    let s = plan(total, &silences);
    assert_eq!(s.first().unwrap().start_ms, 0);
    assert_eq!(s.last().unwrap().end_ms, total);
    for pair in s.windows(2) {
        assert_eq!(pair[0].end_ms, pair[1].start_ms);
    }
    for (i, sec) in s.iter().enumerate() {
        assert_eq!(sec.index as usize, i);
        assert!(sec.end_ms > sec.start_ms);
        // Never longer than target plus window, so memory stays bounded.
        assert!(sec.end_ms - sec.start_ms <= TARGET_MS + WINDOW_MS);
    }
}

#[test]
fn a_file_that_is_all_silence_still_plans() {
    let s = plan(
        600_000,
        &[Silence {
            start_ms: 0,
            end_ms: 600_000,
        }],
    );
    assert!(s.iter().all(|x| x.end_ms > x.start_ms));
    assert_eq!(s.last().unwrap().end_ms, 600_000);
}

#[test]
fn a_tiny_tail_is_merged_into_the_last_section() {
    let s = plan(TARGET_MS + 2_000, &[]);
    assert_eq!(s.len(), 1);
}

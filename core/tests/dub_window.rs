use flickfluent_core::pipeline::run::window_ms;
use flickfluent_core::project::dub::{LineStatus, TranscriptLine};

fn line(start_ms: u64, end_ms: u64) -> TranscriptLine {
    TranscriptLine {
        id: format!("l{start_ms}"),
        section_index: 0,
        start_ms,
        end_ms,
        speaker_id: "spk1".into(),
        language: "fr".into(),
        text: String::new(),
        translated: None,
        audio: None,
        audio_ms: None,
        flags: Default::default(),
        status: LineStatus::Transcribed,
        ..Default::default()
    }
}

#[test]
fn a_line_may_use_the_silence_after_it_but_not_the_next_line() {
    let lines = [line(0, 1000), line(1500, 2500), line(9000, 9500)];
    // 500 ms gap, minus the 80 ms margin.
    assert_eq!(window_ms(&lines, 0), 1420);
    // A 60 ms gap is below the margin: the slot only.
    let tight = [line(0, 1000), line(1060, 2000)];
    assert_eq!(window_ms(&tight, 0), 1000);
    let far = [line(0, 1000), line(9000, 9500)];
    assert_eq!(window_ms(&far, 0), 1600);
    // The last line has no next line to hit: capped gap less the margin.
    assert_eq!(window_ms(&lines, 2), 1020);
}

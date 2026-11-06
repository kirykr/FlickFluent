use std::path::Path;

/// Length in milliseconds of a PCM WAV file, read from its header. `None` for anything else.
pub fn wav_duration_ms(path: &Path) -> Option<u64> {
    wav_duration_ms_of(&std::fs::read(path).ok()?)
}

pub fn wav_duration_ms_of(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut rate, mut channels, mut bits) = (0u64, 0u64, 0u64);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = at + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            channels = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().ok()?) as u64;
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?) as u64;
            bits = u16::from_le_bytes(bytes[body + 14..body + 16].try_into().ok()?) as u64;
        } else if id == b"data" {
            // A streamed file may claim a size past the end; trust what is there.
            let size = size.min(bytes.len().saturating_sub(body)) as u64;
            let per_second = rate * channels * bits / 8;
            return (per_second > 0).then(|| size * 1000 / per_second);
        }
        at = body + size + (size & 1);
    }
    None
}

/// Sample rate of a PCM WAV file, read from its header. `None` for anything else.
pub fn wav_sample_rate(path: &Path) -> Option<u32> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = at + 8;
        if &bytes[at..at + 4] == b"fmt " && body + 8 <= bytes.len() {
            return Some(u32::from_le_bytes(
                bytes[body + 4..body + 8].try_into().ok()?,
            ));
        }
        at = body + size + (size & 1);
    }
    None
}

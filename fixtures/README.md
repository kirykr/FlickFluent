# Fixtures

Small sample clips for the stage contract tests. All audio is self-made with macOS `say` voices and `ffmpeg` sine tones (`make-fixtures.sh`), so no third-party licence applies. The files are committed so Windows runs use the same input.

| File | Length | Use |
|------|--------|-----|
| `clip-20s-fr.mp4` | 20 s | French speech from two voices over a tone bed. Main contract-test clip. Reference text: `expected/clip-20s-fr.json` |
| `clip-2min-fr.mp4` | 120 s | The 20 s clip looped 6 times. End-to-end and benchmark clip |
| `clip-music-only.mp4` | 10 s | Tones only. Separation must give near-silent vocals |
| `clip-no-speech.mp4` | 10 s | Silence. Must give zero lines and `NO_SPEECH_FOUND` |
| `clip-no-audio.mp4` | 5 s | Video stream only. Must give `NO_AUDIO_TRACK` |
| `clip-corrupt.mp4` | 4 KB | Random bytes. Must give `UNREADABLE_FILE` |

`make-long.sh` / `make-long.ps1` build a 3-hour file by looping the 2-minute clip (generated, not committed).

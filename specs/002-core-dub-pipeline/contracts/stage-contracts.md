# Contract: Pipeline stages

Each stage has a contract test on `fixtures/clip-20s-*` written before the stage code (Principle VIII). A stage is done when its test passes on Windows and macOS. Inputs and outputs live in the work folder `<name>.flick-work/`.

| # | Stage | Input | Output | Granularity | Pass test (on the 20 s fixture) |
|---|-------|-------|--------|-------------|--------------------------------|
| 1 | extract | source video | `audio.wav` (mono 16 kHz for ASR copy) and `audio-full.wav` (stereo 44.1 kHz for mixing), `source-info.json` | whole file, streamed | duration within 50 ms of source; error `NO_AUDIO_TRACK` for a silent-video fixture; corrupt fixture gives `UNREADABLE_FILE` |
| 2 | separate | `audio-full.wav` window | `vocals.wav`, `music.wav` per section | per section | `vocals + music` within tolerance of input; music-only fixture yields near-silent vocals; lengths equal |
| 3 | transcribe | `vocals.wav` | lines with times and speaker ids per section; detected language | per section | expected words from the fixture's reference text (word error rate under a set limit); no line `endMs <= startMs`; lines sorted; speakers within 1 of the fixture's true count; speech-free fixture gives zero lines and `NO_SPEECH_FOUND` |
| 4 | translate | lines of a section | `translated` per line | per batch | fixture pair gives non-empty text, same line count and order; preserved numbers and names in the reference; unsupported pair is rejected before the stage starts |
| 5 | synthesize | translated lines + voice per speaker | `lineId.wav` per line, `audioMs` | per line | file exists, not silent, sample rate matches; length within 0.3x to 3x of the slot; flags `tooLong`/`tooShort` set per rule |
| 6 | mix | music stems + line clips + options | `dub-mix.wav` | whole file, streamed | length equals source; dub energy present at each line start (±300 ms); music energy present where there is no speech; with `muteOriginal` no original speech energy (checked against `vocals.wav` correlation) |
| 7 | export | `dub-mix.wav` + source video | video file (video stream copied) or audio file | whole file | player-readable by ffprobe; video stream bit-identical when copied; chosen file name used; audio-only produces no video stream; existing file needs `overwrite` |

## Cross-stage rules
- **Order and skipping**: `separate` is skipped when `quality = fast` and the UI said so; `transcribe` then reads `audio.wav` directly; `mix` then has no music stem. With `muteOriginal` on, the output is dub only. With it off, the original audio is kept at its original level under the dub. The UI says that music cannot be kept separately in fast mode.
- **Resume**: a stage section is skipped on resume only when its output files exist, parse, and its recorded `inputHash` matches the current input and settings.
- **Atomic outputs**: every output is written as `name.part`, fsynced, then renamed.
- **Progress**: each stage emits progress as `done sections / total` with a within-section fraction from the worker; ETA = remaining sections × rolling mean of finished section times.
- **Interrupt test**: for each stage a test kills the process mid-section and checks that the next run completes with at most that section redone.
- **Parity**: the same fixture on both platforms must give the same line count and the same flags; audio values only need to be within tolerance.

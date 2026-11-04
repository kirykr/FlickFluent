# Research: Core Dub Pipeline (Local)

Findings come from general knowledge, not live checks in this session. Items marked **verify** must be confirmed in the first implementation task that touches them (licence text, model card, size, hash). Nothing is added to the catalogue until verified.

## R1. ML runtime and process model
- **Decision**: Rust core orchestrates; a Python 3.11 sidecar runs the model workers. The core starts it as a child process and talks over stdin/stdout JSON lines (contracts/sidecar-protocol.md).
- **Rationale**: The constitution already names this stack. The best permissive tools for these steps (faster-whisper, Demucs, CTranslate2, sherpa-onnx) have Python APIs. A thin protocol keeps job logic in Rust and lets the sidecar move off-box later.
- **Distribution**: Do not bundle Python in the installer (25 MB budget from 001). Download a per-platform runtime pack on first dub (python-build-standalone plus pinned wheels, hashes in the catalogue), size shown first. **verify** pack size per platform and arm64 wheel availability for every dependency.
- **Alternatives**: Rust-only with whisper.cpp and ONNX Demucs (no mature, permissive, maintained set for every stage). PyInstaller-frozen sidecar (large, slow start, antivirus false positives on Windows).

## R2. Media handling
- **Decision**: ffmpeg as a subprocess for extract, cut, resample, mix and mux. Use an LGPL build, dynamically separate from the app, downloaded on demand with hash check.
- **Rationale**: Only practical tool for "any common format". Streaming through ffmpeg keeps memory flat for long movies.
- **Licence**: FFmpeg LGPL-2.1+ when built without GPL parts (no x264/x265). Output video is stream-copied when possible; otherwise re-encode with a platform hardware encoder or built-in LGPL encoders. A licence notice is shown in About (Principle VI). **verify** which prebuilt LGPL builds exist for Windows and macOS arm64 and x64; fall back to building from source in CI if none are trustworthy.
- **Alternatives**: libav bindings (static linking raises LGPL obligations); GStreamer (heavier).

## R3. Voice and music separation
- **Decision**: Demucs `htdemucs` (two-stem: vocals vs rest) run on 5-minute windows with overlap, CPU baseline, int8/float32 as benchmarks dictate.
- **Rationale**: Constitution default. MIT code. Weights are released with the repository under MIT (**verify** current model card). Windowing bounds memory.
- **Risk**: CPU speed on 4 cores may exceed the SC-001 time budget. Benchmark in task one. Fallback: an ONNX vocal-only separation model (lighter), or "quality: fast" mode that skips separation for clips and notes the lost background music in the UI.
- **Alternatives**: Spleeter (older quality), MDX-Net/UVR weights (licences mixed, **verify** before any use).

## R4. Transcription and speakers
- **Decision**: faster-whisper (CTranslate2, MIT) with model size chosen by machine class: `tiny`/`base` on minimum, `small` default on mid, `medium`/`large-v3` only on full-local. Use silero-vad (MIT) to cut sections on silence. Language auto-detect from the first speech windows.
- **Speakers**: lightweight local diarization is the weak point. pyannote weights need a gated Hugging Face licence acceptance (not suitable). Decision: use speaker embeddings from a permissive model (**verify** candidates: wespeaker, 3D-Speaker or Silero-based) with simple clustering, and mark confidence. If none passes licence and quality checks, ship v1 with a single speaker per run plus gender-based voice choice and record the gap in the verification note.
- **Whisper language limits**: Whisper has no Khmer, and some low-resource languages are weak. The language-capability data lists them as not transcribable, which triggers the pre-run warning.
- **Alternatives**: whisper.cpp (good on CPU, but no word-level API parity and weaker batching); openai-whisper reference (slower, more RAM).

## R5. Translation
- **Decision**: CTranslate2 int8 with **M2M100 418M** (MIT) as the default, about 100 languages, with **M2M100 1.2B** for mid and full machines. Optional extra: **NLLB-200 distilled** (better quality, 200 languages), whose weights are CC-BY-NC-4.0 (**verify**), so it is isolated as a replaceable catalogue entry, shown with a licence notice, and off by default.
- **Rationale**: Principle VI prefers permissive; M2M100 is permissive but lower quality. Letting the user opt into NLLB keeps quality reachable without silently violating the principle.
- **Pivot**: When no direct pair is strong, translate through English for pairs the catalogue marks weak (quality flag on the pair).
- **Line fit**: translated text is translated per line with neighbour context; length ratio is computed after synthesis to flag too long or too short lines (no fitting in this feature).
- **Alternatives**: Opus-MT pair models (CC-BY-4.0, tiny, strong for common pairs, but one download per pair; kept as an optional catalogue entry for low-RAM machines); LLM translation (too heavy for baseline).

## R6. Speech synthesis
- **Decision**: **sherpa-onnx** (Apache-2.0) as the runtime for CPU TTS, with **Piper** VITS voices and **Kokoro-82M** (Apache-2.0) where they cover the target language. Each voice is its own catalogue entry with its own licence, language and gender. Only voices with permissive licences are listed by default.
- **Rationale**: Runs fast on CPU in small memory, so it fits the low-spec rule. Cloning engines (Chatterbox, others) are heavy and are scheduled with feature 005.
- **Coverage gap (flag for owner)**: Piper/Kokoro cover roughly 20 to 30 languages with good voices. Many target languages (Khmer for example) have only non-commercial voices (such as MMS-TTS, CC-BY-NC) or none. Plan: the capability data marks such languages as "no permissive voice"; the app offers subtitles-only, or an opt-in NC voice with a licence notice and isolation, per Principle VI. Which NC voices to allow is an owner decision, taken before the catalogue task.
- **Piper licence trap**: the original Piper engine repo was archived and the maintained successor is GPL. We use the engine through sherpa-onnx (Apache-2.0) and only the voice files. **verify** each voice's licence in its model card; some voices inherit dataset licences.
- **Timing**: speech is synthesized per line and stored per line, so the mix stage places each at its original start time. Overlong lines may overlap the next line only up to a cap, then are flagged.

## R7. Mixing and export
- **Decision**: Mix with ffmpeg filters: music stem (from separation) + dubbed speech track built by placing line clips at their start times, optional ducking off by default. "Mute original audio" removes the original mix and uses only music stem + dub. Video stream is copied untouched; audio-only exports WAV or AAC. Output container MP4 for video, M4A or WAV for audio.
- **Rationale**: Streaming ffmpeg graph; no full-movie buffer.
- **Edge**: if separation was skipped (fast mode), "mute original" would also remove music; the UI says so.

## R8. Long movies, sections and resume
- **Decision**: Fixed target section length of 5 minutes, cut at the nearest silence (silero-vad) within a 20 s window. Each section produces files in the work folder. A `run-state.json` (atomic write, 001's helper) records per-stage, per-section status with a content hash of inputs and settings. Resume reruns only sections whose status is not `done` or whose input hash changed.
- **Memory**: workers receive a file path and a window, never the whole file. Separation, transcription and synthesis each hold one section. Sidecar is restarted if its memory passes a ceiling from the machine class.
- **Cancel**: sends `cancel` to the sidecar, waits for the current inner step to reach a safe point (a few seconds), keeps finished files.
- **Crash**: next open finds `run-state.json` with a `running` marker and offers resume. Torn files are ignored because every output is written to `.part` and renamed.
- **Disk check**: estimate need before start (about 3 times the source audio size plus synth output) and show the exact shortfall.

## R9. Accelerators
- **Decision**: CPU baseline is required. CTranslate2 and sherpa-onnx can use CUDA, Metal (through ONNX Runtime CoreML) or DirectML where present. In this feature accelerators are optional speed-ups, off unless the benchmark proves a gain, to avoid platform-only code paths (Principle I). Windows CUDA packs are a download option only if licences allow redistribution (**verify**).

## R10. Benchmarks and spec values
- **Decision**: `flickfluent --bench` runs each light and heavy stage on a fixed 2-minute fixture and records wall time, peak memory and real-time factor into `docs/verification/002-benchmarks.md` and a JSON record per machine. The spec definition changes only when at least one minimum-class machine per OS and one full-local machine per OS have records.
- **Open from 001**: RAM minimum stays 8 GB unless a 4 GB machine record shows the light steps fit with the smallest models and 5-minute sections.
- **Method for minimum machine**: use a real low-end laptop where available; otherwise limit CPU and RAM (VM or cgroup/Job Object) and label the record "simulated".

## R11. Language data
- **Decision**: Each catalogue model carries `languages` (ISO 639 codes) and `step`. The core builds the dropdown as the union per step and computes, for a chosen pair, what each step supports. Codes are normalised once (e.g. `iw`→`he`, `zh` variants) in one mapping file. Display names come from the OS locale data in the UI; the code stays the key.
- **Rationale**: Principle IV and SC-005: drop a new catalogue entry in and the dropdown grows.

## R12. Licences summary (all **verify**)
| Component | Licence | Treatment |
|-----------|---------|-----------|
| faster-whisper, CTranslate2, Whisper weights | MIT | default |
| Demucs code and htdemucs weights | MIT | default |
| M2M100 weights | MIT | default |
| NLLB-200 weights | CC-BY-NC-4.0 | optional, isolated, notice |
| sherpa-onnx | Apache-2.0 | default |
| Kokoro-82M | Apache-2.0 | default |
| Piper voices | per voice | permissive only by default |
| silero-vad | MIT | default |
| FFmpeg (LGPL build) | LGPL-2.1+ | subprocess, notice |
| Python, python-build-standalone | PSF | runtime pack |

## Owner decisions (2026-11-04)
1. Non-commercial models and voices (NLLB, MMS-TTS Khmer and similar) are allowed as **opt-in** entries (`licenceClass: nonCommercial`, `defaultEnabled: false`) with a visible licence notice before download and in About. Each stays isolated and replaceable (Principle VI).
2. Minimum-spec benchmark uses **simulated** limits (CPU and RAM caps through VM, cgroup or Job Object). Records are labelled `simulated: true`. Spec values set from them stay flagged in `docs/verification/002-benchmarks.md` as simulated until a real low-end machine is measured.

## T005 verification log (2026-11-04)
Checked on Hugging Face and GitHub pages. Sizes and SHA-256 come from the Hugging Face API (`/api/models/<id>?blobs=true`) when the catalogue entry is written (T041).
| Item | Result |
|------|--------|
| M2M100 418M | MIT confirmed |
| NLLB-200 distilled 600M | CC-BY-NC-4.0 confirmed, kept opt-in (`nonCommercial`) |
| Kokoro-82M | Apache-2.0 confirmed; v1.0 has 8 languages and 54 voices |
| faster-whisper (Systran) small | MIT confirmed; `model.bin` 483,546,902 bytes, plus config, tokenizer, vocabulary files |
| Demucs | Code MIT. Upstream repo archived 2025-01-01; maintained fork `adefossez/demucs`. Weights licence not stated on the repo page: **still open**, so `htdemucs` stays out of the catalogue until the weights licence is confirmed from the model zoo files |
| Multi-file models | Whisper and others are several files, so the catalogue entry gains a `files` list (T009) |

### Follow-up checks (2026-11-04, later)
| Item | Result |
|------|--------|
| faster-whisper small, medium, large-v3 (Systran) | MIT on the model cards |
| M2M100 1.2B | MIT on the model card |
| Kokoro-82M (hexgrad) | Apache-2.0 on the model card |
| silero-vad, wespeaker, sherpa-onnx, CTranslate2, faster-whisper code | MIT, Apache-2.0, Apache-2.0, MIT, MIT (GitHub licence field) |
| Piper engine | `rhasspy/piper` (MIT) is archived. The maintained `OHF-Voice/piper1-gpl` is GPL-3.0, so it stays out. Voice files only, through sherpa-onnx. `rhasspy/piper-voices` shows MIT at repo level, but each voice has its own dataset licence: check the voice's `MODEL_CARD` when its catalogue entry is written (T041+). Voice entries with no clear permissive licence stay out. |
| Demucs | Maintained fork `adefossez/demucs` is MIT and active (pushed 2026-10-26). README says "Demucs is released under the MIT license" without a separate weights statement. The weights were trained on MUSDB18 plus extra data. **Owner decision (2026-11-04): accepted.** The README statement is taken as covering the htdemucs weights, so `htdemucs` may enter the catalogue (T041) as `permissive`. Revisit if a separate weights licence appears. |
| Speaker embeddings | `wespeaker` code is Apache-2.0; pick the exact checkpoint and check its card when T041 writes the entry. `pyannote/segmentation-3.0` is MIT but gated (needs a Hugging Face login), so it is not used. |
| FFmpeg macOS | No trusted prebuilt LGPL build. `.github/workflows/ffmpeg-macos.yml` builds one from the official tag and prints size and SHA-256. |

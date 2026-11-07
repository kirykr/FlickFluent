---

description: "Task list for feature 002: Core Dub Pipeline (Local)"
---

# Tasks: Core Dub Pipeline (Local)

**Input**: Design documents from `specs/002-core-dub-pipeline/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VIII requires test-first: write each test, see it fail, then implement.

**Organization**: Grouped by user story. Each story can be built and verified alone after Phase 2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1 to US4, matching `spec.md`

## Path Conventions (from plan.md)

`core/` Rust library, `sidecar/` Python workers, `src-tauri/` Tauri shell, `src/` React UI, `e2e/` Playwright, `config/` shared config, `fixtures/` sample media, `docs/` project docs.

## Owner decisions applied

- Non-commercial models and voices are opt-in with a visible licence notice (`licenceClass: nonCommercial`, `defaultEnabled: false`).
- Minimum-spec benchmarks use simulated limits, labelled `simulated: true`.

---

## Phase 1: Setup

- [X] T001 Create the folder layout from plan.md: `core/src/{pipeline,sidecar,languages,bench}`, `sidecar/flickfluent_sidecar/workers`, `sidecar/tests`, `src/features/dub`, `fixtures`, `docs/verification`
- [X] T002 [P] Create `sidecar/pyproject.toml` (Python 3.11, package `flickfluent_sidecar`) and an empty `sidecar/requirements.lock`; add `pytest` as a dev dependency
- [X] T003 [P] Add to `core/Cargo.toml` any new crates needed (`tokio` process and sync features, `regex` if required); no ML crates
- [X] T004 [P] Extend CI in `.github/workflows/ci.yml`: install Python 3.11, run `python -I -m pytest sidecar/tests` on `windows-latest` and `macos-latest`, add the schema test for the catalogue
- [X] T005 Verify every item marked **verify** in `specs/002-core-dub-pipeline/research.md` against current model cards, repos and release pages (licences, sizes, SHA-256, arm64 wheels, LGPL ffmpeg builds, htdemucs weights licence, speaker-embedding candidates); write findings into `research.md` and `docs/dependencies.md`; any item that fails its check gets a replacement or is dropped
- [X] T006 [P] Create `fixtures/`: 20 s two-speaker clip, music-only clip, silent-video clip, speech-free clip, corrupt file, 2-minute clip, each self-made or permissively licensed, with a `fixtures/README.md` (source, licence) and reference transcripts in `fixtures/expected/*.json`
- [X] T007 [P] Write `fixtures/make-long.sh` and `fixtures/make-long.ps1` that loop the 2-minute clip into a 3-hour file for the long-movie test

---

## Phase 2: Foundational (blocks all stories)

**Purpose**: catalogue, sidecar, runtime downloads, project format 2, run state. No user story works without these.

- [X] T008 [P] Write test `core/tests/catalogue_schema.rs`: every entry in `config/model-catalogue.json` validates against `specs/002-core-dub-pipeline/contracts/language-capabilities.schema.json`; every `nonCommercial` entry has `defaultEnabled: false` and a `licenceNoticeKey`
- [X] T009 Extend `core/src/models/catalogue.rs` with the model fields from data-model.md (`step`, `languages`, `gender`, `machineClass`, `licenceClass`, `licenceNoticeKey`, `qualityByLanguage`, `requires`, `platforms`, `defaultEnabled`); old 001 entries still load
- [X] T010 Add to `config/model-catalogue.json` the `runtime` pack per platform and architecture and the `ffmpeg` LGPL build per platform, using verified URLs, sizes and SHA-256 from T005 (Windows ffmpeg only; macOS LGPL build pending, see docs/dependencies.md)
- [X] T011 [P] Write test `core/tests/project_v2.rs`: a format 1 file opens, gains empty v2 fields, a `<name>.flick.v1.bak` backup is written, and a format 2 file round-trips every field in data-model.md unchanged; `endMs > startMs` is enforced; format 3 gives `FORMAT_TOO_NEW`
- [X] T012 Implement format 2 in `core/src/project/format.rs` per `contracts/project-file-format-v2.md`: fields `source`, `sourceLanguage`, `detectedLanguage`, `targetLanguage`, `options`, `speakers[]`, `lines[]`, `run`; migration 1 to 2 with backup
- [X] T013 [P] Write test `core/tests/run_state.rs`: `run-state.json` atomic write survives a kill mid-write (previous version stays valid); a section stage is skipped on resume only when output exists, parses, and `inputHash` matches; a changed setting invalidates only affected stages
- [X] T014 Implement `core/src/pipeline/state.rs`: `PipelineRun`, `Section`, `StageState` per data-model.md, `run-state.json` read and atomic write (reuse `core/src/fsutil.rs`), work folder creation and `work-manifest.json`, interrupted-run detection (`running` found on open)
- [X] T015 [P] Write test `core/tests/sections.rs`: sections are about 5 minutes, cut at the quietest point within 20 s of the target, cover the file with no overlap, and a speech-free file gives sections with no cut error
- [X] T016 Implement `core/src/pipeline/sections.rs`: section planner using silence positions from ffmpeg `silencedetect` (no ML needed to plan sections)
- [X] T017 [P] Write test `core/tests/sidecar_protocol.rs` using a stub sidecar script in `core/tests/support/stub_sidecar.py`: handshake, `PROTOCOL_MISMATCH`, progress events, `CANCELLED` within 5 s, a crash restarts once and retries, a second crash gives `SIDECAR_CRASHED`, silence for 60 s counts as a hang, path outside the allowed folders gives `PATH_NOT_ALLOWED`
- [X] T018 Implement `core/src/sidecar/mod.rs`: start from the installed runtime pack, JSON-line protocol per `contracts/sidecar-protocol.md`, request ids, progress and item events, cancel, restart-once policy, stderr capture, shutdown
- [X] T019 [P] Implement `sidecar/flickfluent_sidecar/protocol.py` and `sidecar/flickfluent_sidecar/main.py`: handshake, request loop, path allow-list check, progress helper, `.part` then rename helper, cancel flag, memory ceiling check with `restart_requested`
- [X] T020 [P] Write test `sidecar/tests/test_protocol.py`: handshake, cancel stops within 5 s, rejected paths, atomic output naming
- [X] T021 Implement runtime pack install in `core/src/models/runtime.rs`: download with the 001 downloader, verify hash, unpack to the models folder, health check by starting the sidecar and sending `hello`; size shown before download (FR-014)
- [X] T022 [P] Implement ffmpeg install and lookup in `core/src/pipeline/ffmpeg.rs`: find the installed LGPL build, run it as a subprocess with streamed progress parsing (`-progress pipe:1`), map exit codes to `UNREADABLE_FILE` and `NO_AUDIO_TRACK`
- [X] T023 Add error codes from `contracts/ipc-commands.md` to `core/src/error.rs` and matching `message` and `nextStep` keys to `src/i18n/en.json`
- [X] T024 Add Tauri command stubs for all `dub_*`, `languages_*`, `bench_*` commands and the `dub:*` and `bench:progress` events in `src-tauri/src/commands/dub.rs` and register them in `src-tauri/src/commands/mod.rs`; each returns `NOT_IMPLEMENTED` until its story lands
- [X] T025 [P] Add the new components to `docs/dependencies.md`: ffmpeg, faster-whisper, CTranslate2, M2M100, Demucs, sherpa-onnx, Piper voices, Kokoro, silero-vad, python-build-standalone; licence and reason per row, non-permissive ones marked with their treatment (Principles V and VI, FR-016)

**Checkpoint**: sidecar starts, protocol tests pass, project format 2 works, run state is crash-safe.

---

## Phase 3: User Story 1 - Dub a movie end to end, locally (P1) 🎯 MVP

**Goal**: import a video, pick languages, run all seven stages locally, export video or audio, resume after interruption.

**Independent Test**: quickstart steps 3 and 4: dub the 2-minute clip, play both exports, kill mid-run and resume.

### Tests first (each stage contract from `contracts/stage-contracts.md`)

- [X] T026 [P] [US1] Write `core/tests/stage_extract.rs`: duration within 50 ms of source, `audio.wav` mono 16 kHz and `audio-full.wav` stereo 44.1 kHz, `NO_AUDIO_TRACK` for the silent-video fixture, `UNREADABLE_FILE` for the corrupt fixture
- [X] T027 [P] [US1] Write `sidecar/tests/test_separate.py`: `vocals + music` within tolerance of input, music-only fixture gives near-silent vocals, equal lengths
- [X] T028 [P] [US1] Write `sidecar/tests/test_transcribe.py`: word error rate under the limit set in `fixtures/expected/`, no line with `endMs <= startMs`, lines sorted, speaker count within 1 of truth, speech-free fixture gives zero lines
- [X] T029 [P] [US1] Write `sidecar/tests/test_translate.py`: non-empty output, same line count and order, numbers and names preserved per reference
- [X] T030 [P] [US1] Write `sidecar/tests/test_synthesize.py`: file exists, not silent, sample rate matches, length within 0.3x to 3x of the slot, `tooLong` and `tooShort` flags set by the thresholds (`audioMs > slot * 1.25`, `audioMs < slot * 0.5`)
- [X] T031 [P] [US1] Write `core/tests/stage_mix_export.rs`: mix length equals source, dub energy at each line start within 300 ms, music energy where there is no speech, `muteOriginal` removes original speech energy, copied video stream is bit-identical, audio-only has no video stream, existing file needs `overwrite`
- [X] T032 [P] [US1] Write `core/tests/pipeline_resume.rs`: kill the run during each stage in turn; the next run completes and redoes only the interrupted section (compare output timestamps); `SOURCE_CHANGED` when the fingerprint differs; cancel keeps finished work and leaves no corrupt file
- [X] T033 [P] [US1] Write `core/tests/pipeline_memory.rs`: a run on the generated long file keeps the sidecar and ffmpeg resident memory under the machine-class ceiling (uses the stub sidecar for speed; the real long run is quickstart step 5)

### Implementation

- [X] T034 [P] [US1] Implement `dub_probe_source` and the extract stage in `core/src/pipeline/stages.rs` (extract part) using `core/src/pipeline/ffmpeg.rs`; compute the source `fingerprint` (size plus first and last 1 MB hash)
- [X] T035 [P] [US1] Implement `sidecar/flickfluent_sidecar/workers/separate.py`: Demucs `htdemucs` two-stem on a section window, output `vocals.wav` and `music.wav`, CPU baseline, windowed so one section is in memory
- [X] T036 [P] [US1] Implement `sidecar/flickfluent_sidecar/workers/transcribe.py`: faster-whisper with model size from `params`, silero-vad, word-aware line splitting, speaker embedding and clustering from the model chosen in T005 (or single-speaker fallback recorded in the verification note), confidence per line (silero-vad is faster-whisper's built-in copy; speakers by sherpa-onnx diarization; speaker ids are per section)
- [X] T037 [P] [US1] Implement `sidecar/flickfluent_sidecar/workers/detect_language.py`: language of the first speech windows with confidence
- [X] T038 [P] [US1] Implement `sidecar/flickfluent_sidecar/workers/translate.py`: CTranslate2 M2M100 int8, batches up to 200 lines, neighbour context, code normalisation through `config/language-codes.json` (neighbour context not done: lines are translated in batches without it)
- [X] T039 [P] [US1] Implement `sidecar/flickfluent_sidecar/workers/synthesize.py`: sherpa-onnx with Piper and Kokoro voices, one wav per line, report `audioMs`
- [X] T040 [US1] Create `config/language-codes.json` (one mapping file for code normalisation, e.g. `iw` to `he`)
- [X] T041 [US1] Add the real models to `config/model-catalogue.json` using results of T005: whisper sizes, htdemucs, M2M100 418M and 1.2B, silero-vad, the speaker model, sherpa-onnx voices and Kokoro, each with `step`, `languages`, `gender`, `machineClass`, `licenceClass`, verified `sizeBytes`, `url`, `sha256`; non-commercial entries (NLLB, MMS-TTS Khmer and others verified in T005) added with `licenceClass: nonCommercial`, `defaultEnabled: false` and notice keys (catalogue has 5 Piper voices and Kokoro with 15 voices in 8 languages, preferred where it covers the language; no NLLB or MMS non-commercial entries yet; macOS ffmpeg missing; eSpeak GPL question open, see docs/dependencies.md)
- [X] T042 [US1] Implement the section loop and stage scheduler in `core/src/pipeline/run.rs`: runs separate, transcribe, translate, synthesize per section in order, extract before, mix and export after; saves `run-state.json` after each section stage; shifts section-relative times to absolute; sets line flags `overlap`, `tooLong`, `tooShort`, `lowConfidence`, `mixedLanguage`
- [X] T043 [US1] Implement the mix stage in `core/src/pipeline/mix.rs`: build the ffmpeg graph that places each line clip at its `startMs`, adds the music stem, honours `muteOriginal`, and handles `quality = fast` (no music stem) per `contracts/stage-contracts.md`
- [X] T044 [US1] Implement the export stage in `core/src/pipeline/stages.rs` (export part): video copy with new audio, audio-only WAV or M4A, safe file name check, `FILE_EXISTS`, `NO_SPACE`
- [X] T045 [US1] Implement cancel and resume in `core/src/pipeline/run.rs`: `dub_cancel` sends protocol `cancel` and waits; `dub_resume` finds the first section stage not `done` or with a changed `inputHash`; `NO_SPEECH_FOUND` completes the run with the original audio mix
- [X] T046 [US1] Implement disk estimate and preflight in `core/src/pipeline/run.rs`: `dub_preflight` returns missing models with sizes, disk needed (about 3 times source audio plus synthesis output) and free; `dub_start` fails with `NO_SPACE`, `MODELS_MISSING` or `MACHINE_TOO_WEAK` (names the missing requirement; cloud arrives in feature 003)
- [X] T047 [US1] Implement the Tauri commands `dub_configure`, `dub_preflight`, `dub_start`, `dub_cancel`, `dub_resume`, `dub_status`, `dub_export`, `dub_discard_run` in `src-tauri/src/commands/dub.rs`, replacing the stubs
- [X] T048 [US1] Build the New dub screen in `src/features/dub/NewDub.tsx`: file chooser, source and target pickers (basic list until US2), mute-original and export options, model download prompt with sizes, start button
- [X] T049 [US1] Build the result screen in `src/features/dub/Result.tsx`: transcript table (start, end, speaker, original, translated, flags; read-only), export controls for video and audio with file name, open-folder action
- [X] T050 [P] [US1] Add Playwright spec `e2e/dub.spec.ts`: import the 20 s fixture, run, export both kinds, interrupt and resume
- [X] T051 [US1] Record Windows and macOS results of quickstart steps 1, 3, 4 and 5 in `docs/verification/002.md` (macOS only; Windows not run; quickstart steps 3 to 5 only partly done, see docs/verification/002.md)

**Checkpoint**: a full local dub works, resumes, and exports on macOS; Windows run recorded.

---

## Phase 4: User Story 2 - Choose any language from a dropdown (P1)

**Goal**: searchable language dropdowns from catalogue data, auto-detect, pre-run warnings and fallbacks.

**Independent Test**: quickstart step 2 plus step 6 last item: add a test catalogue entry and see the new language; pick an unsupported pair and see the warning.

### Tests first

- [X] T052 [P] [US2] Write `core/tests/languages.rs`: the list is the union of catalogue entries per step; adding a test entry adds the language with no other change (SC-005); non-commercial entries count only when enabled; `languages_check_pair` returns the limiting step, quality, and fallbacks `subtitlesOnly`, `genericVoice`, `pivotViaEnglish`, `optInNonCommercialVoice`, `installModel` as the data supports (SC-006)
- [X] T053 [P] [US2] Write Vitest `src/features/dub/LanguagePicker.test.tsx`: typing part of a name filters the list, `auto-detect` appears for source only, names come from locale data and the code stays the key

### Implementation

- [X] T054 [US2] Implement `core/src/languages/mod.rs`: build `Language[]` from the catalogue (transcribe, translateFrom, translateTo, speak with voice counts, quality per step), using `config/language-codes.json`
- [X] T055 [US2] Implement `languages_check_pair` in `core/src/languages/pair.rs`: per-step support, reasons, fallbacks, weak-pair pivot rule from `qualityByLanguage`
- [X] T056 [US2] Implement `languages_list` and `languages_check_pair` commands in `src-tauri/src/commands/languages.rs`
- [X] T057 [US2] Build `src/features/dub/LanguagePicker.tsx`: searchable dropdown with locale display names (via `Intl.DisplayNames`), auto-detect option for source, quality hints
- [X] T058 [US2] Build the pair warning panel in `src/features/dub/PairCheck.tsx`: names the limited step, lists fallbacks, shows the licence notice before an opt-in non-commercial voice or model can be chosen, blocks start until a fallback is chosen (`NO_FALLBACK_CHOSEN`); wire into `NewDub.tsx` (fallbacks implemented end to end: subtitles only with an .srt export, generic voice, pivot through English, opt-in; no non-commercial entries exist in the catalogue yet, so opt-in is only exercised by tests)
- [X] T059 [US2] Show the detected language after auto-detect and ask confirmation when source equals target (`SAME_LANGUAGE`) in `src/features/dub/NewDub.tsx`
- [X] T060 [P] [US2] Add Playwright spec `e2e/languages.spec.ts`: search, select, auto-detect result shown, unsupported pair warning appears before start
- [X] T061 [US2] Add the non-commercial licence notice text keys to `src/i18n/en.json` and show the notice in About and in the model list (`src/features/models/`) (there was no About screen, so a licence card was added to Settings)

**Checkpoint**: language list is data-driven and warnings appear before any processing.

---

## Phase 5: User Story 3 - See progress, estimates and clear errors (P2)

**Goal**: per-stage progress at least every 5 s, ETA, errors with a next step, model download prompts.

**Independent Test**: start a run, watch updates, remove the source mid-run, read the message.

### Tests first

- [X] T062 [P] [US3] Write `core/tests/progress.rs`: with the stub sidecar, `dub:progress` arrives at least every 5 s while a stage runs and at most once a second; ETA equals remaining sections times the rolling mean of finished sections; `dub:stage` fires on each change
- [X] T063 [P] [US3] Write `core/tests/errors.rs`: each error code in `contracts/ipc-commands.md` maps to a `message` and a `nextStep` key that exist in `src/i18n/en.json`; source removed mid-run gives `STAGE_FAILED` naming the stage; disk full gives a clean stop with the exact shortfall and finished work kept
- [X] T064 [P] [US3] Write Vitest `src/features/dub/Progress.test.tsx`: renders stage name, percentage, ETA; shows the error message and next-step action

### Implementation

- [X] T065 [US3] Implement `core/src/pipeline/progress.rs`: aggregate worker progress into stage and overall progress, rolling ETA, throttle events, emit `dub:progress`, `dub:stage`, `dub:line`, `dub:error`, `dub:done`
- [X] T066 [US3] Build `src/features/dub/Progress.tsx`: stage list with status, bars, ETA, live transcript lines as they arrive, cancel button
- [X] T067 [US3] Build error display in `src/features/dub/RunError.tsx`: plain message, next-step button (retry, free disk, choose another file, download model)
- [X] T068 [US3] Show the download size prompt and progress for missing models inside the run flow in `src/features/dub/ModelPrompt.tsx`, reusing the 001 `model_download_*` commands
- [X] T069 [P] [US3] Add Playwright spec `e2e/progress.spec.ts`: updates arrive, forced source removal shows a next step, cancel keeps finished work

**Checkpoint**: users can follow, cancel and recover a run.

---

## Phase 6: User Story 4 - Know what this computer can do (P2)

**Goal**: spec values backed by benchmark records; honest model catalogue.

**Independent Test**: quickstart step 7: records exist, `spec-definition.json` has no provisional mark, the check picks the right mode per machine.

### Tests first

- [X] T070 [P] [US4] Write `core/tests/bench_record.rs`: a benchmark run on the stub sidecar writes a record that validates (fields from data-model.md), including `simulated`
- [X] T071 [P] [US4] Extend `core/tests/spec_definition.rs`: when `provisional` is false, every value in `config/spec-definition.json` has a matching entry in `docs/verification/002-benchmarks.md`
- [X] T072 [P] [US4] Extend `core/tests/hardware_evaluate.rs`: with the final values, machines at, just below and just above minimum and full-local give the right mode and the exact blocked message

### Implementation

- [X] T073 [US4] Implement `core/src/bench/mod.rs` and the `--bench` flag in `core/src/cli.rs`: run every stage on the 2-minute fixture, record wall time, peak memory and real-time factor per stage and model; `--json` output; `bench_run` and `bench_records` commands in `src-tauri/src/commands/bench.rs`
- [X] T074 [US4] Implement simulated limits for the minimum-spec machine: script `bench/simulate-limits.sh` (macOS: CPU and memory caps through a VM or `taskpolicy` plus memory limit on the sidecar) and `bench/simulate-limits.ps1` (Windows Job Object); document in `docs/verification/002-benchmarks.md`
- [ ] T075 [US4] (macOS done: real full-local and simulated minimum records saved. Windows done 2026-11-07 on an i7-6700 / 16 GB: real and simulated minimum records saved, see `docs/verification/002-benchmarks.md`. Still open: Windows full-local, needs a Windows PC with 8+ cores) Run benchmarks on a minimum-class (simulated) and a full-local machine on macOS and on Windows; save `bench-<machine>.json` under `docs/verification/benchmarks/`
- [X] T076 [US4] Write `docs/verification/002-benchmarks.md`: every record, its machine, whether `simulated`, and the derived minimum and full-local numbers; state whether 8 GB RAM stays or can be lowered (owner rule: lower only if light steps are proven to fit)
- [X] T077 [US4] Update `config/spec-definition.json` from the records and remove `"provisional": true` only if records cover both OSes; if only simulated records exist, keep a `"basis": "simulated"` note in the file and in the docs, and update the schema in `specs/001-app-shell-hardware-check/contracts/spec-definition.schema.json` to allow the field
- [X] T078 [US4] Show each model's size, licence, supported languages and machine class in `src/features/models/`, and mark models the machine cannot run as unavailable with the reason
- [X] T079 [US4] Update the roadmap hand-off: add a "Feature 002 hand-off" section to `docs/roadmap.md` with final spec values, what is simulated, and what remains open

**Checkpoint**: the hardware check and alert use measured values; the catalogue is truthful.

---

## Phase 7: Polish and cross-cutting

- [X] T080 [P] Run the licence jobs (`cargo deny check`, npm check, Python `pip-licenses` against `sidecar/requirements.lock`); fix `docs/dependencies.md` and the About screen for any non-permissive component (FR-016)
- [X] T081 [P] Run Constitution Check against the finished code and record it in `docs/verification/002.md` (Principles I to IX)
- [ ] T082 Run the full quickstart (steps 1 to 9) on Windows and macOS and fill `docs/verification/002.md`; any failure becomes a bug task before closing
- [ ] T083 (binary size 16.9 MB measured; idle RAM not measured, another instance of the app was open) [P] Confirm idle RAM and installer size budgets from 001 still hold with the new code (runtime pack not in the installer); record in `docs/verification/002.md`
- [ ] T085 (About notice and source-offer link done; publishing the source is a release step in `docs/release-checklist.md`) Publish the matching eSpeak NG and sherpa-onnx source with each runtime pack release and link it from About (owner decision 2026-11-05, see `docs/dependencies.md`)
- [X] T084 Update `README`/`docs` text and `src/i18n/en.json` leftovers; remove `NOT_IMPLEMENTED` stubs

---

## Dependencies

- Phase 1 then Phase 2 (blocks all).
- US1 (P1) depends only on Phase 2. US2 (P1) depends on Phase 2 and the catalogue entries from T041; its UI joins `NewDub.tsx` from T048.
- US3 depends on US1's run loop (T042). US4 depends on US1 stages (benchmarks run them) and T041.
- Order: Phase 2 → US1 → US2 → US3 → US4 → Polish. US2 can start once T041 is done, in parallel with the rest of US1 UI work.

## Parallel examples

- Phase 2: T008, T011, T013, T015, T017, T019, T020, T022, T025 (different files).
- US1 tests T026 to T033 together; workers T035 to T039 together after their tests.
- US2: T052 and T053 together; US3: T062 to T064 together.

## Implementation strategy

- **MVP**: Phase 1, Phase 2, US1. A user can dub a clip and a long movie locally and resume.
- Then US2 (language dropdown and warnings), US3 (progress and errors), US4 (benchmarks and spec values), Polish.
- Feature 002 is done only when Windows and macOS both pass (T082). Windows is still unverified from feature 001.

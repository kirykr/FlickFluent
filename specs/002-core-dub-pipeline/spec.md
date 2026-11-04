# Feature Specification: Core Dub Pipeline (Local)

**Feature Branch**: `002-core-dub-pipeline` (no git branch; repo is not under git)

**Created**: 2026-11-04

**Status**: Draft

**Input**: User description: "Feature 002: Core dub pipeline (local). Covers master spec Stories 1 and 2, FR-002 to FR-008, FR-012, FR-016 (basic), FR-024, FR-025. Depends on feature 001 (app shell and hardware check). Also replaces provisional hardware values in config/spec-definition.json with real benchmarks and adds real models to the model catalogue."

Master spec: `docs/master-spec/spec.md`. Roadmap: `docs/roadmap.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Dub a movie end to end, locally (Priority: P1)

A user imports a video, picks source and target language, and starts processing. The app separates voice from background music, transcribes the speech with times and speakers, translates it, produces dubbed speech, mixes it with the original background music, and exports a dubbed video or audio file. Everything runs on the user's machine. No cloud key is needed.

**Why this priority**: This is the product. Every later feature builds on a working local dub.

**Independent Test**: Import a 2-minute clip in language A, pick language B, press start, and play the exported file. Speech is in language B, background music is still audible, and speech lines up with the on-screen action.

**Acceptance Scenarios**:

1. **Given** an imported video and chosen languages, **When** the user starts processing, **Then** progress is shown per stage, and a transcript with start time, end time, speaker and text for each line is available when transcription ends.
2. **Given** processing has finished, **When** the user exports full video, **Then** the output has dubbed speech plus the original background music, and it plays in common media players on Windows and macOS.
3. **Given** processing has finished, **When** the user exports audio only, **Then** only the mixed dubbed audio file is produced, with the chosen file name.
4. **Given** the user chose to mute the original audio, **When** the dub is mixed, **Then** no original speech remains in the output.
5. **Given** a long movie (over 90 minutes), **When** processing runs, **Then** it completes without running out of memory, because it works in sections.
6. **Given** processing is running, **When** the user cancels, closes the app, or the app crashes, **Then** finished stages and sections are kept, the project is not corrupted, and a later run resumes without redoing them.

---

### User Story 2 - Choose any language from a dropdown (Priority: P1)

The user selects source and target language from searchable dropdowns. The list shows every language the installed engines support. The source language can also be "auto-detect".

**Why this priority**: "Any language" is the headline promise, and it must be data-driven (constitution Principle IV).

**Independent Test**: Open the dropdown, search for a language, select it, run a short clip, and confirm output in that language. Pick a language a step does not support and confirm a warning appears before processing.

**Acceptance Scenarios**:

1. **Given** the language dropdown, **When** the user types part of a language name, **Then** matching languages are listed.
2. **Given** source set to auto-detect, **When** processing starts, **Then** the detected language is shown to the user and used for the run.
3. **Given** a language that one step cannot handle (transcribe, translate or speak), **When** the user selects it, **Then** the app names the limited step and offers a fallback (for example subtitles only, or a generic voice) before any processing starts.
4. **Given** a language is added to the engine capability data, **When** the app is reopened, **Then** it appears in the dropdown with no change to the interface.

---

### User Story 3 - See progress, estimates and clear errors (Priority: P2)

While a dub runs, the user sees which stage is active, how far along it is, and an estimated time remaining. When something fails, the app says what failed and what to do next.

**Why this priority**: A dub of a long movie takes a long time. Without visible progress and clear errors, users cannot trust or recover a run.

**Independent Test**: Start a dub, watch stage names and estimates update, then force a failure (for example remove the source file mid-run) and read the message.

**Acceptance Scenarios**:

1. **Given** a running dub, **When** a stage advances, **Then** the stage name, percentage and estimated time remaining update.
2. **Given** a stage fails, **When** the error is shown, **Then** it names the stage and the cause in plain words and gives a next step (retry, free disk space, choose another file).
3. **Given** a required model is not yet installed, **When** processing needs it, **Then** the app shows the download size and asks before downloading.

---

### User Story 4 - Know what this computer can do (Priority: P2)

The hardware values that decide minimum and full-local mode come from measured results of the real models, not guesses. The model catalogue lists the real models with size, licence, languages and which machines can run them.

**Why this priority**: The shell from feature 001 ships with provisional values. Real models make the mode choice and the incompatibility message truthful.

**Independent Test**: Run the benchmark on a machine at the minimum spec and on a stronger one. Check that the final values in the spec definition match results, and that the hardware check picks the right mode on each.

**Acceptance Scenarios**:

1. **Given** benchmark results for light steps on low-end hardware, **When** the spec definition is updated, **Then** it is no longer marked provisional and the check, alert and docs all use the new values.
2. **Given** the model catalogue, **When** the user views a model, **Then** its size, licence and supported languages are shown before download.
3. **Given** a machine that cannot run a model, **When** the catalogue is shown, **Then** that model is marked unavailable and the reason is stated.

---

### Edge Cases

- Video with no speech, or only music: the app reports no speech found and still allows export of the original audio mix.
- Unsupported or corrupt video file: clear error naming the problem, no crash.
- Video with no audio track: clear error before processing starts.
- Overlapping speakers: the overlap is marked on the affected lines.
- Mixed languages in one video: lines in other languages are flagged; per-line language control belongs to a later feature.
- Disk space runs out during processing: the app stops cleanly, keeps finished work, and states how much space is needed.
- Translated speech is much longer or shorter than its time slot: the line is flagged before export. Automatic fitting belongs to a later feature.
- Chosen output language has no available voice: warn before processing and offer a fallback.
- User closes the app or loses power during processing: no corrupted project; at most the section in progress is lost.
- Source and target language are the same: the app warns and asks to confirm.
- Machine cannot run a heavy step locally: the app states this clearly. Cloud fallback arrives in feature 003.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to import a video file of any common format and length, short clip or full-length movie, and see an error naming the problem for files that cannot be read.
- **FR-002**: The app MUST separate speech from background music and keep the music for the final mix.
- **FR-003**: The app MUST transcribe speech with start and end times for each line and detect speakers.
- **FR-004**: The app MUST translate the transcript from the chosen source language to the chosen target language, on the user's machine.
- **FR-005**: The app MUST produce dubbed speech for the translated transcript using a standard voice for the target language, with a male or female choice per speaker where the engine allows.
- **FR-006**: Users MUST be able to pick source and target language from searchable dropdowns, with an auto-detect option for source.
- **FR-007**: The language list MUST come from engine capability data, so adding a language needs no interface change.
- **FR-008**: The app MUST warn before processing if a chosen language is unsupported by a step, and offer a fallback.
- **FR-009**: The app MUST mix dubbed speech with the original background music and let the user mute the original audio.
- **FR-010**: The app MUST export full video or audio only, with a user-chosen file name.
- **FR-011**: The app MUST process long movies in sections so memory use stays bounded, and MUST make work resumable after cancel, crash or close.
- **FR-012**: The app MUST show per-stage progress, estimated time remaining, and errors that state the cause and a next step.
- **FR-013**: The app MUST stay responsive while a dub runs, and allow the user to cancel at any time without losing finished work.
- **FR-014**: Models MUST be downloaded on demand with their size shown first. The model catalogue MUST list each real model with size, licence, supported languages and the machine class that can run it.
- **FR-015**: Hardware values in the spec definition MUST be replaced with values backed by recorded benchmarks of the real models, and the "provisional" mark removed. The RAM minimum from the owner decision (8 GB) is lowered only if light steps are proven to fit on less.
- **FR-016**: Each reused open-source component and its licence MUST be recorded in `docs/dependencies.md`, with a visible licence notice for any whose licence is not commercial-friendly.
- **FR-017**: Each pipeline stage (extract, separate, transcribe, translate, synthesize, mix and export) MUST have a contract test on a short sample clip that passes on Windows and macOS.

### Key Entities

- **Project**: A saved dubbing job: source video, languages, transcript, chosen voices, export settings, and per-stage processing state.
- **Transcript Line**: Start time, end time, original text, translated text, speaker, voice, status, and flags (overlap, too long, too short).
- **Speaker**: A detected person with a gender and an assigned standard voice.
- **Language**: Code, display name, and which steps support it (transcribe, translate, speak).
- **Model**: A downloadable engine file with step, size, licence, supported languages and required machine class.
- **Pipeline Run**: One attempt to process a project, with stage states, section progress and resume point.
- **Benchmark Record**: Measured time and memory for a model on a stated machine, used to set the spec values.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can dub a 2-minute clip from import to exported file in under 10 minutes on a machine at the minimum spec, not counting one-time model downloads.
- **SC-002**: A full-length movie of up to 3 hours completes on a minimum-spec machine without crashing or running out of memory.
- **SC-003**: Closing the app or losing power mid-run loses no more than the section being processed, in 100% of tested interruptions.
- **SC-004**: Background music remains audible and speech is intelligible in at least 90% of reviewed test clips.
- **SC-005**: Adding a language to the engine capability data makes it appear in the dropdown with no interface update.
- **SC-006**: In 100% of tested unsupported-language selections, the warning and fallback appear before processing starts.
- **SC-007**: The same sample clip produces an equivalent result on Windows and macOS, and all acceptance scenarios pass on both.
- **SC-008**: At least 90% of first-time users complete a first dub without help.
- **SC-009**: Every stage shows progress that updates at least once every 5 seconds while it runs, and every failure message names a next step.
- **SC-010**: The spec definition has no provisional mark, and each value traces to a benchmark record.

## Assumptions

- Voice cloning, speaker profiles with control instructions, the transcript editor, Smart Fit and timing tools, subtitle import and export, and video tools belong to features 004 to 006. This feature uses standard voices and does not fit speech to timing; it only flags lines that run too long or too short.
- Cloud engines and API keys belong to feature 003. Here, translation and speech run locally. A machine that cannot run a heavy step locally gets a clear message naming the missing requirement until feature 003 adds the cloud option.
- Export in this feature is full video or audio only (basic FR-016). Aspect ratio, privacy mask, colour and editor project export belong to feature 006.
- "Any language" means every language supported by the installed engines, not equal quality in all of them.
- A short saved project is part of resume support; multi-project tabs and batch queues belong to feature 007.
- Defaults from the constitution apply: ffmpeg, faster-whisper, Demucs, NLLB or M2M100, and a permissively licensed TTS. The plan confirms the choices and licences.
- The app shell, hardware check, theme, hotkeys and settings from feature 001 are reused as they are.
- Windows is still unverified from feature 001. This feature's tests must be run there before it is called done.

# Feature Specification: FlickFluent Movie Dubbing App

**Feature Branch**: `001-movie-dubbing-app` (no git branch; repo is not under git)

**Created**: 2026-11-04

**Status**: Draft

**Input**: User description: "A cross-platform (Windows + macOS) desktop app that translates and dubs a short or long movie from any source language to any target language chosen from dropdowns, with voice cloning, vocal/music separation, subtitles, a timeline editor, batch projects, and a hardware compatibility check. Editor modelled on the reference video 'Professor Storytelling AI Video and Voice Studio PRO'."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Dub a movie end to end (Priority: P1)

A user imports a video, picks the source and target language from dropdowns, and starts processing. The app separates voice from background music, transcribes the speech with timestamps, translates it, produces dubbed speech, mixes it with the original background music, and exports a dubbed video.

**Why this priority**: This is the product. Without it nothing else has value.

**Independent Test**: Import a 2-minute clip in language A, pick language B, press start, and play the exported file. Speech is in language B, background music is still audible, and speech lines up with the on-screen action.

**Acceptance Scenarios**:

1. **Given** an imported video and chosen languages, **When** the user starts processing, **Then** progress is shown per stage and a transcript with start and end times appears in the editor table.
2. **Given** processing has finished, **When** the user exports full video, **Then** the output has dubbed speech plus the original background music and plays in common media players on both Windows and macOS.
3. **Given** a long movie (over 90 minutes), **When** processing runs, **Then** it completes without running out of memory, and the user can cancel and later resume without redoing finished stages.

---

### User Story 2 - Choose any language from a dropdown (Priority: P1)

The user selects source and target language from searchable dropdowns. The list shows every language the available engines support. The source language can also be "auto-detect".

**Why this priority**: Explicit requirement. "Any language" is the headline promise.

**Independent Test**: Open the dropdown, search for a language, select it, run a short clip, and confirm output in that language. Pick a language a step does not support and confirm a warning appears before processing.

**Acceptance Scenarios**:

1. **Given** the language dropdown, **When** the user types part of a language name, **Then** matching languages are listed.
2. **Given** a language that the voice step cannot speak, **When** the user selects it, **Then** the app states which step is limited and offers a fallback (for example subtitles only, or a generic voice) before any processing starts.
3. **Given** a newly supported language is added to the engines, **When** the app is reopened, **Then** it appears in the dropdown with no change to the interface.

---

### User Story 3 - Install and run on weak or unsupported computers (Priority: P1)

At install and at first launch the app checks the computer. If it is strong enough it uses local processing. If it is weak but above the minimum, it runs light steps locally and offers cloud processing for heavy steps. If it is below the minimum it stops and shows a clear message.

**Why this priority**: Explicit requirement and a non-negotiable principle in the constitution.

**Independent Test**: Run the check on a machine below minimum (or with a simulated low spec) and read the message. Run it on a mid-spec machine and confirm hybrid mode is chosen.

**Acceptance Scenarios**:

1. **Given** a computer below the minimum, **When** the installer or first launch runs, **Then** the app shows "Your computer is not compatible with this app. You need [exact missing requirement, for example 8 GB RAM] to run this app." and does not continue.
2. **Given** a computer above the minimum but too weak for local heavy steps, **When** the check finishes, **Then** hybrid mode is selected and the user is told which steps need a cloud key.
3. **Given** a strong computer, **When** the check finishes, **Then** all steps run locally and no cloud key is requested.
4. **Given** the same app version, **When** installed on Windows and on macOS, **Then** both perform the same check and show equivalent messages.

---

### User Story 4 - Clone voices and pick speaker profiles (Priority: P2)

The user chooses, per speaker or per line, whether the dubbed voice should clone the original speaker, or use a male or female profile with a short control instruction (for example "a warm, professional narrator").

**Why this priority**: Voice cloning is a key differentiator in the reference tool, but a dub works without it using standard profiles.

**Independent Test**: In a clip with two speakers, assign one cloned voice and one profile voice, synthesize, and confirm the two voices sound different and the cloned one resembles the original speaker.

**Acceptance Scenarios**:

1. **Given** a transcript with detected speakers, **When** the user opens the voice actor column, **Then** each line shows its assigned voice and can be changed.
2. **Given** a speaker profile with an instruction, **When** the user synthesizes, **Then** that instruction is applied to the voice.
3. **Given** voice cloning is requested, **When** the app starts, **Then** it shows a notice that the user must have the right to clone and dub those voices, and records that it was shown.

---

### User Story 5 - Edit transcript, subtitles and timing (Priority: P2)

The user edits lines in a transcription table (start, end, text, voice actor, status), with insert, edit, delete, and search and replace. An audio timeline shows tracks and lets the user adjust speed, volume and pitch, and run Smart Fit and End Sync so dubbed speech matches the original timing. The user can import and export subtitles and apply caption templates.

**Why this priority**: Automatic output is rarely perfect. Editing makes the result usable.

**Independent Test**: Edit one line, re-synthesize only that line, run Smart Fit, and confirm the line fits its time slot. Export subtitles and re-import them.

**Acceptance Scenarios**:

1. **Given** an edited line, **When** the user synthesizes, **Then** only changed lines are regenerated.
2. **Given** dubbed speech longer than its slot, **When** the user runs Smart Fit, **Then** speed is adjusted within a safe range so it fits, and lines that cannot fit are flagged.
3. **Given** a transcript, **When** the user exports subtitles in a standard subtitle format and imports them in another project, **Then** text and times match.
4. **Given** a caption template, **When** applied, **Then** subtitles in the preview and export use its style.

---

### User Story 6 - Video tools and export options (Priority: P3)

The left tool panel offers import media, add soundtrack, import voiceover, vocal separator, mute original audio, aspect ratio, privacy mask, colour adjustment, vignette, and saved presets. Export settings let the user choose full video or audio only, background fill, and file name.

**Why this priority**: Present in the reference tool, but not needed for a working dub.

**Independent Test**: Apply an aspect ratio change and colour preset, export, and confirm both are in the output.

**Acceptance Scenarios**:

1. **Given** an imported video, **When** the user changes aspect ratio with background fill on, **Then** empty areas are filled and the preview matches the export.
2. **Given** a privacy mask over a region, **When** exported, **Then** that region is obscured for the whole chosen range.
3. **Given** export set to audio only, **When** exported, **Then** only the mixed audio file is produced.

---

### User Story 7 - Batch and multi-project work (Priority: P3)

The user keeps several projects open in tabs, queues several videos, and saves and reopens projects.

**Why this priority**: Productivity feature from the reference tool ("many projects at the same time").

**Independent Test**: Queue three clips, close the app mid-run, reopen, and confirm finished items stay finished and the rest resume.

**Acceptance Scenarios**:

1. **Given** several videos queued, **When** processing runs, **Then** each shows its own status and one failure does not stop the others.
2. **Given** a saved project, **When** reopened, **Then** transcript, voices, edits and settings are restored.

---

### User Story 8 - Settings and engines (Priority: P3)

Settings cover general options, theme (light and dark), hotkeys, analytics opt-in, the voice and translation engine (local or cloud), speaker profiles, and the user's own cloud API key.

**Why this priority**: Needed to make hybrid mode usable but not for first success.

**Independent Test**: Add a cloud key, verify the connection from settings, and run a translation through it.

**Acceptance Scenarios**:

1. **Given** a cloud key entered, **When** the user chooses "Verify connection", **Then** the app reports success or the exact failure reason.
2. **Given** the key provider rate-limits a request, **When** it happens mid-run, **Then** the app waits and retries, or tells the user how long to wait, instead of losing work.

---

### User Story 9 - Lip-sync (Priority: P4)

The user can turn on lip-sync so the speaker's mouth movement in the video matches the dubbed speech.

**Why this priority**: Requested by the owner. Very compute-heavy, so it comes after the core dub works.

**Independent Test**: Dub a clip with a visible face, turn lip-sync on, and compare mouth movement to the new speech against the same clip without it.

**Acceptance Scenarios**:

1. **Given** lip-sync is on and the machine can handle it, **When** the user exports, **Then** visible faces move in time with the dubbed speech.
2. **Given** a machine that cannot run lip-sync locally, **When** the user turns it on, **Then** the app offers the cloud option or tells the user what spec is needed, and the normal dub still works.
3. **Given** a scene with no visible face, **When** lip-sync runs, **Then** that scene is left unchanged.

---

### User Story 10 - Real-time dubbing (Priority: P4)

The user plays a video or a live audio source and hears dubbed speech in the target language with a short delay, without processing the whole file first.

**Why this priority**: Requested by the owner. It needs the fastest engines and a strong machine or cloud, so it follows the offline dub.

**Independent Test**: Play a 5-minute clip in real-time mode and measure the delay between original speech and dubbed speech.

**Acceptance Scenarios**:

1. **Given** real-time mode is on, **When** playback starts, **Then** dubbed speech begins within a stated delay (see SC-010) and keeps pace without growing lag.
2. **Given** the machine or connection cannot keep up, **When** lag passes the limit, **Then** the app tells the user and offers subtitles-only or switching to offline dubbing.
3. **Given** real-time mode ends, **When** the user chooses to keep it, **Then** the session is saved as a project for editing.

---

### User Story 11 - Use FlickFluent on Linux, web and mobile (Priority: P5)

The user can run FlickFluent on Linux, in a web browser, and on a phone or tablet, with projects usable across devices.

**Why this priority**: Requested by the owner. Windows and macOS ship first. Each new platform follows once the core is stable.

**Independent Test**: Open the same project on each new platform, and confirm the transcript, voices and export settings are present and a dub can be started.

**Acceptance Scenarios**:

1. **Given** Linux, **When** installed, **Then** the hardware check and all desktop features behave as on Windows and macOS.
2. **Given** the web or mobile version, **When** the device cannot run heavy steps, **Then** those steps use the cloud option or are blocked with a clear message naming the missing requirement.
3. **Given** a project saved on one platform, **When** opened on another, **Then** it loads with no loss of data.

---

### Edge Cases

- Video with no speech, or only music: the app reports no speech found and still allows export.
- Multiple overlapping speakers: the app marks overlap and lets the user assign voices manually.
- Mixed languages in one video: the user can set the language per line.
- Very long movie on a low-spec machine: the app processes in sections and shows an estimated time.
- No internet in hybrid mode when a cloud step is needed: the app says which step is blocked and offers to wait or switch to local.
- Disk space runs out during processing: the app stops cleanly, keeps finished work, and states how much space is needed.
- Unsupported or corrupt video file: clear error naming the problem.
- Translated speech is much longer or shorter than the original: flagged for the user before export.
- Output language has no available voice: warn before processing.
- The user closes the app during processing: no corrupted project, and processing can resume.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST run on Windows and macOS with the same features on both.
- **FR-002**: Users MUST be able to import a video file of any common format and length, short or full-length movie.
- **FR-003**: The app MUST separate speech from background music and keep the music for the final mix.
- **FR-004**: The app MUST transcribe speech with start and end times and detect speakers.
- **FR-005**: The app MUST translate the transcript from the chosen source language to the chosen target language.
- **FR-006**: Users MUST be able to pick source and target language from searchable dropdowns, with an auto-detect option for source.
- **FR-007**: The language list MUST come from the capabilities of the engines in use, so adding a language needs no interface change.
- **FR-008**: The app MUST warn before processing if a chosen language is unsupported by a step, and offer a fallback.
- **FR-009**: The app MUST produce dubbed speech, with options to clone the original speaker's voice or use a male or female profile with a control instruction.
- **FR-010**: The app MUST show a consent notice before first use of voice cloning.
- **FR-011**: The app MUST fit dubbed speech to the original timing and provide Smart Fit, End Sync, and per-line speed, volume and pitch controls.
- **FR-012**: The app MUST mix dubbed speech with background music and allow the original audio to be muted.
- **FR-013**: Users MUST be able to insert, edit, delete, search and replace transcript lines, and re-synthesize only changed lines.
- **FR-014**: The app MUST import and export subtitles in standard subtitle formats and support caption templates.
- **FR-015**: The app MUST provide aspect ratio change with background fill, privacy mask, colour adjustment, vignette, and saved presets.
- **FR-016**: The app MUST export full video or audio only, with a user-chosen file name.
- **FR-017**: The app MUST support batch queues and multiple open projects in tabs, with saved and restorable projects.
- **FR-018**: The app MUST check CPU, memory, free disk and acceleration hardware at install and first launch, and choose full-local, hybrid, or blocked mode.
- **FR-019**: If below the minimum, the app MUST stop and display "Your computer is not compatible with this app. You need [exact missing requirement] to run this app."
- **FR-020**: Light steps (audio extraction, transcription, vocal separation) MUST run locally on any machine above the minimum.
- **FR-021**: Heavy steps (translation, voice cloning) MUST run locally only if the machine can, otherwise via an optional cloud service using the user's own key.
- **FR-022**: Cloud steps MUST be off by default, clearly labelled, and show what data will be sent before the first use.
- **FR-023**: Stored cloud keys MUST NOT be kept in plain text.
- **FR-024**: Long movies MUST be processed in sections so memory use stays bounded, and work MUST be resumable after cancel, crash or close.
- **FR-025**: The app MUST show per-stage progress, estimated time, and clear errors with a next step.
- **FR-026**: Models MUST be downloaded on demand, with the size shown before download.
- **FR-027**: The app MUST provide theme choice (light and dark), hotkeys, and an optional analytics setting that is off by default.
- **FR-028**: Each reused open-source component and its licence MUST be recorded, with a visible licence notice for any component whose licence is not commercial-friendly.
- **FR-029**: The transcript editor MUST also offer merge, split and duplicate of lines, bulk editing of all lines in one dialog, automatic gender detection per speaker, and a live memory-use indicator.
- **FR-030**: Transcription, voice generation and export MUST each be stoppable at any time, with progress shown and finished work kept.
- **FR-031**: The app MUST accept several translation API keys and switch to the next key on rate-limit or failure, retrying without losing work.
- **FR-032**: The app MUST generate title suggestions for a movie from its title and optional subtitle files.
- **FR-033**: The app MUST export the dubbed result as a project that can be opened in common video editors, in addition to a finished video or audio file.
- **FR-034**: The app MUST offer lip-sync that adjusts visible mouth movement to the dubbed speech, run locally where the machine can handle it and otherwise via the cloud option, and MUST leave scenes without faces unchanged.
- **FR-035**: The app MUST offer real-time dubbing of a playing video or live audio, with a bounded delay, and MUST warn and offer fallbacks when the machine or connection cannot keep up.
- **FR-036**: The app MUST run on Linux, in a web browser, and on mobile devices, in later releases after Windows and macOS, with the same project format and the same hardware check and alert (FR-018, FR-019) adapted to each platform.
- **FR-037**: A project saved on one platform MUST open on every other supported platform without data loss.

### Key Entities

- **Project**: A saved dubbing job: source video, languages, transcript, voice assignments, edits, export settings, processing state.
- **Transcript Line**: Start time, end time, original text, translated text, speaker, voice actor, speed, volume, pitch, status.
- **Speaker**: A detected person with a voice assignment (cloned or profile).
- **Speaker Profile**: A named voice setup: gender, control instruction, optional reference audio.
- **Language**: Code, display name, and which steps support it (transcribe, translate, speak).
- **Machine Capability Report**: CPU, memory, free disk, acceleration, chosen mode, and any missing requirement.
- **Queue Item**: A project waiting or running in a batch, with status.
- **Caption Template**: Subtitle look and position.
- **Preset**: Saved video adjustments (aspect ratio, colour, vignette).
- **Engine Configuration**: Local or cloud choice per step, with the user's key reference.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can dub a 2-minute clip from import to exported file in under 10 minutes on a machine at the minimum spec, not counting one-time model downloads.
- **SC-002**: A full-length movie (up to 3 hours) completes on a minimum-spec machine without crashing or running out of memory.
- **SC-003**: At least 90% of first-time users complete a first dub without help.
- **SC-004**: On a machine below the minimum, 100% of installs and launches show the incompatibility message naming the exact missing requirement, with no crash.
- **SC-005**: The same project produces equivalent results on Windows and macOS, and all acceptance scenarios pass on both.
- **SC-006**: Dubbed lines start within 0.3 seconds of the original line start for at least 90% of lines after Smart Fit.
- **SC-007**: Background music remains audible and speech is intelligible in at least 90% of reviewed test clips.
- **SC-008**: Closing the app or losing power mid-run loses no more than the section being processed.
- **SC-009**: Adding a language to the engines makes it appear in the dropdown without any interface update.
- **SC-010**: In real-time mode, dubbed speech starts within 5 seconds of the original and the delay does not grow over a 30-minute session on a machine that meets the real-time spec.
- **SC-011**: With lip-sync on, at least 80% of reviewed clips are judged by viewers as better matched than without it.
- **SC-012**: A project created on one platform opens correctly on every other supported platform in 100% of test cases.

## Assumptions

- Target users are content creators and hobbyists dubbing their own or licensed video; no accounts or sign-in are needed in v1.
- The minimum spec will be defined in the plan after benchmarking light steps on low-end hardware; the check and alert use that single definition.
- "Any language" means every language supported by the selected engines, not a promise of equal quality in all of them.
- Cloud use is optional and uses the user's own key and cost; the app does not provide a hosted service.
- Voice cloning needs a short clean reference sample; the app extracts it from the separated speech.
- Release order (owner decision: all of these are wanted): v1 Windows and macOS offline dubbing (P1 to P3); then lip-sync and real-time dubbing (P4); then Linux, web and mobile (P5). The plan may reorder within this if benchmarks demand it.
- Lip-sync and real-time dubbing are the heaviest features. On low-spec machines they are expected to rely on the cloud option or to be blocked with a message naming the missing requirement. This is a known tension with the low-spec principle, handled by the hybrid design.
- Web and mobile cannot run heavy local models on most devices, so they are expected to be cloud-assisted by default.
- The reference tool is studied for behaviour and layout only; no assets, code or branding are copied.
- The reference review covered the full video. Frames only; no audio transcript, because the speech model has no Khmer support.

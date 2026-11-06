---

description: "Task list for feature 004: Transcript Editor and Timeline"
---

# Tasks: Transcript Editor and Timeline

**Input**: Design documents from `specs/004-transcript-editor-timeline/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VIII asks for test-first on the pipeline stages, and the editor extends synthesize and mix. Write each test, see it fail, then implement. Everything runs against the fake sidecar (`core/tests/support/stub_sidecar.py`, `fake_models_sidecar.py`) and the fake cloud server; real models are needed only for T081.

**Organization**: Grouped by user story. Each story can be built and verified alone after Phase 2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1 to US4, matching `spec.md`

## Path Conventions (from plan.md)

`core/` Rust library, `src-tauri/` Tauri shell, `src/` React UI, `e2e/` Playwright (fake cores in `e2e/support/`), `config/` shared config, `docs/` project docs. New core code lives in `core/src/editor/` and `core/src/subtitles/`; UI in `src/features/editor/`. Labels go in `src/i18n/en.json` under `editor` (the only language file).

## Decisions applied

- Changed mark is derived from `speechHash`, never stored as a flag (research R1). Old projects: empty hash with a clip means unchanged.
- Speed, volume and pitch are an ffmpeg post chain on the raw clip; limits speed 0.85 to 1.30, volume -12 to +6 dB, pitch -4 to +4 semitones, in `core/src/editor/limits.rs` only (R4).
- End Sync changes `dubStartMs` only, never `startMs` (R5).
- SRT and WebVTT only; the caption template styles the preview and the WebVTT export; SRT is plain (R8).
- Undo and redo live in the core, in memory, one entry per bulk action (R9).
- A section the editor changed is marked `edited`: a resume never re-runs it, and a fresh run asks before replacing edited lines (R16, spec FR-020). The editor never rewrites `sec.json` or `translated.json`.
- Voices, models, engine and the cloud amount check come from the shared `EditorRuntime` and `core/src/cloud/guard.rs`, not from a private copy (R17, R18).
- Templates end with Khmer font fallbacks that exist on Windows and macOS; nothing is bundled (R19).
- Windows is carried as for 002 and 003: the feature is not closed before T084 passes on Windows.

---

## Phase 1: Setup

**Purpose**: dependencies, shared data file, test fixtures.

- [X] T001 Add `subtp = "0.2.0"`, `chardetng = "1.0.0"` and `encoding_rs = "0.8.44"` to `core/Cargo.toml`; run `cargo deny check licenses` and add anything new to `deny.toml` if it needs an allow entry (expected: all `MIT OR Apache-2.0`, `encoding_rs` also `BSD-3-Clause`)
- [X] T002 [P] Add `@tanstack/react-virtual` 3.x to `package.json` (pin the exact version) and run the npm licence check the repo already uses (expected MIT)
- [X] T003 [P] Create `config/caption-templates.json` with the three templates from `contracts/caption-templates.md` (`classic`, `soft-box`, `top-note`), each `fontFamily` ending with the Khmer fallbacks `"Noto Sans Khmer", "Khmer UI", "Khmer Sangam MN", sans-serif` (research R19) and add the name keys `editor.captions.classic`, `editor.captions.softBox`, `editor.captions.topNote` to `src/i18n/en.json`
- [X] T004 [P] Create the subtitle fixtures listed in `contracts/subtitle-formats.md` under `core/tests/fixtures/subtitles/`: `plain.srt`, `crlf-bom.srt`, `dot-millis.srt`, `styled.vtt`, `notes-and-regions.vtt`, `latin1.srt` (real Latin-1 bytes), `out-of-order.srt`, `bad-time.srt`, `empty.srt`, `not-a-subtitle.txt`
- [X] T005 [P] Add the new libraries to `docs/dependencies.md` (purpose, version, licence, why chosen) for `subtp`, `chardetng`, `encoding_rs` and `@tanstack/react-virtual`; note that `subtp` is dropped if T048 shows it fails the fixtures

---

## Phase 2: Foundation (blocks every story)

**Purpose**: the data fields, the hash, the session with undo, the commands shell. No user story works without these.

- [X] T006 [P] Write test `core/tests/editor_model.rs`: a project file without any 004 field loads with the defaults of `data-model.md` (`speed` 1.0, `volumeDb` 0, `pitchSemitones` 0, `dubStartMs` absent, `speechHash` empty, `userEdited` false, speaker `genderSource` `detected`) and saves back without losing a 002 or 003 field; `SUPPORTED_FORMAT_VERSION` stays 2
- [X] T007 Add the fields to `TranscriptLine` and `Speaker` in `core/src/project/dub.rs` exactly as in `data-model.md` (`rawAudio`, `rawMs`, `speed`, `volumeDb`, `pitchSemitones`, `dubStartMs`, `speechHash`, `userEdited`; `genderSource`), all with serde defaults; add `captionTemplate` (default `classic`) to `DubData`; add `edited: bool` (default false) to `Section` in the same file (research R16, `data-model.md` section record)
- [X] T008 [P] Create `core/src/editor/limits.rs` with the constants `SPEED_MIN = 0.85`, `SPEED_MAX = 1.30` (equal to `MAX_SPEEDUP` in `core/src/cloud/engine.rs`; import it, do not copy the number), `VOLUME_MIN_DB = -12.0`, `VOLUME_MAX_DB = 6.0`, `PITCH_MIN = -4.0`, `PITCH_MAX = 4.0`, `NEIGHBOUR_MARGIN_MS = 80`, and `clamp`/`in_range` helpers; add `core/src/editor/mod.rs` and `pub mod editor;` in `core/src/lib.rs`
- [X] T009 [P] Write test `core/tests/editor_runtime.rs`: for a project saved by a fake run, `EditorRuntime` picks the same voice per speaker and the same engine id (`local:<model>` or `cloud:<service>`) as the run's own plan; changing the engine in the settings changes the engine id; missing models are reported by name with `MODELS_MISSING`; a project whose work folder is gone still opens (text only) and reports it
- [X] T010 Implement `core/src/editor/runtime.rs` (`EditorRuntime::for_project`): move the code that builds the run configuration and calls `plan_models_full` out of `src-tauri/src/commands/dub.rs` (near `fn config`, project path, sidecar, ffmpeg, models dir, catalogue, machine class) into a shared function in `core/src/pipeline/run.rs` that both `dub_start` and the editor call; expose `voice_for(speaker_id)`, `engine_id(step)`, `work_folder()` and `cloud()`; make T009 pass and keep every `dub_*` test green (research R17)
- [X] T011 [P] Write test `core/tests/editor_changed.rs` (quickstart step 3): editing translated text marks the line changed; editing it back to the original text clears the mark when the old clip is still current; changing speaker, speaker gender, voice id, engine id, speed, volume or pitch marks the line; an old project line with a clip and an empty `speechHash` is unchanged and `editor_open` fills the hash; a line with no clip is changed; speed, volume or pitch alone give `needsPost`, not changed
- [X] T012 Implement `core/src/editor/hash.rs`: `speech_hash(line, speaker, voice, engine) -> String` (voice and engine come from the editor runtime; start and end times are NOT inputs, spec FR-004) (SHA-256 of translated-or-original text, language, voice id, speaker id, gender, engine id, speed, volume, pitch) and `raw_hash` (the same without speed, volume, pitch); `changed_ids(lines, ...)` and `needs_post(...)`; make T011 pass
- [X] T013 [P] Write test `core/tests/editor_ops.rs` part 1 (quickstart step 2): `UpdateLine` with end before start returns `EDIT_END_BEFORE_START` and changes nothing; speed outside 0.85 to 1.30 returns `EDIT_OUT_OF_RANGE`; after every accepted op the project file on disk equals the in-memory lines; undo then redo gives the same lines; a rejected op leaves the undo stack alone; undo with nothing returns `EDIT_NOTHING_TO_UNDO`
- [X] T014 Implement `core/src/editor/ops.rs` (`EditOp` variants from `data-model.md`, each with `apply` returning its inverse) and `core/src/editor/session.rs` (`EditSession`: working lines, undo and redo stacks, autosave through the existing atomic project save, bulk ops as one entry; every accepted op sets `Section.edited` on the sections of the lines it touches, research R16); start with `UpdateLine`, `InsertLine`, `DeleteLines`; make T013 pass
- [X] T015 [P] Add the new error codes `EDIT_END_BEFORE_START`, `EDIT_OUT_OF_RANGE`, `EDIT_CANNOT_MERGE`, `EDIT_NOTHING_TO_UNDO`, `SUBTITLE_PARSE_FAILED`, `SUBTITLE_ENCODING_UNKNOWN`, `SUBTITLE_PREVIEW_EXPIRED`, `EDIT_WORK_FOLDER_MISSING`, `DUB_EDITS_WOULD_BE_LOST` to `core/src/error.rs` and `core/src/pipeline/errors.rs` with `editor.error.*` i18n keys and next-step keys; the existing test that checks every listed error has real keys (`core/tests/errors.rs`) must pass
- [X] T016 Create `src-tauri/src/commands/editor.rs` with `editor_open` (builds the editor runtime, T010), `editor_state`, `editor_update_line`, `editor_insert_line`, `editor_delete_lines`, `editor_undo`, `editor_redo` per `contracts/editor-commands.md` (session kept in app state beside the open project; closing the project drops it); register in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs`; add typed wrappers and types to `src/app/ipc.ts`
- [X] T017 [P] Write test `core/tests/cloud_guard.rs`: the shared guard passes under the allowed amount, returns `CLOUD_CONSENT_REQUIRED` with `projectedChars` above it and writes `cloud-needed.json`, passes when there is no agreement record, and the usage merge adds a step's characters to an existing record; the existing `a_run_that_outgrows_its_agreement_stops_before_sending_and_resumes_after_a_new_one` in `core/tests/cloud_pipeline.rs` stays green
- [X] T018 Extract `guard_cloud_amount` and `merged_usage` from `Runner` in `core/src/pipeline/run.rs` into `core/src/cloud/guard.rs` as plain functions of the consent book, the usage so far, the characters about to be sent, the section end and the video length; the run calls them; add `pub mod guard;` to `core/src/cloud/mod.rs`; make T017 pass (research R18)
- [X] T019 [P] Write test `core/tests/editor_run_interaction.rs` (spec FR-020, research R16): an accepted edit marks the line's section `edited`; resuming a stopped run whose section 0 is `edited` runs no stage for it, takes its lines from the project, keeps the edits and sends nothing to the fake sidecar for it; a fresh start on a project with edited lines returns `DUB_EDITS_WOULD_BE_LOST` with `details.editedLines` and deletes nothing; the same call with `discardEdits: true` runs and clears `edited`; `preflight.editsWillBeLost` equals the count
- [X] T020 Implement the run interaction: in `core/src/pipeline/run.rs` an `edited` section counts as complete on resume (`section_complete` and the stage skip) and its lines are read from the project, not from `sec.json`; a fresh start fails with `DUB_EDITS_WOULD_BE_LOST` unless `discardEdits` is true and resets `edited`; add `editsWillBeLost` to the preflight in `core/src/pipeline/preflight.rs`; add `discardEdits` to `dub_start` in `src-tauri/src/commands/dub.rs` and `src/app/ipc.ts`; make T019 pass

**Checkpoint**: lines carry the new fields, the hash and session work, the basic commands answer.

---

## Phase 3: User Story 1 - Edit lines and re-synthesize only what changed (P1)

**Goal**: edit, insert, delete, search and replace, and re-synthesize only the changed lines; the mix is rebuilt from new and kept clips.

**Independent test**: finish a dub of the 2-minute fixture, edit one line, delete one, insert one, synthesize; only those lines reach the voice engine, every other clip file keeps its bytes, the new mix has the edit.

### Tests first

- [X] T021 [P] [US1] Write test `core/tests/editor_search.rs` (quickstart step 4): query over original, translated or both; match case and whole word; matches carry `(lineId, field, start, end)`; `ReplaceMatches` with a chosen subset changes only those lines and is one undo step; 6,000 lines scan in under 100 ms
- [X] T022 [P] [US1] Write test `core/tests/editor_resynth.rs` (quickstart step 6) on the fake sidecar: after a full fake run, edit one line and call `resynthesize(ids = changed)`; the fake sidecar receives exactly that line; every other clip file is byte-equal; `rawAudio` and `speechHash` are updated only after the clip is complete; deleting a line removes its speech from the mix and regenerates nothing; an inserted line gets speech at its times
- [X] T023 [P] [US1] Extend `core/tests/editor_resynth.rs`: a stop (cancel token) after two of five lines keeps the two swapped clips and leaves three changed; the next call does exactly the three; an edit of a line while the run is going leaves that line changed after its clip is written (the snapshot hash no longer matches); with a cloud engine the 003 guard is called with the characters of the changed lines only and the fake cloud sees no request for unchanged lines (`core/tests/support/fake_cloud.rs`); the characters sent are added to `dub.cloud_usage`; with an agreement of 2 and an estimate that fits it, an editing session whose changed lines exceed 5 times the agreement stops with `CLOUD_CONSENT_REQUIRED` before sending (spec FR-017)
- [X] T024 [P] [US1] Write test `core/tests/editor_splice.rs`: render only the sections that hold a changed line and splice them into the old `dub-mix.wav`; the stretches before and after are sample-equal to the old file; a line moved across a section bound re-renders both sections; output length equals the old length

### Implementation

- [X] T025 [US1] Implement `core/src/editor/search.rs` (plain text search, case and whole-word options, no regular expressions) and the `ReplaceMatches` op in `core/src/editor/ops.rs`; make T021 pass
- [X] T026 [US1] In `core/src/pipeline/mix.rs` add `render_sections(…, only: &[usize])` that reuses the existing per-section render for chosen sections and `splice(old_mix, new_pieces, bounds)` using the ffmpeg concat demuxer with `inpoint`/`outpoint` on the same PCM file (research R3); the full `mix` keeps its behaviour and tests; make T024 pass
- [X] T027 [US1] Implement `core/src/editor/resynth.rs`: `resynthesize(project, ids, cancel, on_event)` — snapshot `(id, speechHash)`; get voices, models and engine from `EditorRuntime`; call the synthesize worker (local) or the cloud speak engine (after the shared guard in `core/src/cloud/guard.rs` with the chars of these lines, and add them to `dub.cloud_usage`); return `EDIT_WORK_FOLDER_MISSING` when the work folder is gone for the list; write each clip under `clips/<id>.<hash8>.wav` and the raw clip as `clips/<id>.raw.wav`; swap `audio`, `audioMs`, `rawAudio`, `rawMs`, `speechHash` in the session only if the line's expected hash still equals the snapshot; remove the old file after the swap; then call `render_sections` and `splice` for the touched sections; emit `progress`, `lineDone`, `finished`; make T022 and T023 pass
- [X] T028 [US1] Make the 002 synthesize stage also write the raw clip and the speech hash for every line it makes (`core/src/pipeline/run.rs`, `apply_clips`), so a fresh run is already editable; keep every 002 and 003 test green
- [X] T029 [US1] Add commands `editor_search`, `editor_replace`, `editor_resynthesize`, `editor_stop` and the `editor:event` stream in `src-tauri/src/commands/editor.rs` per the contract; reuse `CancelToken` and the progress cadence of `commands/dub.rs` (at least every 5 s); add the wrappers to `src/app/ipc.ts`
- [X] T030 [P] [US1] Write Vitest `src/features/editor/useEditor.test.ts` with a fake ipc: open, edit, undo, redo, changed marks follow `changedIds`, a stop event leaves remaining ids marked, an edit during a run is kept
- [X] T031 [US1] Implement `src/features/editor/useEditor.ts` (state from the commands and `editor:event`)
- [X] T032 [US1] Replace the sample table in `src/features/editor/TranscriptionEditor.tsx` with the real one: shadcn `Table`, rows virtualised with `@tanstack/react-virtual`, columns start, end, speaker, original, translated, flags, status; edit cells inline (Input, Textarea, Select for speaker); insert and delete buttons; a "changed" badge; a Synthesize button with the count of changed lines; remove `SAMPLE_LINES` use. Reuse `src/components/ui` only; no hand-rolled equivalents
- [X] T033 [P] [US1] Create `src/features/editor/SearchReplace.tsx` (shadcn Dialog or Popover, Input, Checkbox for case and whole word, match list with a checkbox per match, Replace selected, Replace all) and wire it to the toolbar search button
- [X] T034 [P] [US1] Add the `editor.*` labels and messages for US1 to `src/i18n/en.json` (table headings, changed, synthesize, search and replace, error messages for the new codes)
- [X] T035 [US1] In the dub start flow (`src/features/dub/NewDub.tsx` and `DubScreen.tsx`) when the preflight has `editsWillBeLost` above 0, show a shadcn `Dialog` that says how many edited lines a new run would replace, with Cancel and a confirm button that calls `dub_start` with `discardEdits: true`; resume is not affected; add the labels to `src/i18n/en.json` and a Vitest case beside `src/features/dub/Progress.test.tsx` style tests
- [X] T036 [US1] Write Playwright `e2e/editor.spec.ts` (US1 part) with a new fake core `e2e/support/fakeEditorCore.ts` (same style as `fakeDubCore.ts`): edit a cell and see "changed", undo, search and replace a chosen subset, insert, delete, synthesize and see the count drop, a 6,000 line table that scrolls, and the discard-edits dialog before a new run

**Checkpoint**: US1 works alone: a finished dub can be corrected and re-synthesized line by line.

---

## Phase 4: User Story 2 - Fit speech to the original timing (P1)

**Goal**: Smart Fit, End Sync, and per-line speed, volume and pitch, without calling an engine.

**Independent test**: run Smart Fit on a `tooLong` line and see it fit or stay flagged with a reason; set speed on another line and only that line is re-made.

### Tests first

- [X] T037 [P] [US2] Write test `core/tests/editor_fit.rs` (quickstart step 5): Smart Fit raises speed until the clip fits its window and clears `tooLong`; needing more than 1.30 stops at 1.30, keeps the flag, reason `needsMoreThanLimit`; a clip much shorter than its slot is slowed no lower than 0.85, a clip already inside its slot is left alone; the report counts `fitted` and `stillFlagged`; no sidecar or cloud request is made
- [X] T038 [P] [US2] Extend `core/tests/editor_fit.rs` for End Sync: `dubStartMs = max(previousDubEnd + 80, endMs - clipMs)` and never later than `startMs`; never overlaps the previous line; `startMs` and `endMs` untouched; reason `noRoomBefore` when it cannot move; a line with no free time is reported, not moved
- [X] T039 [P] [US2] Write test `core/tests/editor_post.rs`: the post chain (ffmpeg `atempo`, `volume`, `asetrate`+`aresample`+`atempo` for pitch) on a raw clip gives a clip whose length matches `rawMs / speed` within 20 ms for speeds 0.85 to 1.30, whose level changes by the set dB within 0.5 dB, and whose length does not change with pitch; changing only speed, volume or pitch re-runs the post chain, never the engine, and only that line's clip file changes

### Implementation

- [X] T040 [US2] Implement the post chain in `core/src/editor/resynth.rs` (or `core/src/editor/post.rs`): `make_line_clip(raw, speed, volume, pitch) -> clip` with ffmpeg filters, written under a new name and swapped when complete; used by US1's resynthesis after the engine returns, and alone when only speed, volume or pitch changed; make T039 pass
- [X] T041 [US2] Implement `core/src/editor/fit.rs`: `smart_fit(lines, ids) -> FitReport` using each line's window from `window_ms` in `core/src/pipeline/run.rs` and `rawMs`; `end_sync(lines, ids) -> FitReport`; reasons as in `data-model.md`; effective total speed (engine speed times post speed) in the report; make T037 and T038 pass
- [X] T042 [US2] Make the mix use `dubStartMs` when present, else `startMs` (`core/src/pipeline/mix.rs` `MixLine.start_ms` and its caller in `core/src/pipeline/run.rs`); recompute `overlap`, `tooLong` and `tooShort` flags after any fit or post change; add the case to `core/tests/editor_splice.rs`
- [X] T043 [US2] Add `editor_fit` and the speed, volume, pitch fields of `editor_update_line` (per `contracts/editor-commands.md`, validated against `limits.rs`) in `src-tauri/src/commands/editor.rs` and `src/app/ipc.ts`; the `ApplyFit` op is one undo entry
- [X] T044 [P] [US2] Create `src/features/editor/FitPanel.tsx`: Smart Fit and End Sync buttons for the selected lines or all flagged lines, a summary ("12 fitted, 3 still too long" from the report), per-line Slider controls for speed, volume and pitch within the limits (shadcn `Slider`, `Label`, `Badge` for flags), reason text for lines that cannot fit
- [X] T045 [US2] Replace the placeholder knobs and clips in `src/features/editor/AudioTimeline.tsx` with the real lines: a clip per line on a track per speaker, positioned by `dubStartMs`/`startMs`, width from `audioMs`, flag colour from the theme tokens; selecting a clip selects the table row; remove `SAMPLE_CLIPS` use
- [X] T046 [P] [US2] Add US2 labels and the reason messages to `src/i18n/en.json`
- [X] T047 [US2] Extend `e2e/editor.spec.ts` (US2 part): run Smart Fit and see the summary, a line that cannot fit stays flagged with its reason, End Sync moves a clip in the timeline, change a slider and only that line shows as needing re-make

**Checkpoint**: US1 and US2 work: edited lines can be re-made and fitted.

---

## Phase 5: User Story 3 - Import and export subtitles (P2)

**Goal**: SRT and WebVTT in and out, an import preview with a choice, and caption templates.

**Independent test**: export translated subtitles, import them into a new project of the same video, text and times match; apply a template and see it in the preview and the WebVTT export.

### Tests first

- [X] T048 [P] [US3] Write test `core/tests/subtitles.rs` (quickstart step 1): each fixture in `core/tests/fixtures/subtitles/` parses as `contracts/subtitle-formats.md` says (BOM, CRLF, dot milliseconds, tags dropped, notes and regions skipped, Latin-1 detected and named, out-of-order cues sorted, `bad-time.srt` and `empty.srt` and `not-a-subtitle.txt` refused with `SUBTITLE_PARSE_FAILED` naming the first problem); a generated 6,000 cue file parses in under a second
- [X] T049 [P] [US3] Extend `core/tests/subtitles.rs`: SRT and WebVTT writers match the contract byte for byte on a small transcript (no BOM, numbering, `,` and `.` milliseconds, WebVTT `STYLE` and cue settings from a template, escaping of `&`, `<`, `>`); `source = both` gives original, line break, translated; empty-text lines are skipped; round trip of export then `replaceText` import gives equal text and times to 1 ms (SC-003)
- [X] T050 [P] [US3] Write test `core/tests/editor_import.rs`: preview reports counts, `allMatch`, first 20 differences and the applicable choices; nothing changes before `apply`; `replaceLines` builds lines with speakers by time overlap; `fillByOverlap` fills each line from the cue that overlaps it most and leaves lines with no cue; each apply is one undo step; an expired `previewId` returns `SUBTITLE_PREVIEW_EXPIRED`; a subtitles-only project (no speech models) can import, edit and export (FR-018)
- [X] T051 [P] [US3] Write test `core/tests/caption_templates.rs`: the shipped file validates; a missing field, a colour that is not `#rrggbb` or `#rrggbbaa`, or `sizePercent` outside 2 to 12 is refused; three templates ship; every `nameKey` exists in `src/i18n/en.json`; every template's `fontFamily` names `Noto Sans Khmer`, `Khmer UI` and `Khmer Sangam MN`, and a file without them is refused

### Implementation

- [X] T052 [US3] Implement `core/src/subtitles/read.rs`: encoding (BOM, UTF-8, then `chardetng` guess decoded with `encoding_rs`, name reported, `SUBTITLE_ENCODING_UNKNOWN` when it fails) and parsing SRT and WebVTT with `subtp`; if T048 shows `subtp` cannot meet the fixtures, write the small parser instead and remove the dependency and its `docs/dependencies.md` entry (T005)
- [X] T053 [US3] Implement `core/src/subtitles/write.rs`: move `subtitles_text` and `srt_time` out of `core/src/pipeline/stages.rs` (keep `export_subtitles` calling it, its tests green), add WebVTT with the template; `core/src/subtitles/mod.rs` exports both; make T049 pass
- [X] T054 [US3] Implement `core/src/subtitles/import.rs` (preview, three choices, preview store with expiry) and the `ImportSubtitles` op; make T050 pass
- [X] T055 [US3] Implement `core/src/subtitles/templates.rs` loading and validating `config/caption-templates.json` (bundled with `include_str!` like the catalogue); make T051 pass
- [X] T056 [US3] Add `subtitles_export`, `subtitles_import_preview`, `subtitles_import_apply`, `caption_templates_list`, `caption_template_set` to `src-tauri/src/commands/editor.rs` and `src/app/ipc.ts`; import and export honour `editor_stop` and emit `progress`
- [X] T057 [P] [US3] Create `src/features/editor/SubtitlesDialog.tsx`: tabs Export and Import (shadcn `Dialog`, `Tabs`, `Select`, `RadioGroup`-style `ToggleGroup`); export choices source and format, a note under SRT that it has no style; import file pick, then the preview (counts, differences, encoding used) and the choices; nothing applied before the user confirms
- [X] T058 [P] [US3] Create `src/features/editor/CaptionPreview.tsx`: the current line drawn over the preview frame from the chosen template's fields; replace the sample in `src/features/editor/PreviewMonitor.tsx`; template `Select` in the dialog and toolbar
- [X] T059 [P] [US3] Add US3 labels and messages to `src/i18n/en.json` (subtitle dialog, preview texts, SRT-has-no-style note, template names already in T003)
- [X] T060 [US3] Extend `e2e/editor.spec.ts` (US3 part): export, import preview with a count difference and the three choices, a bad file shows the first problem and changes nothing, template change updates the preview, SRT note visible, a Khmer sample caption renders in the preview in every template

**Checkpoint**: US1 to US3 work: subtitles round-trip and templates apply.

---

## Phase 6: User Story 4 - Bulk tools, speakers and stopping (P3)

**Goal**: merge, split, duplicate, bulk dialog, speaker gender correction, memory indicator, stopping long work.

**Independent test**: split one line, merge two others, change a speaker's gender, start re-synthesis of many lines and stop it half way; finished lines stay, the rest stay changed.

### Tests first

- [X] T061 [P] [US4] Extend `core/tests/editor_ops.rs` (part 2): split divides the text at `textOffset` and the time in proportion, both lines changed, one undo step; merge only for same-speaker neighbours (else `EDIT_CANNOT_MERGE`), texts joined, times combined; duplicate places a copy after the line without overlap; `BulkText` changes only lines whose text differs and is one undo step; each has a working inverse
- [X] T062 [P] [US4] Extend `core/tests/editor_changed.rs`: `SetSpeakerGender` sets `genderSource = user`, marks that speaker's lines changed through the hash, and a later 002 run does not overwrite a user value
- [X] T063 [P] [US4] Extend `core/tests/editor_resynth.rs`: stop during many lines emits `finished { state: "stopped", remainingChangedIds }`; progress events come at least every 5 s with a slow fake sidecar; stopping an import or export in progress leaves the project unchanged (import applies in one step at the end)

### Implementation

- [X] T064 [US4] Implement `MergeLines`, `SplitLine`, `DuplicateLine`, `BulkText`, `SetSpeakerGender` in `core/src/editor/ops.rs`; make T061 and T062 pass; keep the `userEdited` flag so a later run does not overwrite these lines
- [X] T065 [US4] Add the matching commands in `src-tauri/src/commands/editor.rs` and `src/app/ipc.ts` (`editor_merge_lines`, `editor_split_line`, `editor_duplicate_line`, `editor_bulk_text`, `editor_set_speaker_gender`); make T063 pass by completing stop and `finished` handling in `core/src/editor/resynth.rs`
- [X] T066 [P] [US4] Create `src/features/editor/BulkEditDialog.tsx` (shadcn `Dialog` with a `Textarea` of one line per row in the form `id | translated`, a diff count before confirm) and add merge, split (at the caret in the cell), duplicate to the table's row menu (`DropdownMenu`)
- [X] T067 [P] [US4] Add speaker rows with a gender control (`ToggleGroup`: male, female, unknown, with the detected value labelled) to the editor side panel; changing it calls `editor_set_speaker_gender` and shows the count of lines now changed
- [X] T068 [P] [US4] Show the live memory use in the editor header by reusing the existing memory event and component from feature 001 (`src/app/StatusBar.tsx` badge source); do not add a second sampler; update while re-synthesis runs
- [X] T069 [US4] Add a progress bar (`Progress`) and a Stop button to the synthesize area, shown for re-synthesis, import and export, with finished lines kept; after a stop show how many lines are still changed
- [X] T070 [P] [US4] Add US4 labels and messages to `src/i18n/en.json`
- [X] T071 [US4] Extend `e2e/editor.spec.ts` (US4 part): split, merge, duplicate, bulk dialog, change gender and see the changed count, stop a run half way and see the remaining count, memory indicator updates

**Checkpoint**: all four stories work.

---

## Phase 7: Polish and cross-cutting

- [X] T072 [P] Restart test `core/tests/editor_persist.rs` (SC-007): make edits and leave some unsynthesized, drop the session, reopen from the project file; every edit and every changed mark is restored; the undo stacks are empty (spec assumption)
- [X] T073 [P] Edge-case tests in `core/tests/editor_edges.rs` from the spec: empty text keeps the line with a flag; an overlap after an edit sets the flag and moves nothing; source video missing still allows text edits and subtitle export; a removed model is named and offered before synthesis; with the work folder missing, text edits, search, fitting arithmetic and subtitle import and export still work and synthesis returns `EDIT_WORK_FOLDER_MISSING`; a cloud line without consent returns `CLOUD_CONSENT_REQUIRED`
- [X] T074 [P] Performance test `core/tests/editor_perf.rs`: 6,000 lines — open, search, replace all, bulk text, undo each finish within the budgets of `plan.md` (search under 100 ms in the core)
- [X] T075 [P] Run the licence jobs (`cargo deny check licenses`, the npm lockfile check) and update `docs/dependencies.md` and the About notices if the new libraries need one
- [ ] T076 [P] Idle RAM and installer size: confirm the new crates keep the release binary under the 25 MB budget and idle RAM no worse than 003; record in `docs/verification/004.md` (close any running copy of the app first: the single-instance plugin hides a second one)
- [X] T077 Run `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `npx vitest run`, `npx playwright test`; fix anything the new code broke in 001 to 003 tests
- [X] T078 Run Constitution Check against the finished code and record it in `docs/verification/004.md` (Principles I to IX)
- [X] T079 Update `docs/roadmap.md` with the Feature 004 hand-off (built, verified, open) and tick the matching boxes in this file
- [X] T080 Run the quickstart steps 1 to 8 on macOS and record the results in `docs/verification/004.md`
- [ ] T081 [P] Write ignored test `core/tests/editor_real.rs` and run it by hand with real models (quickstart step 9): edit one line of the 2-minute clip, re-synthesize, print the time and the number of clip files that changed (exactly one); listen at the splice join and to pitch +4 and -4 on one line; record the time against SC-001 (under 15 s on the full-local machine) and the listening verdict in `docs/verification/004.md`; run Smart Fit on every flagged line of the real 2-minute run and record fitted and still-flagged counts against SC-002 (lines needing no more than 1.30 must be at least 90% fitted); look at Khmer text in the caption preview and a WebVTT export in each template and record any empty boxes; if `asetrate` pitch sounds bad at the limits, narrow `PITCH_MIN` and `PITCH_MAX` in `core/src/editor/limits.rs`
- [X] T082 [P] Measure SC-005 in the browser with the 6,000 line fixture of `e2e/editor.spec.ts`: matches shown in under 2 s, scroll stays smooth; record in `docs/verification/004.md`
- [ ] T083 Usability check for SC-004 by hand: one person who has not seen the editor fixes one mistranslation in the 2-minute fixture project and hears the change; record the time and what slowed them in `docs/verification/004.md`; if over 2 minutes, record the cause as a bug task before closing
- [ ] T084 Run quickstart steps 1 to 8 on Windows once a Windows machine is available (carried from 002 and 003); record results and any bug in `docs/verification/004.md`. Feature 004 is not closed until this Windows run passes (Constitution I)

---

## Dependencies and order

- Phase 1 has no dependencies; T002 to T005 run beside T001.
- Phase 2 needs Phase 1 for T001 only (T006 to T020 do not use the new crates). T006 to T020 block every story. Inside Phase 2: the editor runtime (T009, T010) comes before `editor_open` (T016); the shared guard (T017, T018) and the run interaction (T019, T020) come after the session (T014) and need nothing from the UI.
- US1 and US2 are both P1. US2 needs the post chain in `resynth.rs` that US1 creates (T027), so build US1 first; after T027 the two stories overlap safely (US2 works in `fit.rs`, `FitPanel.tsx`, `AudioTimeline.tsx`).
- US3 depends only on Phase 2 (plus T001, T003, T004) and can start beside US1: it works in `core/src/subtitles/` and its own dialogs. The `ImportSubtitles` op (T054) needs the ops file from T014.
- US4 depends on T014 and T027 (ops and the run); T068 is independent.
- Phase 7 comes last; T084 waits for a Windows machine.

## Parallel examples

- After Phase 2: US1 core (T021 to T028) beside US3 core (T048 to T055) by two workers.
- Inside US1 tests: T021, T022, T023, T024 are different files and run together.
- Inside US3 tests: T048, T049, T050, T051.
- UI tasks marked [P] in each story (labels, dialogs, preview) beside the core work once the command contract in `contracts/editor-commands.md` is fixed.

## Implementation strategy

1. **MVP**: Phase 1, Phase 2, US1 (T001 to T036). A finished dub can be corrected and re-made line by line, which is the smallest usable editor.
2. Add US2 (fit), then US3 (subtitles), then US4 (bulk and stop). Each leaves the app working.
3. Keep every 001 to 003 test green at each checkpoint (T028 and T042 touch the 002 stages and mix).
4. Close with Phase 7. The real-model run (T081) and Windows (T084) are manual and are the two open gates.

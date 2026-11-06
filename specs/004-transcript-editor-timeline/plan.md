# Implementation Plan: Transcript Editor and Timeline

**Branch**: `004-transcript-editor-timeline` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-transcript-editor-timeline/spec.md`

## Summary

Turn the editor placeholders from feature 001 into a working editor on top of the finished dub from feature 002. A new core module `core/src/editor/` holds all rules: edit operations with undo and redo, the derived "changed" mark (a hash of what a clip was made from), search and replace, Smart Fit and End Sync, and the subtitle import and export. Re-synthesis reuses the 002 synthesize worker and the 003 cloud engine for just the changed lines, swaps each clip in only when complete, and rebuilds the mix by re-rendering only the touched sections and splicing them into the existing mix file. Speed, volume and pitch are an ffmpeg post chain on the raw clip, so they cost no engine call and no cloud characters. Edited sections are marked in the run record so a later resume keeps them and a fresh run asks before replacing them (R16); voices, models and the cloud amount check come from a shared editor runtime and guard (R17, R18). The UI replaces the sample data in `src/features/editor` with a virtualised table, a fitting and subtitles toolbar, a caption preview and the memory indicator, using the shadcn parts already installed.

## Technical Context

**Language/Version**: Rust 1.80+ (all editor logic), TypeScript 5.x with React (views). Python sidecar unchanged.

**Primary Dependencies**: Existing: Tauri 2, tokio, serde, sha2, ffmpeg (bundled build), shadcn/ui parts. New: `subtp` 0.2.0 (SRT and WebVTT parse, kept only if its tests pass, R6), `chardetng` 1.0.0, `encoding_rs` 0.8.44 (encoding), `@tanstack/react-virtual` 3.x (row virtualisation). All permissive (MIT or Apache-2.0; `encoding_rs` adds BSD-3-Clause); to be confirmed with `cargo deny` and the npm licence check, then recorded in `docs/dependencies.md`.

**Storage**: Lines in `project.json` gain optional fields (data model); no format bump. Clips and the mix in the project's work folder. `config/caption-templates.json` for templates. No new app-data file.

**Testing**: `cargo test` first (subtitle fixtures, edit ops, changed marks, search, fitting, re-synthesis against the fake sidecar and fake cloud), then Vitest and Playwright with a fake core; an ignored real-model test run by hand (`editor_real`).

**Target Platform**: Windows 10 1809+ and macOS 12+ (x64 and arm64), same behaviour (Constitution I). Nothing new is platform-specific: ffmpeg filters used (`atempo`, `volume`, `asetrate`, `aresample`) exist in the shipped builds.

**Project Type**: desktop-app (Rust core + Tauri shell + React UI), unchanged.

**Performance Goals**: one edited line re-synthesised and audible in under 15 s on a full-local machine (SC-001); search over 6,000 lines shows matches in under 2 s with a responsive window (SC-005); Smart Fit on a line under 1 s; the table scrolls smoothly at 6,000 rows.

**Constraints**: a rejected edit changes nothing; disk equals memory after every edit; no clip is overwritten before its replacement is complete; no whole-film load into memory (the splice reads the mix by time ranges); installer still under 25 MB; no change to the sidecar protocol; cloud rules of 003 apply to changed lines only; a section the editor changed is never re-run by a resume.

**Scale/Scope**: up to about 6,000 lines per film; 4 speakers typical; 3 caption templates; 2 subtitle formats. About 20 Tauri commands, one new core module, one new UI feature folder (replacing placeholders), 7 error codes, 1 data file.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Windows and macOS first | PASS, with a standing gap | All logic is shared Rust and web code; no platform branch is added. CI runs the fake-sidecar tests on both. Windows has still not run 002 or 003, so 004 cannot close before it does (carried, same wording as 003). |
| II. Low-spec | PASS | Re-synthesis is per line, the mix is re-rendered per section and spliced, the table is virtualised: memory stays bounded on a long film. Smart Fit, speed, volume and pitch run on the raw clip with ffmpeg and need no heavy model. |
| III. Install-time check and alert | N/A | No change to the hardware check. |
| IV. Any language from a dropdown | PASS | Editor labels come from the language files; no language is named in code. Caption fonts are in the template data and end with Khmer fallbacks that exist on both systems (R19); another script is a data change in the template file, not a code change. |
| V. Reuse open source | PASS | Subtitle parse (`subtp`), encoding (`chardetng`, `encoding_rs`), time-stretching (ffmpeg `atempo`), row virtualisation (`@tanstack/react-virtual`), shadcn parts for every control. Written by us: edit rules, hash, fitting arithmetic, SRT and WebVTT writers, splice. Recorded in `docs/dependencies.md`. |
| VI. Licence discipline | PASS | New crates and package are MIT or Apache-2.0 (`encoding_rs` also BSD-3-Clause). No non-commercial or copyleft component is added. |
| VII. Voice cloning consent | N/A | No cloning here (feature 005). |
| VIII. Test-first | PASS | Subtitle fixtures and edit-rule tests are written before the code (quickstart steps 1 to 6). The editor extends the synthesize and mix stages, whose contract tests stay green. |
| IX. Privacy and local-first | PASS | Editing is local. Re-synthesis through a cloud step uses the 003 consent and amount guard for the changed lines' characters; subtitle files are read and written only where the user points. Nothing new leaves the machine. |

No violations. Complexity Tracking is empty.

**Re-check after Phase 1**: unchanged. The design adds no new network path and no platform-specific code. Three risks are recorded and have verify steps: edits colliding with a later run (R16, tested in `editor_run_interaction.rs`); pitch quality of `asetrate` (R4, listening check in quickstart step 9) and `subtp` robustness (R6, fixtures first).

## Project Structure

### Documentation (this feature)

```text
specs/004-transcript-editor-timeline/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── editor-commands.md
│   ├── subtitle-formats.md
│   └── caption-templates.md
├── checklists/requirements.md
└── tasks.md            (made by /speckit-tasks)
```

### Source Code (repository root)

```text
core/src/editor/            # new
├── mod.rs
├── session.rs              # EditSession: working lines, undo and redo stacks, autosave
├── ops.rs                  # EditOp and its inverse
├── hash.rs                 # speech input hash, changed set
├── search.rs
├── fit.rs                  # Smart Fit, End Sync, limits
├── resynth.rs              # per-line synthesis, post chain, swap, splice
├── runtime.rs              # voices, models, engine and run config for an open project (R17)
└── limits.rs               # the numbers in data-model.md, in one place
core/src/subtitles/         # new
├── mod.rs
├── read.rs                 # parse, encoding
├── write.rs                # SRT, WebVTT (moves subtitles_text out of pipeline/stages.rs)
└── import.rs               # preview and the three choices
core/src/cloud/guard.rs     # new: the amount guard and usage merge shared by a run and the editor (R18)
core/src/pipeline/mix.rs    # keep: add render of chosen sections and the splice
core/src/pipeline/run.rs    # edited sections count as complete on resume; fresh run confirms (R16)
core/src/project/dub.rs     # line and speaker fields
core/tests/                 # editor_*.rs, subtitles.rs, fixtures/subtitles/
src-tauri/src/commands/editor.rs     # new commands (contracts/editor-commands.md)
config/caption-templates.json
src/features/editor/        # placeholders become real
├── TranscriptionEditor.tsx # virtualised table, cell editing, toolbar
├── SearchReplace.tsx
├── BulkEditDialog.tsx
├── FitPanel.tsx            # Smart Fit, End Sync, per-line sliders
├── SubtitlesDialog.tsx     # import preview, export, template choice
├── CaptionPreview.tsx
├── useEditor.ts            # state from editor_* commands and events
└── *.test.tsx
e2e/editor.spec.ts
e2e/support/fakeEditorCore.ts
docs/verification/004.md
```

**Structure Decision**: the existing three-layer layout (core crate, Tauri commands, React feature folder). Editor rules stay in the core so the UI is a view and everything is testable without a window. `pipeline/stages.rs` keeps calling the same SRT writer through the new `subtitles` module.

## Complexity Tracking

None.

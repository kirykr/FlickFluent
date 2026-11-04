# Implementation Plan: Core Dub Pipeline (Local)

**Branch**: `002-core-dub-pipeline` (no git branch) | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-core-dub-pipeline/spec.md`

## Summary

Add the local dubbing pipeline to the shell from feature 001. The Rust core owns the job: it splits the movie into sections, runs seven stages (extract, separate, transcribe, translate, synthesize, mix, export), saves every result to disk as it goes, and resumes from the last good section. The ML work runs in a Python sidecar that the core starts and talks to over stdin/stdout JSON lines. The sidecar runtime, ffmpeg and every model are downloaded on demand with sizes shown first, so the installer stays small. A benchmark command measures real models on a machine and writes records; those records replace the provisional numbers in `config/spec-definition.json`. The language list is built from capability data shipped in the model catalogue, never hard-coded in the UI.

## Technical Context

**Language/Version**: Rust 1.80+ (core orchestration), TypeScript 5.x (UI), Python 3.11 (sidecar, ML only)

**Primary Dependencies**: Existing: Tauri 2, React 18, tokio, serde, sysinfo, reqwest, sha2, zip. New: ffmpeg (LGPL build, subprocess), faster-whisper (CTranslate2), Demucs (htdemucs), CTranslate2 with M2M100 for translation, sherpa-onnx with Piper and Kokoro voices for speech, `silero-vad` (speech and silence cuts). See research R1 to R8.

**Storage**: `.flick` project file (ZIP, bumped to format 2: adds transcript, voices, run state). Large audio and intermediate files live in a work folder next to the project (`<name>.flick-work/`), because they must not sit inside a ZIP that is rewritten on save. Models, sidecar runtime and ffmpeg live in the models folder from 001.

**Testing**: `cargo test` (stage contracts and orchestration, test-first), `pytest` (sidecar workers), Vitest (UI), Playwright (end-to-end on a 20 s sample clip). Windows and macOS CI matrix. A pre-recorded fixtures folder holds the sample clip and expected stage outputs.

**Target Platform**: Windows 10 1809+ and macOS 12+ (x64 and arm64). Stage workers must run CPU-only; GPU is an optional speed-up behind the same interface.

**Project Type**: desktop-app (Rust core + Tauri shell + React UI + managed Python sidecar)

**Performance Goals**: 2-minute clip dubbed in under 10 minutes on the minimum spec (SC-001); progress update at least every 5 s per running stage (SC-009); UI stays responsive during a run

**Constraints**: Memory ceiling set per machine class and enforced by section size (the whole movie is never in RAM); installer still under 25 MB; no network except user-approved downloads; identical behaviour on both platforms; at most one section of work lost on interruption (SC-003)

**Scale/Scope**: Movies up to 3 hours; about 3,000 to 6,000 transcript lines; 30 to 100 languages in the dropdown depending on installed models; 3 new screens (new dub, run progress, language and model chooser)

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Windows and macOS first | PASS | Every stage has a contract test run in the CI matrix on both. Sidecar runtime ships per platform and architecture. CPU path is the baseline; accelerators are optional (R9). Windows still needs a real-machine run (carried from 001). |
| II. Low-spec | PASS | Light steps (extract, separate, transcribe) always local with the model size picked from the machine class. Section size bounds memory. Heavy steps (translate, synthesize) use small int8 models that run on CPU. Where a machine truly cannot run them, the app names the missing requirement; cloud arrives in 003. |
| III. Install-time check and alert | PASS | Reused unchanged from 001. Benchmarks in this feature replace the provisional spec values, with the "provisional" mark removed only when records back every number. |
| IV. Any language from dropdown | PASS | Language list = union of capabilities in the model catalogue (`config/model-catalogue.json`), read at run time. Per-step support is shown before processing and unsupported choices get a fallback (R6, contract `language-capabilities`). |
| V. Reuse open source | PASS | ffmpeg, faster-whisper, Demucs, CTranslate2, M2M100, sherpa-onnx, silero-vad. Only orchestration, resume logic, mixing rules and UI are written. Recorded in `docs/dependencies.md`. |
| VI. Licence discipline | PASS with note | Default models are MIT, Apache-2.0 or BSD. NLLB weights (CC-BY-NC) are offered only as an optional, isolated, clearly labelled extra. Piper voices are chosen per voice, permissive only. Khmer speech has no permissive voice found yet (R6 note); flagged to owner. |
| VII. Voice cloning consent | N/A here | No cloning in this feature (feature 005). |
| VIII. Test-first | PASS | Stage contract tests are written before each stage (see quickstart and contracts/stage-contracts.md). A stage is done only when its test passes on both platforms. |
| IX. Privacy, local-first | PASS | No cloud step. Media never leaves the machine. Downloads are only model, runtime and ffmpeg files the user approves. |

No violations. Complexity Tracking lists one justified item.

## Project Structure

### Documentation (this feature)

```text
specs/002-core-dub-pipeline/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ipc-commands.md
│   ├── sidecar-protocol.md
│   ├── stage-contracts.md
│   ├── project-file-format-v2.md
│   └── language-capabilities.schema.json
├── checklists/requirements.md
└── tasks.md             # created later by /speckit-tasks
```

### Source Code (repository root)

```text
config/
├── spec-definition.json         # final values after benchmarks (provisional mark removed)
└── model-catalogue.json         # real models, each with step, languages, licence, machine class

core/src/
├── pipeline/                    # NEW: run orchestrator, stage graph, sections, resume, progress, cancel
│   ├── run.rs  stages.rs  sections.rs  state.rs  progress.rs  mix.rs
├── sidecar/                     # NEW: start, health, JSON-line protocol, restart on crash
├── languages/                   # NEW: build language list and per-step support from the catalogue
├── bench/                       # NEW: benchmark runner and record writer
├── project/                     # CHANGED: format 2 + migration from format 1
└── models/                      # CHANGED: runtime pack, ffmpeg and model entries; capability fields

sidecar/                         # NEW: Python workers, ML only, no orchestration logic
├── flickfluent_sidecar/{main.py, protocol.py, workers/{separate,transcribe,translate,synthesize,detect_language}.py}
├── tests/
└── requirements.lock            # pinned, with hashes

src-tauri/src/commands/          # CHANGED: dub_*, languages_*, bench_* commands (thin)
src/features/dub/                # NEW: new-dub form, language picker, progress, result
src/i18n/en.json                 # CHANGED: new strings
e2e/                             # CHANGED: dub flow spec
fixtures/                        # NEW: short sample clips and expected outputs (tiny, MIT or self-made)
docs/{dependencies.md, verification/002.md, verification/002-benchmarks.md}
```

**Structure Decision**: Orchestration, resume and file safety stay in Rust (`core/`) where 001 already has atomic writes, locking and downloads. Python is limited to model calls, so replacing a worker or porting to another platform later (feature 010) does not touch the job logic. The sidecar speaks a small, versioned line protocol so it can also run on a remote machine in feature 003 or 010.

## Complexity Tracking

| Item | Why needed | Simpler option rejected because |
|------|------------|---------------------------------|
| Second language runtime (Python sidecar) | faster-whisper, Demucs, CTranslate2 and sherpa-onnx are mature Python/C++ tools; rebuilding them in Rust breaks Principle V | Rust-only bindings exist for some (whisper.cpp, ct2rs) but not Demucs; would mean writing and maintaining ML glue per model. Runtime is downloaded on demand to protect the installer budget. |

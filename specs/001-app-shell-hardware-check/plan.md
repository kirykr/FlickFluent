# Implementation Plan: App Shell and Hardware Check

**Branch**: `001-app-shell-hardware-check` (no git branch) | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-app-shell-hardware-check/spec.md`

## Summary

Build the desktop foundation for FlickFluent: a small Tauri 2 app (Rust core, web UI) that runs the hardware check and picks a mode, stores projects in a portable versioned file, keeps settings, downloads models safely, and shows memory use. No dubbing yet. Python and the ML pipeline arrive in feature 002; this feature leaves a clean place for them but does not add them.

## Technical Context

**Language/Version**: Rust 1.80+ (stable, core), TypeScript 5.x (UI)

**Primary Dependencies**: Tauri 2 (shell, installer bundling, later mobile), React 18 + Vite (UI), `sysinfo` (CPU, RAM, disk, process memory), `wgpu` (GPU adapter listing only), `reqwest` + `sha2` (resumable downloads and integrity), `zip` + `serde_json` (project file), `i18next` (UI text catalogue), `tauri-plugin-single-instance`

**Storage**: Local files only. Project = single `.flick` file (ZIP container with a versioned JSON manifest). Settings = JSON in the OS app-config folder. Models = files in a user-chosen models folder with a JSON index.

**Testing**: `cargo test` (core, contract tests first), Vitest (UI), Playwright against the built app for end-to-end; CI matrix on Windows and macOS

**Target Platform**: Windows 10 1809+ and macOS 12+ now; structure must not block Linux, web, mobile (feature 010)

**Project Type**: desktop-app

**Performance Goals**: hardware check under 10 s (SC-002); memory indicator refresh every 2 s; UI idle CPU under 2%

**Constraints**: installed app (no models) under 25 MB on disk and under 150 MB RAM idle; no network needed except for model downloads; every behaviour identical on both platforms

**Scale/Scope**: about 5 screens (compat result, projects, settings, models, status bar); tens of projects; model catalogue of tens of entries

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Windows and macOS first | PASS | Tauri 2 builds both; CI matrix runs every test on both; no platform-only code without a shared interface. Linux, web, mobile not blocked: core is a library, UI is plain web. |
| II. Low-spec | PASS | Tauri uses the OS web view, so installed size and idle RAM stay small (budget above). Heavy ML is out of this feature. |
| III. Install-time check and alert | PASS with note | First-launch check is authoritative on both platforms. Windows installer also runs the check through a command-line mode and aborts install if blocked. macOS drag-and-drop install has no install hook, so macOS alerts at first launch. See research R5. |
| IV. Any language from dropdown | N/A here | No language selection in this feature. UI text is kept in a catalogue so UI translation is possible later. |
| V. Reuse open source | PASS | Tauri, sysinfo, wgpu, reqwest, zip, i18next. Only glue and UI written. Recorded in `docs/dependencies.md`. |
| VI. Licence discipline | PASS | All chosen crates and packages are MIT or Apache-2.0 (verified in research R8; re-verify when adding). Model catalogue shows licence before download. |
| VII. Voice cloning consent | N/A here | Feature 005. |
| VIII. Test-first | PASS | Contract tests for check, project file, downloader written before code (see quickstart). |
| IX. Privacy, local-first | PASS | No network except user-started downloads. Analytics off by default. No keys stored in this feature. |

No violations. Complexity Tracking not needed.

## Project Structure

### Documentation (this feature)

```text
specs/001-app-shell-hardware-check/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ipc-commands.md
│   ├── project-file-format.md
│   └── spec-definition.schema.json
└── tasks.md             # created later by /speckit-tasks
```

### Source Code (repository root)

```text
config/
└── spec-definition.json         # the one minimum spec and full-local spec (FR-003)

core/                            # Rust library: all logic, no UI, reusable by later platforms
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── hardware/                # probe, evaluate against spec, mode, message text
│   ├── project/                 # .flick read/write, versioning, backup, recovery, lock
│   ├── settings/                # settings model, atomic save, hotkey conflicts
│   ├── models/                  # catalogue, downloader, verify, storage
│   └── memory/                  # memory sampler
└── tests/                       # contract + integration tests

src-tauri/                       # thin Tauri shell: wires core to UI commands/events
├── Cargo.toml
├── tauri.conf.json
├── installer/                   # Windows installer hook that runs the check
└── src/main.rs

src/                             # React UI
├── app/                         # shell, routing, theme
├── features/{compat,projects,settings,models,status}/
├── i18n/en.json
└── tests/

e2e/                             # Playwright specs
docs/dependencies.md             # reused components + licences (Principle V, VI)
```

**Structure Decision**: Logic lives in a UI-free Rust library (`core/`) so the same code can serve desktop now and Linux, web and mobile later. `src-tauri/` only connects it to the window. The React UI is plain web code so it can be reused on web and mobile.

## Complexity Tracking

No violations to justify.

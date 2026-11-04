---

description: "Task list for feature 001: App Shell and Hardware Check"
---

# Tasks: App Shell and Hardware Check

**Input**: Design documents from `specs/001-app-shell-hardware-check/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VIII requires test-first: write each test, see it fail, then implement.

**Organization**: Grouped by user story. Each story can be built and verified alone after Phase 2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1 to US5, matching `spec.md`

## Path Conventions (from plan.md)

`core/` Rust library, `src-tauri/` Tauri shell, `src/` React UI, `e2e/` Playwright, `config/` shared config, `docs/` project docs.

---

## Phase 1: Setup

- [X] T001 Create the folder layout from plan.md: `core/src/{hardware,project,settings,models,memory}`, `core/tests/fixtures`, `src-tauri/installer`, `src/{app,features,i18n}`, `e2e`, `config`, `docs`
- [X] T002 Create the Cargo workspace in `Cargo.toml` at the repo root with members `core` and `src-tauri`
- [X] T003 Create `core/Cargo.toml` with dependencies `sysinfo`, `wgpu`, `reqwest` (stream), `tokio`, `sha2`, `zip`, `serde`, `serde_json`, `thiserror`, `uuid`, `time`
- [X] T004 Scaffold Tauri 2 with React 18, Vite and TypeScript: `package.json`, `vite.config.ts`, `tsconfig.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src/main.tsx`
- [X] T005 [P] Add lint and format config: `rustfmt.toml`, `clippy` settings in `Cargo.toml`, `.eslintrc.cjs`, `.prettierrc`
- [X] T006 [P] Add CI in `.github/workflows/ci.yml`: matrix `windows-latest` and `macos-latest` running `cargo test`, `npm test`, `npm run e2e`, `cargo deny check`, and an npm licence check
- [X] T007 [P] Create `docs/dependencies.md` with a table (component, purpose, version, licence, why chosen) and fill the rows for the chosen stack (Principles V and VI)
- [X] T008 Verify every item marked "verify" in `specs/001-app-shell-hardware-check/research.md` (WebView2 on Windows 10 1809, Tauri 2 NSIS hook syntax, each dependency licence) against current docs, and update `research.md` and `docs/dependencies.md` with what was found

---

## Phase 2: Foundational (blocks all stories)

**CRITICAL**: no story starts until this phase is done.

- [X] T009 Create `config/spec-definition.json` with the provisional values from research R3 (`"provisional": true`) and a test `core/tests/spec_definition.rs` that validates it against `specs/001-app-shell-hardware-check/contracts/spec-definition.schema.json`
- [X] T010 [P] Create `core/src/error.rs`: `AppError` with `code`, `message_key`, `details` and the stable codes `FORMAT_TOO_NEW`, `DAMAGED`, `READ_ONLY`, `HOTKEY_CONFLICT`, `NO_SPACE`, `OFFLINE`, `HASH_MISMATCH` from `contracts/ipc-commands.md`
- [X] T011 [P] Create `src/i18n/en.json` and `src/i18n/index.ts` (i18next); all user-visible text goes through keys (FR-020)
- [X] T012 Create `core/src/lib.rs` exporting the modules, and `src-tauri/src/main.rs` with the command registry skeleton and `tauri-plugin-single-instance`
- [X] T013 [P] Create the UI shell in `src/app/App.tsx`, `src/app/routes.tsx` and `src/app/StatusBar.tsx` (empty slot for the memory badge)
- [X] T014 [P] Create fake-machine fixtures `core/tests/fixtures/low.json`, `mid.json`, `high.json`, `unknown-gpu.json`

**Checkpoint**: foundation ready.

---

## Phase 3: User Story 1 - Compatibility check at install and first launch (P1) MVP

**Goal**: below-minimum machines are blocked with the exact message; others get hybrid or full local mode.

**Independent Test**: `flickfluent --check-hardware --fake-machine <fixture>` gives exit 10 and the exact message for `low.json`, exit 0 with `hybrid` for `mid.json`, exit 0 with `fullLocal` for `high.json`.

### Tests (write first, must fail)

- [X] T015 [P] [US1] `core/tests/hardware_evaluate.rs`: blocked, hybrid, full local, and an unknown value goes to hybrid (rules in `data-model.md`)
- [X] T016 [P] [US1] `core/tests/blocked_message.rs`: message equals `Your computer is not compatible with this app. You need 8 GB RAM (found 4 GB) to run this app.` form, listing every missing item
- [X] T017 [P] [US1] `core/tests/cli_check.rs`: `--check-hardware` prints report JSON and exits 0 or 10

### Implementation

- [X] T018 [US1] `core/src/hardware/probe.rs`: read CPU bits and cores, RAM, free disk on the app drive, OS version via `sysinfo`, GPU adapter list via `wgpu`; take an injectable machine input so fixtures can replace real hardware; unreadable values become `unknown`, never a panic
- [X] T019 [US1] `core/src/hardware/evaluate.rs`: build `MachineCapabilityReport` per `data-model.md`: any `belowMinimum` gives `blocked`, else any `belowFull` or `unknown` gives `hybrid`, else `fullLocal`
- [X] T020 [US1] `core/src/hardware/message.rs`: build the blocked message from `missing[]` only
- [X] T021 [US1] CLI mode in `src-tauri/src/main.rs`: `--check-hardware`, JSON on stdout, exit 0 or 10; `--fake-machine <file>` only in debug and test builds
- [X] T022 [US1] Tauri commands `run_compat_check` and `get_last_compat_report` in `src-tauri/src/commands/hardware.rs`
- [X] T023 [US1] UI `src/features/compat/CompatScreen.tsx` and a first-launch gate in `src/app/App.tsx`: blocked screen with no way to continue, hybrid notice naming which later features need a cloud key, nothing for full local
- [X] T024 [US1] Windows installer hook `src-tauri/installer/check-hardware.nsh` wired in `src-tauri/tauri.conf.json`: runs `--check-hardware` and aborts install with the message on exit 10 (macOS relies on T023)
- [X] T025 [US1] `e2e/compat.spec.ts`: first launch with each fixture shows the right screen on both platforms

**Checkpoint**: US1 works alone.

---

## Phase 4: User Story 2 - Create, save and reopen projects (P1)

**Goal**: a single portable `.flick` file that opens on both platforms, with backups and recovery.

**Independent Test**: create, save, close, reopen; copy to the other OS and open; kill the app mid-edit and recover.

### Tests (write first, must fail)

- [X] T026 [P] [US2] `core/tests/project_roundtrip.rs`: save and reopen with a Khmer name and a Chinese name; contents identical
- [X] T027 [P] [US2] `core/tests/project_versioning.rs`: `formatVersion` newer than supported gives `FORMAT_TOO_NEW` and the file bytes are unchanged; unknown keys survive a save
- [X] T028 [P] [US2] `core/tests/project_recovery.rs`: damaged file reports `DAMAGED` with `backupAvailable`; atomic save leaves no half file; only the last 3 backups are kept; `.recover` file offered after a simulated crash
- [X] T029 [P] [US2] `core/tests/project_lock.rs`: second opener is read-only; stale lock is ignored

### Implementation

- [X] T030 [US2] `core/src/project/format.rs`: read and write the ZIP with `manifest.json` and `project.json` per `contracts/project-file-format.md`
- [X] T031 [US2] `core/src/project/save.rs`: write temp file, flush, rename; rotate `.bak.1` to `.bak.3`
- [X] T032 [US2] `core/src/project/recover.rs`: write `<name>.flick.recover` every 30 s while dirty, delete on clean save or close, restore from recover or backup
- [X] T033 [US2] `core/src/project/lock.rs`: `.flick.lock` sidecar with process ID
- [X] T034 [US2] `core/src/project/recent.rs`: recent list, max 20, mark missing paths unavailable
- [X] T035 [US2] Commands `project_new`, `project_open`, `project_save`, `project_recover`, `project_close`, `project_recent`, `project_delete` and event `project:dirty` in `src-tauri/src/commands/project.rs`
- [X] T036 [US2] UI `src/features/projects/ProjectsScreen.tsx` with new, open, save, recent list, read-only banner, recovery dialog, newer-version message
- [X] T037 [US2] `e2e/projects.spec.ts`: create, save, close, reopen; recovery after killing the app

**Checkpoint**: US1 and US2 work alone and together.

---

## Phase 5: User Story 3 - Settings (P2)

**Goal**: general options, light and dark theme, hotkeys with conflict detection, analytics off by default.

**Independent Test**: change theme and one hotkey, restart, both persist; fresh install has analytics off.

- [X] T038 [P] [US3] `core/tests/settings.rs`: defaults (`analyticsEnabled` is false, `theme` is `system`); atomic save; corrupted file gives defaults plus a notice; hotkey conflict returns `HOTKEY_CONFLICT` with the clashing action; reset leaves projects untouched
- [X] T039 [P] [US3] `core/src/settings/model.rs`: `Settings` per `data-model.md`
- [X] T040 [US3] `core/src/settings/store.rs`: load and atomic save in the OS config folder
- [X] T041 [US3] `core/src/settings/hotkeys.rs`: unique key combos and conflict detection
- [X] T042 [US3] Commands `settings_get`, `settings_set`, `settings_reset` and event `settings:changed` in `src-tauri/src/commands/settings.rs`
- [X] T043 [P] [US3] `src/app/theme.css`: light and dark variables plus follow-system
- [X] T044 [US3] UI `src/features/settings/SettingsScreen.tsx`: general, theme, hotkeys, analytics (shows what is collected when turned on), reset, and a "Re-run compatibility check" button (FR-007 of this feature) calling `run_compat_check`
- [X] T045 [US3] `e2e/settings.spec.ts`: theme and hotkey persist across restart; analytics off on a fresh profile

---

## Phase 6: User Story 4 - On-demand model downloads (P2)

**Goal**: show size first, download with pause and resume, verify, store, delete.

**Independent Test**: start, pause, quit, relaunch, resume, hash verified; a download larger than free space is refused.

- [X] T046 [P] [US4] `core/tests/downloader.rs` using a local test HTTP server: resume after interruption, hash mismatch deletes the file, `NO_SPACE` with needed and free bytes, `OFFLINE`, server without Range restarts from zero
- [X] T047 [P] [US4] `core/src/models/catalogue.rs` and `config/model-catalogue.json`: entries with `name`, `purpose`, `licence`, `sizeBytes`, `url`, `sha256`; ship one small test entry only (real models come with feature 002)
- [X] T048 [US4] `core/src/models/downloader.rs`: streaming download to `.part`, HTTP Range resume, pause, cancel, free-space check before start
- [X] T049 [US4] `core/src/models/verify.rs`: SHA-256 check; on mismatch delete and reset to `notInstalled`
- [X] T050 [US4] `core/src/models/storage.rs`: JSON index of installed models, sizes, delete
- [X] T051 [US4] Commands `models_catalogue`, `model_download_start`, `model_download_pause`, `model_download_resume`, `model_download_cancel`, `model_delete` and event `model:progress` in `src-tauri/src/commands/models.rs`
- [X] T052 [US4] UI `src/features/models/ModelsScreen.tsx`: confirm dialog showing name, purpose, size and licence before any download; progress, pause, resume, cancel; storage list with delete
- [X] T053 [US4] `e2e/models.spec.ts`: pause, restart, resume to a verified file; refusal message shows needed and free space

---

## Phase 7: User Story 5 - Live memory-use indicator (P3)

**Goal**: a status-bar badge with a warning state.

**Independent Test**: badge updates every 2 s and turns to warning above 85% system memory use.

- [X] T054 [P] [US5] `core/tests/memory_level.rs`: `warning` at or above 85% system use, `normal` below
- [X] T055 [US5] `core/src/memory/sampler.rs`: app and system memory via `sysinfo`, level rule from T054
- [X] T056 [US5] Emit `memory:sample` every 2 s from `src-tauri/src/events.rs`
- [X] T057 [US5] UI `src/features/status/MemoryBadge.tsx` in `src/app/StatusBar.tsx`: value, warning style, hint text

---

## Phase 8: Polish and cross-cutting

- [X] T058 Run every scenario in `quickstart.md` on Windows and on macOS; record pass or fail per success criterion SC-001 to SC-010 in `docs/verification/001.md`
- [X] T059 Measure the budgets from plan.md (installed size under 25 MB, idle RAM under 150 MB, check under 10 s, idle CPU under 2%) on both platforms and record them in `docs/verification/001.md`
- [X] T060 Run `cargo deny check` and the npm licence check, then update `docs/dependencies.md`; any non-permissive licence needs a recorded justification (Principle VI)
- [X] T061 Update `docs/roadmap.md` to mark feature 001 done and note the final spec-definition values for feature 002

---

## Dependencies

- Phase 1 then Phase 2, then stories.
- US1 and US2 are independent of each other; both are P1.
- US3 needs US1 only for the re-run button (T044 calls T022).
- US4 and US5 are independent of the other stories.
- T024 (installer) needs T021.
- T008 should finish before T024 and T018 (they use its findings).

## Parallel examples

- Phase 2: T010, T011, T013, T014 together after T009.
- US1 tests T015, T016, T017 together.
- US2 tests T026 to T029 together.
- After Phase 2, US1, US2, US4 and US5 can proceed in parallel; US3 starts after T022.

## Implementation strategy

1. MVP: Phases 1, 2 and 3 (US1). Stop and check it on both platforms.
2. Add US2, then US3, US4 and US5 in that order, verifying each.
3. Finish with Phase 8. Feature 002 does not start until T058 passes on both platforms.

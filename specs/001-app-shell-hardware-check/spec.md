# Feature Specification: App Shell and Hardware Check

**Feature Branch**: `001-app-shell-hardware-check` (no git branch; repo is not under git)

**Created**: 2026-11-04

**Status**: Draft

**Input**: User description: "Feature 001, app shell and hardware check. A desktop app shell for Windows and macOS that checks the computer at install and first launch against one minimum spec and picks a mode (full local, hybrid, or blocked), shows the incompatibility message when below the minimum, provides settings (general, light and dark theme, hotkeys, analytics off by default), creates, saves and reopens projects in a portable format, manages on-demand model downloads, and shows a live memory-use indicator. No dubbing yet."

**Parent**: `docs/master-spec/spec.md` (Story 3, parts of Story 8, FR-001, FR-018 to FR-020, FR-026, FR-027, FR-037). Roadmap: `docs/roadmap.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Compatibility check at install and first launch (Priority: P1)

When the user installs or first opens the app, it checks processor, memory, free disk space and graphics or acceleration hardware, compares them with one defined minimum spec, and chooses a mode: full local, hybrid, or blocked. If the computer is below the minimum, the app stops and shows: "Your computer is not compatible with this app. You need [exact missing requirement] to run this app."

**Why this priority**: Explicit owner requirement and a non-negotiable constitution principle. Every later feature depends on the mode it picks.

**Independent Test**: Run the check on a machine above the minimum, a machine between minimum and full spec, and a machine below the minimum (real or simulated). Confirm the three outcomes and read the message on the blocked one.

**Acceptance Scenarios**:

1. **Given** a computer below the minimum on one or more items, **When** the installer or first launch runs the check, **Then** the app shows the incompatibility message naming every missing requirement with the needed value and the detected value, and does not let the user continue.
2. **Given** a computer above the minimum but below the full-local spec, **When** the check finishes, **Then** the app selects hybrid mode and tells the user in plain words which later features will need an optional cloud key.
3. **Given** a computer at or above the full-local spec, **When** the check finishes, **Then** the app selects full local mode and shows no cloud prompt.
4. **Given** the same app version on Windows and on macOS, **When** the check runs on each, **Then** both use the same minimum spec and show equivalent messages.
5. **Given** the user later upgrades the computer, **When** the user chooses "Re-run compatibility check" in settings, **Then** the mode is updated from the new hardware.
6. **Given** the check cannot read one hardware value, **When** it runs, **Then** it reports that value as unknown, treats it cautiously, and lets the user continue in hybrid mode with a warning rather than crashing.

---

### User Story 2 - Create, save and reopen projects (Priority: P1)

The user creates a new project, gives it a name, saves it, closes the app, and later reopens it with everything as it was. A project file made on Windows opens on macOS and the other way round.

**Why this priority**: Every later feature stores its work in a project. Cross-platform portability is part of the constitution.

**Independent Test**: Create a project, save, close, reopen. Copy the file to the other platform and open it there.

**Acceptance Scenarios**:

1. **Given** a new project, **When** the user saves it, **Then** a single project file is written to the chosen place and appears in a recent-projects list.
2. **Given** a saved project, **When** the app is reopened and the project chosen, **Then** its contents and settings are restored with no loss.
3. **Given** a project file made on another platform, **When** opened, **Then** it loads correctly, including names with non-Latin characters.
4. **Given** a project file made by a newer version of the app, **When** opened by an older version, **Then** the app says the file needs a newer version and does not damage it.
5. **Given** a damaged project file, **When** opened, **Then** the app reports what is wrong and offers the last automatic backup.
6. **Given** the app closes unexpectedly while a project is open, **When** the app restarts, **Then** it offers to recover the unsaved work.

---

### User Story 3 - Settings: general, theme, hotkeys, analytics (Priority: P2)

The user opens settings and changes general options, switches between light and dark theme, edits keyboard shortcuts, and chooses whether to share anonymous usage data. Analytics is off by default.

**Why this priority**: Expected in the reference tool and needed by later features, but the app is usable with defaults.

**Independent Test**: Change the theme and one hotkey, restart the app, and confirm both persist. Confirm analytics is off on a fresh install.

**Acceptance Scenarios**:

1. **Given** settings open, **When** the user switches theme, **Then** the whole app changes at once and the choice is remembered after restart.
2. **Given** the user assigns a hotkey already in use, **When** saving, **Then** the app shows the conflict and asks which to keep.
3. **Given** a fresh install, **When** the user opens settings, **Then** analytics is off, and turning it on shows what is collected.
4. **Given** the user chooses "Reset to defaults", **When** confirmed, **Then** settings return to defaults and projects are not touched.
5. **Given** the system theme is dark, **When** the user picks "Follow system", **Then** the app matches it.

---

### User Story 4 - On-demand model downloads (Priority: P2)

When a feature needs a model file, the app shows its size and what it is for before downloading. The user can start, pause, resume and cancel. The app checks free disk space first and keeps downloaded models for reuse.

**Why this priority**: Later features need models. A reliable downloader avoids a huge install and suits low-spec machines.

**Independent Test**: Start a download, pause, close the app, reopen, resume, and confirm the file completes and is verified. Try a download larger than free space and confirm it is refused with a clear message.

**Acceptance Scenarios**:

1. **Given** a feature needs a model, **When** the user is asked, **Then** the name, purpose, size and licence are shown before any download starts.
2. **Given** a download in progress, **When** the user pauses or the connection drops, **Then** progress is kept and the download resumes from where it stopped.
3. **Given** not enough free disk space, **When** the user tries to download, **Then** the app refuses and says how much space is needed and how much is free.
4. **Given** a finished download, **When** it is checked, **Then** a corrupted file is detected and re-downloaded instead of used.
5. **Given** downloaded models, **When** the user opens model storage in settings, **Then** each model's size is shown and it can be deleted to free space.
6. **Given** no internet, **When** a download is needed, **Then** the app says so and offers to retry later.

---

### User Story 5 - Live memory-use indicator (Priority: P3)

A small indicator in the window shows how much memory the app is using, so users on weak machines can see load.

**Why this priority**: Appears in the reference tool; helpful but not essential.

**Independent Test**: Open the app and watch the indicator change as work starts and stops.

**Acceptance Scenarios**:

1. **Given** the app is open, **When** memory use changes, **Then** the indicator updates within a few seconds.
2. **Given** memory use passes a warning level, **When** it happens, **Then** the indicator changes appearance and offers a hint on how to reduce load.

---

### Edge Cases

- Computer has plenty of memory but almost no free disk: blocked or warned, with the exact missing disk space named.
- Computer reports no graphics acceleration: allowed, mode chosen from the remaining values.
- Virtual machine or remote desktop reports odd hardware: the check uses what is reported and shows detected values so the user can see why.
- Installer run without admin rights: the check still runs, and any step that needs rights explains itself.
- Project folder is read-only or on a full disk: save fails with a clear message and no data loss.
- Two copies of the app open the same project: the second opens read-only and says why.
- Model download interrupted by sleep, shutdown or lost network: resumes cleanly.
- Very long or unusual project names and paths: handled on both platforms.
- Settings file missing or corrupted: app starts with defaults and says so.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST run on Windows and macOS with identical behaviour for every requirement in this feature.
- **FR-002**: The installer and first launch MUST check processor, memory, free disk space and graphics or acceleration hardware.
- **FR-003**: The minimum spec and the full-local spec MUST each be defined once, in one place, and used by the check, the message and the documentation.
- **FR-004**: The check MUST choose exactly one mode: full local, hybrid, or blocked.
- **FR-005**: If the computer is below the minimum, the app MUST stop and show "Your computer is not compatible with this app. You need [exact missing requirement] to run this app." listing every missing item with required and detected values.
- **FR-006**: In hybrid mode the app MUST tell the user which later features need a cloud key, in plain language.
- **FR-007**: Users MUST be able to re-run the check from settings.
- **FR-008**: If a hardware value cannot be read, the app MUST show it as unknown, treat it cautiously, and not crash.
- **FR-009**: Users MUST be able to create, name, save, reopen and delete projects, with a recent-projects list.
- **FR-010**: A project MUST be stored as a single portable file that opens on every supported platform without data loss, including non-Latin names.
- **FR-011**: The project file MUST carry a format version, and the app MUST refuse to damage a file from a newer version.
- **FR-012**: The app MUST keep automatic backups and offer recovery after a crash or a damaged file.
- **FR-013**: The app MUST provide settings for general options, light and dark theme (with follow-system), hotkeys with conflict detection, and analytics.
- **FR-014**: Analytics MUST be off by default and, when enabled, MUST show what is collected.
- **FR-015**: Settings MUST persist across restarts and be resettable without touching projects.
- **FR-016**: The app MUST download models only on request, showing name, purpose, size and licence first.
- **FR-017**: Downloads MUST support pause, resume, cancel, resume after interruption, free-space check, and integrity verification.
- **FR-018**: Users MUST be able to see and delete downloaded models in settings.
- **FR-019**: The app MUST show a live memory-use indicator with a warning state.
- **FR-020**: All text shown to users MUST be separable from the interface so it can be translated later.

### Key Entities

- **Machine Capability Report**: Detected processor, memory, free disk, acceleration, each with required and detected value, plus the chosen mode and any missing requirements.
- **Spec Definition**: The single minimum spec and full-local spec used everywhere.
- **Project**: A saved file with name, format version, creation and save times, and a place for later features to store their data.
- **Settings**: General options, theme, hotkeys, analytics choice, model storage location.
- **Model Record**: Name, purpose, size, licence, download state, location, integrity result.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a computer below the minimum, the incompatibility message appears in 100% of installs and launches, names every missing requirement, and the app never crashes.
- **SC-002**: The compatibility check finishes in under 10 seconds on a minimum-spec machine.
- **SC-003**: The same machine gets the same mode on Windows and macOS builds in 100% of test cases.
- **SC-004**: A project saved on one platform opens with no data loss on the other in 100% of test cases, including non-Latin names.
- **SC-005**: After a forced app quit, at most the last 60 seconds of unsaved work is lost.
- **SC-006**: An interrupted model download resumes without restarting from zero in at least 95% of tests.
- **SC-007**: A corrupted download is detected and never used in 100% of tests.
- **SC-008**: At least 90% of first-time users reach a saved, reopened project in under 3 minutes without help.
- **SC-009**: The app itself, before any model is downloaded, stays small enough to install on a minimum-spec machine; the plan sets the exact limit.
- **SC-010**: Theme, hotkey and analytics choices survive a restart in 100% of tests.

## Assumptions

- The minimum spec and full-local spec numbers are not set here. They need benchmarking of the real dubbing steps, so the plan for feature 002 confirms them; this feature ships a first definition with values marked provisional.
- A single hardware-compatibility alert wording is used, with the missing requirement filled in.
- No account or sign-in exists.
- Updates of the app itself are out of scope for this feature.
- The user language of the interface is English for now; text is kept separable for later translation.
- Linux, web and mobile come in feature 010, but this feature's project format and spec definition must not block them.
- Cloud keys are not handled here; feature 003 adds them. This feature only tells the user when hybrid mode will need one.

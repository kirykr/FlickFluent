# FlickFluent Roadmap

The full requirements live in `docs/master-spec/spec.md` (umbrella, read-only reference). Each feature below gets its own speckit pass, one at a time: specify, plan, tasks, implement. Do not start the next feature until the previous one is implemented and verified on both platforms.

Story and FR numbers refer to the master spec.

| # | Feature | Master spec coverage | Depends on |
|---|---------|----------------------|------------|
| 001 (built, macOS only verified) | App shell and hardware check | Story 3, parts of Story 8 (theme, hotkeys, settings), FR-001, FR-018 to FR-020, FR-026, FR-027, FR-037 | none |
| 002 | Core dub pipeline (local) | Stories 1 and 2, FR-002 to FR-008, FR-012, FR-016 (basic), FR-024, FR-025 | 001 |
| 003 | Hybrid cloud engines and keys | Rest of Story 8, FR-021 to FR-023, FR-031 | 002 |
| 004 | Transcript editor and timeline | Story 5, FR-011, FR-013, FR-014, FR-029, FR-030 | 002 |
| 005 | Voice cloning and speaker profiles | Story 4, FR-009, FR-010 | 002, 003 |
| 006 | Video tools and export options | Story 6, FR-015, FR-033 | 002 |
| 007 | Batch and multi-project | Story 7, FR-017, FR-032 | 004 |
| 008 | Lip-sync | Story 9, FR-034 | 002, 003 |
| 009 | Real-time dubbing | Story 10, FR-035 | 002, 003 |
| 010 | Linux, web and mobile | Story 11, FR-036 | all above |

Constitution (`.specify/memory/constitution.md`) applies to every feature. Every plan needs a Constitution Check.

## Owner decisions

- 2026-11-04: keep the 8 GB RAM minimum (4 cores, 10 GB free disk) in `config/spec-definition.json`. Still provisional: re-check against real benchmarks in feature 002, and lower it only if light steps are proven to fit on less.

## Feature 001 hand-off (2026-11-04)

Built and tested on macOS; see `docs/verification/001.md`. Before starting 002:

- Final spec values are still provisional in `config/spec-definition.json`: minimum 4 cores, 8 GB RAM, 10 GB free disk, Windows 10 or macOS 12; full local 8 cores, 16 GB RAM, 40 GB free disk and Apple Silicon or a discrete GPU. Feature 002's benchmarks replace them.
- Windows is entirely unverified, including the installer hook.
- Idle RAM is about 150 MB against a 150 MB budget.
- The model catalogue holds only a download self-test file. Feature 002 adds real models.

## Feature 002 hand-off (2026-11-05)

Details and numbers: `docs/verification/002-benchmarks.md`.

- **Spec values (`config/spec-definition.json`):** unchanged and still provisional, `"basis": "simulated"`. Minimum 4 cores, 8 GB RAM, 10 GB free disk, Windows 10 or macOS 12; full local 8 cores, 16 GB RAM, 40 GB free disk and Apple Silicon or a discrete GPU.
- **What was measured:** the 2-minute French clip, on one Mac (Apple M5, 10 cores, 16 GB), once as itself (full-local models) and once held to the minimum spec (minimum models, 3 worker threads, memory watchdog). Heaviest stage: 4066 MB full-local, 1642 MB minimum. The 8 GB minimum stays, because no real low-memory computer and no Windows computer was tried.
- **What is simulated:** the minimum spec (a Mac held to its numbers, not a real 8 GB computer).
- **What remains open:** all Windows runs (the `.ps1` limiter was never executed); a real 8 GB computer; a full-length film; the slow, non-linear Whisper large-v3 time; the eSpeak source offer in About (T085); NLLB inference with the real model; the macOS LGPL FFmpeg build; Windows on ARM has no local engine.
- **Owner decisions this feature:** eSpeak NG stays in the separate-process pack with the matching source published (2026-11-05); NLLB-200 is an opt-in non-commercial model.

## Feature 004 hand-off (2026-11-07)

**Built (macOS):** the transcript editor and timeline on top of a finished dub. A table of the lines (virtualised, 6,000 rows) with inline edits, insert, delete, merge, split, duplicate, search and replace, a bulk text dialog, undo and redo kept in the core, and a project file that always equals what is on screen. A line is "changed" when a hash of what its clip was made from differs (text, voice, speaker, gender, engine, speed, volume, pitch); making speech again sends only those lines to the voice engine (local or the 003 cloud engines under the shared amount guard), swaps each clip in when complete, and rebuilds the mix for the touched sections only, spliced in by sample number. Smart Fit and End Sync fit speech to its window with ffmpeg only; speed, volume and pitch are a post chain on the raw clip. SRT and WebVTT import and export with a preview and three choices, and three caption templates with Khmer font fallbacks. A section the editor changed is kept by a resume, and a fresh dub asks before it replaces edited lines.

**Verified:** 399 Rust tests, all Vitest and Playwright cases on macOS, against the fake sidecar and fake cloud server (`docs/verification/004.md`).

**Open:**
- Windows has not run anything of 004. Do not close the feature before that (T084).
- Real models (T081): the time for one edited line (SC-001), Smart Fit on a real run (SC-002), a listen at the splice join and at the pitch limits, Khmer in the preview. `core/tests/editor_real.rs` is ready.
- Usability check (T083) and idle RAM (T076) not done.
- The CI npm licence check already fails on six packages from before 004 (Geist font OFL-1.1, `dompurify`, `json-schema`, `khroma`, `lightningcss`, `robust-predicates`); decide the allow-list.
- Each edit saves the whole project file (20 to 29 ms at 6,000 lines, fine) but also rotates the backups, so the copy from before an editing session is lost after three edits; consider rotating only on the first save of a session.

## Feature 003 hand-off (2026-11-06)

**Built (macOS):** optional cloud engines for translation and speech with the user's own keys. Services: Google Gemini API and Azure AI services, both described in `config/cloud-services.json`. Keys live only in the OS secure storage (`keyring-core`); `cloud-keys.json` and `cloud-consent.json` hold no secret. Cloud is off by default; each service and step needs a notice that names the amount (agreed again only above 5 times the agreed amount). Several keys per service rotate on a limit or refusal, wait up to 10 minutes, then pause the run so it can resume. The language pair check offers a cloud engine or a "set up a key" hint, and cloud-only languages appear in the list once a key is verified.

**Verified:** 290 Rust tests, 80 Vitest, 57 Playwright on macOS, against a local fake server (`docs/verification/003.md`). The no-leak test searches every file and message for the key.

**Open:**
- Windows has not run anything of 003 (OS store test, quickstart, no-leak search). Do not close the feature before that.
- No real service was called: run `core/tests/cloud_real.rs` with your keys (T071), then the Gemini listening test (T072). Model ids, voice genders and language lists are marked unverified in the data file.
- The estimate (10 characters per second) was checked against two synthetic French fixtures only (T066); the run now also stops before any send past 5 times the agreed amount. Idle RAM was measured once and needs a like-for-like comparison (T068).
- If no Gemini Khmer voice is acceptable, Khmer cloud speech stays unsolved and the local-model decision comes back to the owner.

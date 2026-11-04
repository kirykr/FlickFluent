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

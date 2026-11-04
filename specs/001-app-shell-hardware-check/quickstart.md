# Quickstart: validating feature 001

Prerequisites: Rust stable, Node 20+, Tauri 2 prerequisites for the OS. Run on both Windows and macOS.

## Setup
```bash
npm install
cargo build --manifest-path core/Cargo.toml
```

## Test-first order (Principle VIII)
Write and see these fail before implementing:
1. Core contract tests: `cargo test --manifest-path core/Cargo.toml` covering
   - hardware evaluation: fake machines for blocked, hybrid, full local, unknown value (see `data-model.md` rules)
   - blocked message text matches `contracts/ipc-commands.md` exactly
   - `.flick` round trip, newer-version refusal, damaged file with backup, Unicode name (see `contracts/project-file-format.md`)
   - downloader: resume after interruption, hash mismatch, no-space refusal
2. UI tests: `npm test`.
3. End to end: `npm run e2e` against the built app.

## Manual scenarios (map to spec)
| Spec item | Steps | Expected |
|-----------|-------|----------|
| US1 blocked | `flickfluent --check-hardware --fake-machine tests/fixtures/low.json` | Exit code 10 and the exact incompatibility message naming each missing item |
| US1 hybrid / full | same with `mid.json`, `high.json` | Exit code 0, mode `hybrid` / `fullLocal` |
| US1 real machine | run app first time | check result in under 10 s |
| US2 | create, save, close, reopen; copy file to the other OS and open | contents identical, Unicode name intact |
| US2 crash | kill the app after an edit, relaunch | recovery offered, loss under 60 s |
| US3 | switch theme, change a hotkey, restart | both persist; analytics off on fresh install |
| US4 | start a download, pause, quit, relaunch, resume; try one larger than free space | completes with verified hash; refusal shows needed and free space |
| US5 | watch the status bar while a download runs | updates every 2 s; warning state above 85% |

## Pass criteria
All success criteria SC-001 to SC-010 in `spec.md` hold on both platforms. Record results per platform in the PR or a note.

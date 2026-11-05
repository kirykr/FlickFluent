# Implementation Plan: Hybrid Cloud Engines and API Keys

**Branch**: `003-hybrid-cloud-keys` | **Date**: 2026-11-05 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-hybrid-cloud-keys/spec.md`

## Summary

Add optional cloud engines for the two heavy steps, translation and speech, on top of feature 002's pipeline. The user adds their own key for a service (one key serves every step the service offers), verifies it, and may then switch a step from local to that service. Keys live only in the operating system's secure storage. Before the first cloud step, and again if a run is much larger than the one agreed to, a notice says what is sent and how much; nothing is sent before consent and nothing is sent by default. All cloud traffic runs in the Rust core, never in the Python sidecar, through one adapter per service. A rotation layer moves to the next key on a rate limit or failure, waits up to 10 minutes when every key is limited, and keeps finished work so a run resumes where it stopped. Two services ship first, the Google Gemini API (one key for translation and speech, with Khmer speech documented) and Azure AI services (wide language and voice lists). Services and their languages are data, so adding one changes no screen.

## Technical Context

**Language/Version**: Rust 1.80+ (all cloud logic), TypeScript 5.x with React (settings, notice, labels). Python sidecar unchanged.

**Primary Dependencies**: Existing: Tauri 2, tokio, reqwest, serde, sha2, uuid, time. New: `keyring-core` 1.0.1, `apple-native-keyring-store` 1.0.2 (macOS), `windows-native-keyring-store` 1.1.0 (Windows), `zeroize` (clear secrets from memory; MIT or Apache-2.0, to be confirmed with `cargo deny`). All `MIT OR Apache-2.0`. See research R1.

**Storage**: Secrets: OS secure storage only. Non-secret files in the app data folder: `cloud-keys.json` (key list), `cloud-consent.json`. `settings.json` gains `engines`. A project's dub data gains an optional `cloudUsage`. Cloud voice lists cached in the app data folder. No project format bump (R9).

**Testing**: `cargo test` first (adapter contracts, keys, no-leak, off-by-default, consent, rotation with a fake clock, pipeline) against a local fake cloud server; Vitest and Playwright for the screens with a fake core; opt-in real-service test; ignored OS-store test run by hand on each OS.

**Target Platform**: Windows 10 1809+ and macOS 12+ (x64 and arm64), same behaviour on both (Constitution I).

**Project Type**: desktop-app (Rust core + Tauri shell + React UI), unchanged.

**Performance Goals**: "Verify connection" answers in under 30 s on a normal connection (SC-001); a cloud step adds no more than one request in flight per key; the UI stays responsive while the run waits on a limit (progress events keep coming, at least every 5 s as in 002).

**Constraints**: no secret in any file, log, error text, project or export (SC-002); zero requests when engines are local (SC-003); installer still under 25 MB (new crates are small); no change to the sidecar protocol; the 10-minute wait budget and the 5× consent rule are fixed by the spec.

**Scale/Scope**: up to about 6,000 lines per film, so up to about 120 translation requests and 6,000 speech requests per cloud run; 2 services at first, up to 5 keys each; 1 new settings card, 1 consent dialog, small additions to the run progress, result details and language pair check.

## Constitution Check

*GATE: must pass before Phase 0. Re-checked after Phase 1 design.*

| Principle | Status | How this plan complies |
|-----------|--------|------------------------|
| I. Windows and macOS first | PASS, with a standing gap | One interface (`KeyStore`) with a Windows and a macOS store behind it; all other code is shared. CI covers both for the fake-server tests. The OS stores themselves are exercised by an ignored test run on each OS by hand. Windows has still not been run on any machine (carried from 002). |
| II. Low-spec | PASS | This feature exists to serve it: hybrid machines get the heavy steps from a service. Light steps stay local. Sections and resume are reused, so memory stays bounded. |
| III. Install-time check and alert | PASS | Unchanged. The hardware mode feeds the guidance in the pair check (Story 5). |
| IV. Any language from a dropdown | PASS | Cloud languages come from data (`config/cloud-services.json`, refreshed by a live call). A language only a service offers appears with no screen change (SC-008). Unsupported pairs still say so before the run and offer fallbacks. |
| V. Reuse open source | PASS | OS stores through `keyring-core` and the two store crates, HTTP through `reqwest`, existing ffmpeg for fitting clips, shadcn parts for screens. Only adapters, rotation and consent are written. Recorded in `docs/dependencies.md`. |
| VI. Licence discipline | PASS | New crates are `MIT OR Apache-2.0`. Vendor terms are linked in the notice. No non-commercial component is added. |
| VII. Voice cloning consent | N/A | Fixed vendor voices only; no cloning (feature 005). |
| VIII. Test-first | PASS | Adapter contracts and the fake server are written before the adapters; rotation, consent and no-leak tests before their code (see quickstart and `contracts/cloud-adapter.md`). |
| IX. Privacy and local-first | PASS | Cloud off by default; per-service, per-step consent with a notice that says what is and is not sent; only step text leaves, never video or audio; keys only in the OS store; no silent local-to-cloud switch; secrets wrapped, scrubbed and searched for in tests. |

No violations. Complexity Tracking is empty.

**Re-check after Phase 1**: unchanged. Design adds nothing that breaks a gate; the one open risk is the amount estimate (R6), which is recorded and has a verify step.

## Project Structure

### Documentation (this feature)

```text
specs/003-hybrid-cloud-keys/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ipc-commands.md
│   ├── cloud-adapter.md
│   └── cloud-services.schema.json
├── checklists/
│   └── requirements.md
└── tasks.md              # created by /speckit-tasks
```

### Source Code (repository root)

```text
config/
└── cloud-services.json              # NEW: services, steps, endpoints, languages (data)

core/
├── src/
│   ├── cloud/                       # NEW
│   │   ├── mod.rs                   # public surface
│   │   ├── secret.rs                # Secret wrapper, scrub
│   │   ├── keystore.rs              # KeyStore trait, OS impl, in-memory impl, session-only
│   │   ├── keys.rs                  # cloud-keys.json, masking, fingerprint, ordering
│   │   ├── consent.rs               # cloud-consent.json, 5x rule, notice version
│   │   ├── services.rs              # load and validate cloud-services.json
│   │   ├── adapter.rs               # trait, CloudFailure
│   │   ├── adapters/{gemini.rs,azure.rs}
│   │   ├── rotation.rs              # key pool, waiting budget, Clock and Sleeper
│   │   ├── engine.rs                # translate in batches, speak per line, usage
│   │   └── estimate.rs              # chars from duration
│   ├── pipeline/run.rs              # EDIT: choose local or cloud per step
│   ├── pipeline/plan.rs             # EDIT: engine choice enters the plan and input hashes
│   ├── pipeline/preflight.rs        # EDIT: cloud block in Preflight
│   ├── pipeline/errors.rs           # EDIT: new codes
│   ├── languages/                   # EDIT: cloud fallback and languages
│   └── settings/                    # EDIT: engines
└── tests/
    ├── support/fake_cloud.rs        # NEW: local fake services
    ├── cloud_adapters.rs  cloud_keys.rs  cloud_no_leak.rs  cloud_off_by_default.rs
    ├── cloud_consent.rs   cloud_rotation.rs  cloud_pipeline.rs
    ├── keystore_os.rs (ignored)  cloud_real.rs (ignored)

src-tauri/src/commands/cloud.rs      # NEW: commands in contracts/ipc-commands.md
src/features/cloud/                  # NEW: CloudCard, KeyDialog, ConsentDialog, EngineSelect, useCloud
src/features/dub/                    # EDIT: cloud labels, consent step, usage in Result
src/features/settings/               # EDIT: mount CloudCard
e2e/cloud.spec.ts                    # NEW (fake core)
docs/dependencies.md  docs/verification/003.md
```

**Structure Decision**: all new logic in one `core/src/cloud/` module with the vendor-specific code isolated in `adapters/`, so the pipeline depends on a trait and not on a vendor. UI in a new `src/features/cloud/` folder, mounted from Settings and the run flow. No new crate or process.

## Complexity Tracking

No violations to justify.

# Quickstart: validating feature 004

Runs against the fake sidecar and fake cloud server unless noted, so no models or accounts are needed for steps 1 to 8.

## Prerequisites

- Features 002 and 003 working (`cargo test --workspace` green).
- Rust, Node, and for step 9 the Python runtime pack and models as for 002.

## Steps

1. **Contracts first.** `cargo test -p flickfluent-core --test subtitles`: the fixtures in `contracts/subtitle-formats.md` parse and write as stated; round trip within 1 ms; 6,000 cues in under a second.
2. **Edit rules.** `cargo test -p flickfluent-core --test editor_ops`: every op in `data-model.md`, its inverse, undo and redo, rejected edits change nothing, disk equals memory after each op.
2b. **Runtime, guard and runs.** `cargo test -p flickfluent-core --test editor_runtime --test cloud_guard --test editor_run_interaction`: the editor picks the same voices and engine as a run; the amount guard is shared and an editing session obeys the 5× rule and adds to the usage record; a resume keeps edited sections, a fresh start asks first (`DUB_EDITS_WOULD_BE_LOST`).
3. **Changed marks.** `cargo test -p flickfluent-core --test editor_changed`: edit and edit back is not changed; speaker, gender, voice and engine changes mark lines; old project with empty hash marks nothing.
4. **Search and replace.** `cargo test -p flickfluent-core --test editor_search`: options, a chosen subset, one undo step, 6,000 lines under 100 ms.
5. **Fitting.** `cargo test -p flickfluent-core --test editor_fit`: Smart Fit to the limits and the reasons when it cannot; End Sync never overlaps the previous line and never moves `startMs`; no engine call.
6. **Re-synthesis.** `cargo test -p flickfluent-core --test editor_resynth`: only changed lines reach the fake sidecar (and the fake cloud, with the 003 guard counting only their characters); unchanged clip files keep their bytes; a stop keeps swapped clips; an edit during a run keeps the line changed; the splice leaves the other sections byte-equal.
7. **UI.** `npx vitest run src/features/editor` and `npx playwright test e2e/editor.spec.ts` (fake core): edit a cell, undo, search and replace, merge, split, bulk dialog, Smart Fit summary, stop, subtitle import preview and the three choices, template preview, memory indicator, and a 6,000 line table that scrolls.
8. **Both platforms.** CI runs steps 1 to 7 on Windows and macOS. Windows is carried as for 002 and 003 until a Windows machine exists.
9. **Real models (by hand).** With `FLICK_TEST_MODELS` and `FLICK_TEST_PYTHON` as in `pipeline_real.rs`: `cargo test -p flickfluent-core --test editor_real -- --ignored --nocapture` edits one line of the 2-minute clip, re-synthesizes it, prints the time (SC-001: under 15 s on the full-local machine) listens by ear for the join at the splice, runs Smart Fit on the flagged lines of the real run for SC-002, and looks at Khmer text in the caption preview. Record the time in `docs/verification/004.md`.

## Expected outcomes

- Steps 1 to 8 pass with no network access.
- Step 9 prints the one-line re-synthesis time and the number of clip files that changed (exactly one).

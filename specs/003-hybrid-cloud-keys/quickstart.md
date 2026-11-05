# Quickstart: validating feature 003

Runs against the fake cloud server unless noted, so no real account is needed for steps 1 to 8. Step 9 uses real keys and is opt-in.

## Prerequisites

- Feature 002 working (`cargo test --workspace` green).
- Rust, Node and the Python runtime pack as for 002.

## Steps

1. **Contracts first.** `cargo test -p flickfluent-core --test cloud_adapters` runs each adapter against `core/tests/support/fake_cloud.rs`: right headers, right batch sizes, one result per input, each failure kind mapped (`contracts/cloud-adapter.md`).
2. **Secrets.** `cargo test -p flickfluent-core --test cloud_keys` uses the in-memory store: add, mask, duplicate refused, remove, session-only, store-unavailable refuses to save. `cargo test -p flickfluent-core --test cloud_no_leak` runs a whole add-verify-run-fail-remove story with a marker key and then searches every file under the temp folders and the captured log for the marker (SC-002).
3. **Default is local.** `cargo test -p flickfluent-core --test cloud_off_by_default`: a full dub on the fake sidecar with default settings makes zero requests to the fake server (SC-003).
4. **Consent.** `cargo test -p flickfluent-core --test cloud_consent`: no run before consent; amount shown; ask again only above 5× the stored amount; withdrawal; notice version bump.
5. **Rotation and waiting.** `cargo test -p flickfluent-core --test cloud_rotation` with a fake clock: the first key limited mid-run, the second takes over; no line missing or doubled; keys rejected or out of credit are set aside and named; all limited gives a wait of the stated time and a pause after 10 minutes with the run resumable; a set-aside key is tried again next run.
6. **Pipeline.** `cargo test -p flickfluent-core --test cloud_pipeline`: translation and speech through the fake server inside a complete run: engine id in the input hash, partial batches kept, usage record written, clips identical in shape to local ones.
7. **UI.** `npx vitest run src/features/cloud` and `npx playwright test e2e/cloud.spec.ts` (fake core): add and verify a key, masked afterwards, consent dialog with the amount, the cloud label in the stage list, the fallback in the language pair check.
8. **Both platforms.** CI runs steps 1 to 7 on Windows and macOS. The Windows and macOS secure-storage tests (`core/tests/keystore_os.rs`, ignored by default, run with `--ignored` on a developer machine) store, read, and delete one real entry.
9. **Real services (optional, by hand).** With `FLICK_TEST_GEMINI_KEY`, or `FLICK_TEST_AZURE_KEY` and `FLICK_TEST_AZURE_REGION`: `cargo test -p flickfluent-core --test cloud_real -- --ignored`. Then, in the app, dub `fixtures/clip-20s-fr.mp4` into English and into Khmer with the engines switched to cloud. **Listening test (owner):** play the Khmer result for every Gemini voice in the curated list (`config/cloud-services.json`), mark which voices are acceptable and fix the female/male assignment. Compare the Gemini French-to-English translation with the local M2M100 result line by line. Record the result in `docs/verification/003.md`.

## Expected outcomes

- Steps 1 to 8 pass with no network access.
- Step 9 prints the languages each live service reports; compare with `config/cloud-services.json` and update its `checkedOn` dates.
- `grep -r <key text>` over the app data folder, a saved project and its work folder finds nothing.

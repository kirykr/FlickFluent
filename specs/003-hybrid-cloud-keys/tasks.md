---

description: "Task list for feature 003: Hybrid Cloud Engines and API Keys"
---

# Tasks: Hybrid Cloud Engines and API Keys

**Input**: Design documents from `specs/003-hybrid-cloud-keys/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Constitution Principle VIII requires test-first: write each test, see it fail, then implement. Cloud traffic is tested against a local fake server; no real account is needed except the opt-in task T071.

**Organization**: Grouped by user story. Each story can be built and verified alone after Phase 2.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: US1 to US5, matching `spec.md`

## Path Conventions (from plan.md)

`core/` Rust library, `src-tauri/` Tauri shell, `src/` React UI, `e2e/` Playwright, `config/` shared config, `docs/` project docs. New core code lives in `core/src/cloud/`; new UI in `src/features/cloud/`.

## Decisions applied

- One key per service, usable for every step that service offers (Q1). Order, consent and engine choice stay per step.
- Rate-limit wait: up to 10 minutes in total per request, then the run pauses and can be resumed (Q2). 30 s when the service states no time.
- The amount of text is shown before every cloud run; ask again only above 5 times the amount last agreed to (Q3).
- First services: Google Gemini API and Azure AI services. Azure Khmer voices are not the Khmer answer (owner, 2026-11-05).
- All cloud calls run in the Rust core, never in the sidecar (research R3).

---

## Phase 1: Setup

**Purpose**: dependencies, shared data file, test server.

- [x] T001 Add `keyring-core = "1.0.1"` and `zeroize` to `core/Cargo.toml`, `apple-native-keyring-store = "1.0.2"` as a macOS-only dependency and `windows-native-keyring-store = "1.1.0"` as a Windows-only dependency; run `cargo deny check licenses` and add anything it flags to `deny.toml` with a one-line reason
- [x] T002 [P] Create `config/cloud-services.json` with two entries, `google-gemini` (steps `translate` and `speak`, `needsRegion: false`, `models` for the Gemini text and speech models, a curated `voices` list of the 30 Gemini voices each with a `gender` of `female` or `male`, `languages` with `checkedOn`) and `azure-ai` (steps `translate` and `speak`, `needsRegion: true`, `voices` absent, `endpoints.voices` set); `endpoints` hold HTTPS base URLs; `dataNotice` keys `cloud.notice.translate` and `cloud.notice.speak`; `limits` `{ batchLines: 50, batchChars: 4000 }`. Mark the voice genders and model ids as unverified in a `_comment` field until T072
- [x] T003 [P] Write test `core/tests/cloud_services.rs`: `config/cloud-services.json` validates against `specs/003-hybrid-cloud-keys/contracts/cloud-services.schema.json`; every `dataNotice` key exists in `src/i18n/en.json`; ids are unique; a service with `voices` lists at least one voice of each gender
- [x] T004 [P] Add `core/tests/support/fake_cloud.rs`: a local HTTP server imitating the Gemini translate, Gemini speak, Azure Translator, Azure Speech, Azure voices and key-check endpoints, with behaviours `ok`, `rate_limit(n, retry_after)`, `reject`, `no_credit`, `slow(ms)`, `drop`, `junk`, `wrong_count`, a request counter, and a log of headers received (per `contracts/cloud-adapter.md`)
- [x] T005 [P] Add the new crates and the two services (with their terms links and the date checked) to `docs/dependencies.md`; add the `FLICK_TEST_GEMINI_KEY`, `FLICK_TEST_AZURE_KEY` and `FLICK_TEST_AZURE_REGION` variables to the header of `core/tests/cloud_real.rs` when T071 creates it; create `docs/verification/003.md` now with one heading per task that fills it in (T026, T066, T068, T069, T070, T072)

---

## Phase 2: Foundational (blocks all stories)

**Purpose**: the secret type, the key store trait, the adapter trait, service data, error codes. No story works without these.

- [x] T006 [P] Write test `core/tests/cloud_secret.rs`: `Secret` has no `Debug`/`Display` output of its value (formatting shows `[secret]`), cannot be serialised, and `scrub(text, &[secrets])` replaces every occurrence of a secret, of its URL-encoded form and of its base64 form with `[key]`
- [x] T007 Implement `core/src/cloud/secret.rs`: `Secret` (zeroed on drop with `zeroize`, `expose()` only for building a request header) and `scrub`; create `core/src/cloud/mod.rs` and add `pub mod cloud;` to `core/src/lib.rs`
- [x] T008 [P] Write test `core/tests/cloud_keystore.rs` against the in-memory store: set, get, delete; deleting a missing entry is `Ok`; an entry that was never set reads as missing; a store that always fails returns `KEY_STORE_UNAVAILABLE`
- [x] T009 Implement `core/src/cloud/keystore.rs`: `KeyStore` trait (`set`, `get`, `delete` by key id), `OsKeyStore` using `keyring-core` with service name `app.flickfluent.desktop` and account `cloud-key:<keyId>` (set the default store once through `set_default_store`; macOS uses the Apple store, Windows the Windows store), `MemoryStore` for tests and for session-only keys
- [x] T010 [P] Implement `core/src/cloud/services.rs`: load `config/cloud-services.json` with `include_str!`, validate on load, expose `Service` per `data-model.md` (id, name, steps, needsRegion, termsUrl, dataNotice, endpoints, languages, models, voices, limits) and a way to replace endpoints for tests
- [x] T011 [P] Implement `core/src/cloud/adapter.rs`: the `CloudAdapter` trait (`check`, `translate`, `voices`, `speak`) and `CloudFailure { kind, retry_after, message }` with kinds `RateLimited`, `Rejected`, `NoCredit`, `Unavailable`, `Offline`, `Unexpected` per `contracts/cloud-adapter.md`
- [x] T012 [P] Add the error codes `CLOUD_CONSENT_REQUIRED`, `CLOUD_NO_KEY`, `CLOUD_KEYS_EXHAUSTED`, `CLOUD_AUTH_FAILED`, `CLOUD_OFFLINE`, `CLOUD_BAD_REPLY`, `KEY_INVALID`, `KEY_DUPLICATE`, `KEY_STORE_UNAVAILABLE` to `core/src/error.rs`, list each with a message key and a next-step key in `core/src/pipeline/errors.rs`, and add the matching `dub.error.*`/`dub.next.*` texts and the `cloud.*` texts used so far to `src/i18n/en.json` (the 002 test `core/tests/errors.rs` then covers them)
- [x] T013 [P] Implement `core/src/cloud/clock.rs`: `Clock` (now) and `Sleeper` (sleep) traits with a real implementation on tokio and a fake one that advances time instantly and records requested sleeps, for rotation tests
- [x] T014 Add an app-data path helper used by the cloud code in `core/src/cloud/mod.rs` (`cloud_keys_path`, `cloud_consent_path`, `voices_cache_dir` under a given app data folder) and read/write helpers using `core/src/fsutil.rs` atomic writes

- [x] T065 Write `core/tests/cloud_no_leak.rs` (SC-002) now and keep it failing until the story tasks are done (Constitution VIII): add a key whose text is a unique marker, then verify, run a cloud dub through the fake server, force a rate limit and a rejection, switch keys, export, remove the key; afterwards search every file under the temp app data, project, work and export folders, the captured log output and every error message for the marker, its URL-encoded form and its base64 form; the marker must be found nowhere. It must pass before Phase 8 closes, and T070 repeats the same search by hand on Windows

**Checkpoint**: secret handling, store trait, adapter trait, service data and error codes exist and are tested; nothing talks to a network yet. The no-leak test (T065) exists and fails.

---

## Phase 3: User Story 1 - Add a key and check that it works (P1) 🎯 MVP

**Goal**: add a key, verify it with an exact failure reason, see it masked, remove it. Keys only in the OS store.

**Independent Test**: add a key, "Verify connection" succeeds; restart the app and the key shows masked; remove it and it is gone from the store.

### Tests first

- [x] T015 [P] [US1] Write test `core/tests/cloud_keys.rs`: add stores the secret in the store and only `last4`, `fingerprint`, label, service, region, order and state in `cloud-keys.json`; the secret text is not in that file; the same key twice gives `KEY_DUPLICATE` (compared by `fingerprint`); a duplicate label within a service is refused; text is trimmed of spaces and line breaks, must not be empty and must be at most 512 characters (else `KEY_INVALID`); a label must not be empty or longer than 40 characters; a service with `needsRegion` refuses a key without a region; remove deletes the store entry first and then the row; reorder changes `order` for keys of one service only; a store that fails gives `KEY_STORE_UNAVAILABLE` and saves nothing, while `session_only` keeps the key in memory and writes no row after exit
- [x] T016 [P] [US1] Write test `core/tests/cloud_adapters.rs` (the `check` part): with the fake server, `check` returns the language lists on success and, per failure behaviour, `Rejected` (401/403), `NoCredit` (402 or quota text), `Offline` (connection refused), `Unavailable` (500), `Unexpected` (junk), for both the Gemini and the Azure adapter; the key is only ever in a request header (never in the URL, checked from the server's log); the failure message does not contain the key; a reply slower than the 25 s request timeout is `Unavailable` (use the `slow(ms)` behaviour with a shortened timeout); an `Offline`, `Unavailable` or `Unexpected` result never changes the `state` or `verifiedAt` of a saved key (spec Story 1 scenario 4: the key is kept), only `Rejected` and `NoCredit` do
- [x] T017 [P] [US1] Write Vitest `src/features/cloud/CloudCard.test.tsx`: lists keys with label, service and masked value only; "Add key" asks for service, label, key and (for Azure) region; "Verify connection" shows success naming the service, or the plain reason for each of `rejected`, `noCredit`, `offline`, `unreachable`, `unexpected`; "Remove" asks first; a key store error offers "use for this session only"

### Implementation

- [x] T018 [US1] Implement `core/src/cloud/keys.rs`: `KeyInfo` per `data-model.md` (id, serviceId, label, region, last4, fingerprint as SHA-256 of the secret, order, state `untested|verified|rejected`, verifiedAt, storage `os|session`), the add/list/verify-state/remove/reorder operations over a `KeyStore` and `cloud-keys.json`, with the validation rules quoted in T015
- [x] T019 [P] [US1] Implement `check` for Gemini in `core/src/cloud/adapters/gemini.rs` (a small request that proves the key, plus the language list; key in the `x-goog-api-key` header; 25 s request timeout, so verify answers well inside SC-001's 30 s; failure mapping per `contracts/cloud-adapter.md`)
- [x] T020 [P] [US1] Implement `check` for Azure in `core/src/cloud/adapters/azure.rs` (key in `Ocp-Apim-Subscription-Key` and the region in `Ocp-Apim-Subscription-Region`; languages and voices lists; same 25 s request timeout and failure mapping)
- [x] T021 [US1] Implement the verify flow in `core/src/cloud/keys.rs`: call the adapter's `check`, update `state` and `verifiedAt` on success, `Rejected` and `NoCredit` only (offline, unreachable and unexpected leave a saved key as it was), store the live language list in the app data cache, return `{ ok, languages }` or `{ ok: false, reason, message }` with reasons `rejected`, `noCredit`, `offline`, `unreachable`, `unexpected`
- [x] T022 [US1] Add `src-tauri/src/commands/cloud.rs` with `cloud_services`, `cloud_keys_list`, `cloud_key_add`, `cloud_key_verify`, `cloud_key_remove`, `cloud_keys_reorder` per `contracts/ipc-commands.md`; the secret is taken as a string argument and passed straight into `Secret`; register in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs`; set up the OS store once in start-up
- [x] T023 [US1] Add `cloudServices`, `cloudKeysList`, `cloudKeyAdd`, `cloudKeyVerify`, `cloudKeyRemove`, `cloudKeysReorder` wrappers and their types to `src/app/ipc.ts` and `src/features/cloud/types.ts`
- [x] T024 [US1] Build `src/features/cloud/CloudCard.tsx` and `src/features/cloud/KeyDialog.tsx` from `src/components/ui` (card, dialog, input, select, badge, button, alert; install `alert-dialog` from the shadcn registry if the remove confirmation needs it): key list with masked value, Add, Verify, Remove, session-only offer; texts in `src/i18n/en.json` under `cloud.*`; mount the card in `src/features/settings/SettingsScreen.tsx`
- [x] T025 [P] [US1] Add Playwright spec `e2e/cloud.spec.ts` (fake core, shared helper in `e2e/support/fakeCloudCore.ts`): add a key, verify success, reload and see it masked, verify a rejected key and read the reason, remove it
- [x] T026 [P] [US1] (macOS run passed; Windows NOT run) Write ignored test `core/tests/keystore_os.rs`: store, read and delete one real entry through `OsKeyStore` (run by hand with `--ignored` on each OS; record the result in `docs/verification/003.md`)

**Checkpoint**: Story 1 works end to end with the fake server; secrets are only in the store.

---

## Phase 4: User Story 2 - Dub with a cloud step, knowing what leaves the computer (P1)

**Goal**: translation and speech through a verified service, behind a consent notice, labelled, with usage recorded. Default is local with zero traffic.

**Independent Test**: with a verified key, start a dub that uses cloud, read the notice, agree, see it labelled and finished; decline and see nothing sent.

### Tests first

- [x] T027 [P] [US2] Write test `core/tests/cloud_consent.rs`: no record means consent is required; a granted record stores service, step, `agreedAt`, `agreedChars` and `noticeVersion`; a later run needs no new question when its estimate is at most 5 times `agreedChars`, and asks again when it is more than 5 times; a lower `noticeVersion` asks again; withdraw removes the record; granting again replaces `agreedChars`; consent for translation does not cover speech; the file holds no key
- [x] T028 [P] [US2] Write test `core/tests/cloud_estimate.rs`: the estimate is `durationSeconds × 15` characters per cloud step and is rounded to the nearest hundred for display
- [x] T029 [P] [US2] Write test `core/tests/cloud_adapters.rs` (the `translate`, `voices` and `speak` part): for both adapters, `translate` returns one string per input in order for a batch of the size limit, a reply with another count is `Unexpected`, rate limit and rejection map to their kinds with `Retry-After` kept in seconds (seconds and HTTP-date forms), the language codes are mapped (`zh` to the vendor code, `pt` to the vendor code), the key is only in a header; `speak` returns a 24 kHz 16-bit mono WAV; the Gemini voice is set from the curated list, the Azure voice from the live list
- [x] T030 [P] [US2] Write test `core/tests/cloud_off_by_default.rs`: a complete dub on the fake sidecar with default settings and a verified key present makes zero requests to the fake cloud server (SC-003)
- [x] T031 [P] [US2] Write test `core/tests/cloud_pipeline.rs`: with engines set to a service and consent granted, a run translates through the fake server in batches of at most 50 lines or 4,000 characters and writes `translated.partial.json` after every batch; speech writes one `clips/<id>.wav` per line, fitted by ffmpeg with the 1.3 speed-up limit and `tooLong` beyond, with the same file names, `audioMs` and flags as the local path; the engine id is in both stage input hashes, so changing the engine redoes the stage and nothing else; a run without consent fails with `CLOUD_CONSENT_REQUIRED` before any request; the usage record has service, chars sent, requests and keys used, and contains no key and no text; switching one step's engine redoes only that step and the stages after it, and leaves the other steps' finished work in place (spec edge "Offline during a run")
- [x] T054 [P] [US2] Write test `core/tests/cloud_engines.rs`: a fresh settings file has `local` for both steps; setting a service id is refused without a verified key for it (`CLOUD_NO_KEY`) and refused for a step the service does not list; removing the last key of the chosen service makes the next run fail with `CLOUD_NO_KEY` instead of switching engine; an old `settings.json` without `engines` still loads
- [x] T032 [P] [US2] Write Vitest `src/features/cloud/ConsentDialog.test.tsx` and extend `src/features/dub/Progress.test.tsx`: the dialog names the service and step, says what is sent (text of the lines, "about N characters") and what is not (video, original audio, other projects), links the terms, and has Agree and Not now; the stage list shows "Cloud: <service>" next to a cloud stage

### Implementation

- [x] T033 [P] [US2] Implement `core/src/cloud/estimate.rs`: characters per cloud step from the video duration, constant `CHARS_PER_SECOND = 15` with a comment pointing to research R6
- [x] T034 [P] [US2] Implement `core/src/cloud/consent.rs`: `cloud-consent.json` read/write, `needs_consent(service, step, estimate)` returning `first`, `withdrawn`, `noticeChanged`, `largerRun` or none, grant, withdraw, `NOTICE_VERSION` constant
- [x] T035 [US2] Implement `translate` for Gemini in `core/src/cloud/adapters/gemini.rs`: one request per batch to the model named in the service data, structured output asking for a JSON list with exactly one string per input line, an instruction to keep names and numbers unchanged and to return only the translation, rejection of any reply whose count differs
- [x] T036 [US2] Implement `translate` for Azure in `core/src/cloud/adapters/azure.rs` (Translator v3, `from` and `to`, one element per line)
- [x] T037 [US2] Implement `speak` and voice choice for both adapters: Gemini uses the `voice` field and returns WAV from the non-streaming call; Azure asks for `riff-24khz-16bit-mono-pcm` and reads the voice list (cached under the app data folder); pick the first voice of the speaker's gender, any voice if none matches (research R8)
- [x] T038 [US2] Implement `core/src/cloud/engine.rs`: `CloudEngine` with `translate_batches` (batching limits from the service data, writing `translated.partial.json` after each batch, skipping batches already done) and `speak_lines` (one request per line, skipping clips that exist, fitting with the Rust `Ffmpeg` helper to the line's slot with a speed-up of at most 1.3), and a usage record updated as it goes (chars sent counted once per request, not per retry)
- [x] T039 [US2] Add `engines: { translate, speak }` (default `"local"`) to `core/src/settings/model.rs` (bump nothing that old files cannot read; a missing field means `"local"`) and a `CloudUsage` list (`#[serde(default)]`) to the dub data in `core/src/project/dub.rs`
- [x] T056 [US2] Implement the engines settings in `core/src/settings/model.rs` and `core/src/settings/store.rs` (read, validate against `config/cloud-services.json` and the key list, write); `cloud_engines_get` and `cloud_engines_set` (T042) call it. Do this before T040 and T042 and make T054 pass
- [x] T040 [US2] Edit `core/src/pipeline/plan.rs` and `core/src/pipeline/run.rs`: when the engine for a step is a service, `translate` and `synthesize` call `CloudEngine` instead of the sidecar; add the engine id to the stage input hashes; fail with `CLOUD_NO_KEY` when no verified key exists and with `CLOUD_CONSENT_REQUIRED` before the first request when consent is missing; never fall back to a different engine by itself; the video file and the original audio are never read by the cloud code
- [x] T041 [US2] Edit `core/src/pipeline/preflight.rs`: add `cloud: Vec<CloudPreflight>` (`step`, `serviceId`, `serviceName`, `estimatedChars`, `needsConsent`, `reason`, `hasKey`) per `contracts/ipc-commands.md`; `check_start` turns a missing consent into `CLOUD_CONSENT_REQUIRED` listing the steps
- [x] T042 [US2] Add `cloud_notice`, `cloud_consent_list`, `cloud_consent_grant`, `cloud_consent_withdraw`, `cloud_usage`, `cloud_engines_get`, `cloud_engines_set` to `src-tauri/src/commands/cloud.rs`; `cloud_engines_set` refuses a service with no verified key (`CLOUD_NO_KEY`); `dub_start` and `dub_resume` in `src-tauri/src/commands/dub.rs` pass the key store, services and consent into the run configuration
- [x] T043 [US2] Build `src/features/cloud/ConsentDialog.tsx` (shadcn `Dialog`) and wire it into `src/features/dub/NewDub.tsx`: when the preflight says a step needs consent, the Start button opens the dialog first; Agree grants and starts, Not now offers the local option if the machine has one or explains what to do; show the estimated amount on the start screen for every cloud run
- [x] T074 [US2] Build the first part of `src/features/cloud/EngineSelect.tsx` (shadcn `Select`) and mount it in `CloudCard.tsx`: per step, "Local" or any service with a verified key; a service without a verified key asks the user to add and verify one first; add a Vitest case in `src/features/cloud/EngineSelect.test.tsx` (default "Local" for both steps; unverified service refused). Without this, no user can start a cloud dub from the app
- [x] T044 [US2] Edit `src/features/dub/Progress.tsx` and `src/features/dub/Result.tsx`: label each cloud stage "Cloud: <service>"; the result details list the steps that used a service, the characters sent and the number of requests (from `cloud_usage`); add the `cloud.*` texts to `src/i18n/en.json`
- [x] T045 [P] [US2] Extend `e2e/cloud.spec.ts` (fake core): start a dub with the engine set to cloud, see the consent dialog with the amount, agree, see the "Cloud: <service>" label in the stage list and in the result details; decline and see that nothing started

**Checkpoint**: a cloud dub works behind consent and is labelled; with defaults nothing is sent.

---

## Phase 5: User Story 3 - Several keys, rate limits and failures without losing work (P2)

**Goal**: move to the next key on a limit or failure, wait up to 10 minutes when all are limited, keep finished work.

**Independent Test**: two keys for one service where the first is limited partway; the run finishes on the second with no line missing or doubled.

### Tests first

- [x] T046 [P] [US3] Write test `core/tests/cloud_rotation.rs` with the fake clock and fake server: first key returns 429 partway, the second takes over from the next unfinished batch and no line is missing or translated twice; a key that returns 401 or 402 is set aside for the rest of the run and named; all keys limited with `Retry-After: 90` waits 90 s (fake time) and starts again from the first key; with no `Retry-After` it waits 30 s; after 10 minutes of waiting in total for one request the run pauses with `CLOUD_KEYS_EXHAUSTED` and `retryAfterSeconds`, resumable; every key rejected gives `CLOUD_AUTH_FAILED` listing key labels and reasons; offline gives `CLOUD_OFFLINE` and a resumable run; a key set aside is `ready` again at the start of the next run; keys of another service are never used (FR-011); with several keys, no key ever has more than one request in flight at a time
- [x] T047 [P] [US3] Write test `core/tests/cloud_resume.rs`: stop a cloud run in the middle of a section and resume it after a close and reopen; the finished batches and clips are kept, nothing finished is requested again, and the usage record continues instead of restarting
- [x] T048 [P] [US3] Write Vitest `src/features/dub/Progress.test.tsx` additions: a `dub:cloud` event of kind `switched`, `setAside`, `waiting` (with seconds) and `resumed` shows a one-line plain message naming the key label; a paused run with `CLOUD_KEYS_EXHAUSTED` shows how long to wait and a Resume button

### Implementation

- [x] T049 [US3] Implement `core/src/cloud/rotation.rs`: `KeyPool` for one service in the user's order with per-run key state `ready | limitedUntil(t) | setAside(reason)`, `Clock` and `Sleeper` injected, the failure table from research R4 (429 or 503 with `Retry-After` moves on; 401/403 and 402 set aside; other 5xx retried once after 2 s then treated as a limit; offline pauses), the 10-minute total wait per request, and a result type the engine can map to the error codes
- [x] T050 [US3] Use the pool in `core/src/cloud/engine.rs`: every adapter call goes through it; a batch or clip that failed is retried on the next key without redoing finished ones; the usage record counts `switches`, `waitedSeconds` and `keysUsed`; at most one request in flight per key
- [x] T051 [US3] Add the `dub:cloud` event (`switched`, `setAside`, `waiting`, `resumed`) to `core/src/pipeline/run.rs` (`RunEvent::Cloud`), emit it at most once a second, send a `waiting` heartbeat at least every 5 s while the run waits on a limit (reuse 002's `core/src/pipeline/progress.rs`; add a case to `core/tests/cloud_rotation.rs` with the fake clock), and send it from `src-tauri/src/commands/dub.rs`; map pool results to `CLOUD_KEYS_EXHAUSTED`, `CLOUD_AUTH_FAILED` and `CLOUD_OFFLINE`, leaving the run in the resumable failed state with finished work kept
- [x] T052 [US3] Edit `src/features/dub/useDubRun.ts`, `src/features/dub/Progress.tsx` and `src/features/dub/RunError.tsx`: show the cloud messages, and for `CLOUD_KEYS_EXHAUSTED` show the wait and a Resume button; add the texts to `src/i18n/en.json`
- [x] T053 [P] [US3] Extend `e2e/cloud.spec.ts` (fake core): a run that switches key shows the message and finishes; a run where all keys are limited shows the wait and then the paused state with Resume

**Checkpoint**: rate limits and key failures no longer lose work.

---

## Phase 6: User Story 4 - Choose local or cloud per step (P2)

**Goal**: per-step engine choice in Settings, consent withdrawal, usage in the project.

**Independent Test**: switch translation to a cloud service, run and see the label; switch back and see nothing sent.

### Tests first

- [x] T055 [P] [US4] Extend Vitest `src/features/cloud/EngineSelect.test.tsx` (the default and unverified-key cases are in T074): a key list reorder changes the order sent to the core; "Withdraw consent" removes the agreement and the next dub asks again

### Implementation

- [x] T057 [US4] (the amount sent shows in the project details, not in Settings) Extend `src/features/cloud/EngineSelect.tsx` (first part built in T074) with the key list per service with reorder (up/down buttons), the consent records with Withdraw, and the amount of text sent by the last cloud runs
- [x] T058 [US4] Check that the project details in `src/features/dub/Result.tsx` (built in T044) list which steps used which service and how much text was sent for a finished cloud project, and fix any gap; no second edit of the same list
- [x] T059 [P] [US4] Extend `e2e/cloud.spec.ts` (fake core): fresh install shows Local for both steps; switch translation to the service, run and see the label; switch back and see no label and no request; withdraw consent and see the dialog again

**Checkpoint**: users control which steps use the cloud.

---

## Phase 7: User Story 5 - Hybrid-mode guidance when the computer is too weak for a step (P3)

**Goal**: the language pair check offers the cloud engine, or a set-up hint, and cloud-only languages appear in the list.

**Independent Test**: on a pair the local models cannot do on this machine, the fallback list includes the cloud service; choosing it starts the run after the notice.

### Tests first

- [x] T060 [P] [US5] Extend `core/tests/languages.rs`: with a verified key for a service that covers a step the local catalogue cannot (for example Khmer speech), `check_pair` adds the fallback `cloudEngine` with that service's id; with the service listed but no verified key it adds `setupCloudKey`; with no service covering it, neither; `languages_list` includes a language only after the service has a verified key; adding a language to a service in `config/cloud-services.json` makes it appear with no other change (SC-008)
- [x] T061 [P] [US5] Extend `src/features/dub/LanguagePicker.test.tsx` and add `src/features/dub/PairCheck.test.tsx` cases: the cloud fallback is shown with the service name and, when chosen, marks the step as cloud; the set-up hint links to Settings

### Implementation

- [x] T062 [US5] Edit `core/src/languages/mod.rs` and `core/src/languages/pair.rs`: read the cloud services (language lists from data, replaced by the cached live list after a verify), add `FallbackKind::CloudEngine` and `FallbackKind::SetupCloudKey`, include cloud-only languages in the list for services with a verified key; the choice arrives as `engines` in the existing fallback choices and does not change the saved default
- [x] T063 [US5] Edit `src-tauri/src/commands/languages.rs` and `src/features/dub/PairCheck.tsx`: show the new fallbacks with `languages.fallback.cloudEngine` / `languages.fallback.setupCloudKey` texts in `src/i18n/en.json`; choosing the cloud fallback sets the run's engine for that step and shows the consent dialog from T043 before starting
- [x] T064 [P] [US5] Extend `e2e/languages.spec.ts` (fake core): a pair only a cloud service covers shows the cloud fallback; choosing it leads to the consent dialog

**Checkpoint**: hybrid mode is discoverable from the language check.

---

## Phase 8: Polish and cross-cutting

- [x] T075 Run `core/tests/cloud_no_leak.rs` (written in T065) and confirm it passes; fix any leak it finds before any other Polish task
- [ ] T066 [P] Confirm the amount estimate (research R6): run the two French fixtures through the local pipeline, count the characters of the transcript, and adjust `CHARS_PER_SECOND` in `core/src/cloud/estimate.rs` if the estimate is off by more than 30%; record the numbers in `docs/verification/003.md`
- [x] T067 [P] Run the licence jobs (`cargo deny check licenses`, the npm lockfile check) and update `docs/dependencies.md` and the About screen notices for the two services (terms links, "your key, your bill, the service's terms apply")
- [ ] T068 [P] Idle RAM and installer size: confirm the new crates keep the release binary under the 25 MB budget and the idle RAM no worse than 002; record in `docs/verification/003.md` (close any running copy of the app first: the single-instance plugin hides a second one)
- [x] T069 Run Constitution Check against the finished code and record it in `docs/verification/003.md` (Principles I to IX)
- [ ] T070 Run the full quickstart (steps 1 to 8) on macOS; on Windows once a Windows machine is available (carried from 002); on Windows also run the ignored `keystore_os` test (T026) and repeat the no-leak search by hand (SC-002 on both systems); record results and any bug in `docs/verification/003.md`. Feature 003 is not closed until this Windows run passes (Constitution I)
- [ ] T071 [P] Write ignored test `core/tests/cloud_real.rs` and run it by hand with real keys (`FLICK_TEST_GEMINI_KEY`, `FLICK_TEST_AZURE_KEY`, `FLICK_TEST_AZURE_REGION`): verify, translate three French lines, speak one line, for each service; print the language lists each service reports and update `checkedOn` and any entries in `config/cloud-services.json` that differ
- [ ] T072 Owner listening test (research R8, quickstart step 9): dub `fixtures/clip-20s-fr.mp4` into Khmer and English through Gemini, play every voice in the curated list, mark the acceptable ones, correct the female/male assignment and the model ids in `config/cloud-services.json`, remove the "unverified" comment, and compare the Gemini French-to-English translation with the local M2M100 result; record the verdict in `docs/verification/003.md`. If no Gemini Khmer voice is acceptable, record that Khmer cloud speech is unsolved and raise the local-model decision with the owner
- [x] T073 [P] Update `docs/roadmap.md` with a "Feature 003 hand-off" section: services supported, what was verified on which OS, what the owner decided about voices, what remains open

---

## Dependencies

- Phase 1 then Phase 2 (blocks all).
- US1 depends only on Phase 2. US2 depends on US1 (keys and `check`) and on Phase 2. US3 depends on US2 (the engine it wraps). US4 depends on US1 and US2. US5 depends on US1 and the data in T002, and uses the consent dialog from US2 (T043).
- Inside US2: T054 and T056 (engine settings) come before T040 and T042; T074 (engine choice screen) comes before T043 and T045, so a cloud dub can be started from the app after US2.
- Order: Phase 1 → Phase 2 → US1 → US2 → US3 → US4 → US5 → Polish. US4 and US5 are independent of each other and can go in either order or in parallel once US2 is done.
- T065 (no-leak test) is written in Phase 2, fails until the stories are done, and is confirmed in T075.
- The feature closes only after the Windows run in T070.

## Parallel opportunities

- Phase 1: T002, T003, T004, T005 together.
- Phase 2: T006, T008, T010, T011, T012, T013 together.
- US1 tests: T015, T016, T017 together; adapters T019 and T020 together; T025 and T026 together.
- US2 tests: T027 to T032 together; T033 and T034 together; T045 alone at the end.
- US3 tests: T046, T047, T048 together.
- US4 tests: T055 alone. US5 tests: T060 and T061 together.
- Polish: T066, T067, T068, T071, T073 together.

## Implementation strategy

- **MVP**: Phase 1, Phase 2, US1 (a key can be added, checked and removed safely) then US2 (a cloud dub behind consent). US1 alone is a testable slice but only US1 plus US2 delivers the point of hybrid mode.
- Then US3 (long films survive rate limits), US4 (settings control), US5 (discoverability), Polish.
- Test-first throughout; keep the no-leak test failing from the start so a secret leak cannot slip in during any story.
- Owner decision pending at T072 (Gemini Khmer voices): do not remove the "unverified" marker on voices or model ids until it is done.

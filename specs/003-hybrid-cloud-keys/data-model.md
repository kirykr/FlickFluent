# Data Model: Hybrid Cloud Engines and API Keys

Nothing here stores a secret except the operating system's secure storage. Every other file named below may be read, copied or shared without exposing a key.

## Cloud Service (static data: `config/cloud-services.json`)

| Field | Type | Notes |
|-------|------|-------|
| id | string | stable, e.g. `google-gemini`, `azure-ai` |
| name | string | shown to the user |
| steps | `("translate" \| "speak")[]` | steps this service offers; one key serves all of them |
| needsRegion | boolean | Azure: the key is paired with a region such as `westeurope` |
| termsUrl | string | link to the vendor's terms, shown in the notice |
| dataNotice | `{ translate?: string, speak?: string }` | i18n keys describing what is sent for each step |
| endpoints | object | base URLs per step and for the key check; tests replace them |
| languages | `{ translate: string[], speak: string[], checkedOn: string }` | codes known when the file was written; replaced by the live list after a key is verified |
| limits | `{ batchLines, batchChars }` | request sizes (R5) |
| models | `{ translate?: string, speak?: string }` | model ids sent to the service (for example the Gemini text and speech models); data, so a rename is a file edit |
| voices | `{ id: string, gender: "female" \| "male" }[]` \| absent | curated list for services without a usable voice list (Gemini, R8); absent when the service has a `voices` endpoint |

Validated by `contracts/cloud-services.schema.json`. Adding a service is a new entry plus one adapter (`contracts/cloud-adapter.md`); no screen changes.

## Cloud Key (`cloud-keys.json` in the app data folder, plus one OS secure-storage entry per key)

| Field | Type | Notes |
|-------|------|-------|
| id | uuid | also the account name in secure storage (`cloud-key:<id>`) |
| serviceId | string | which service |
| label | string | the user's own name for it; unique within the service |
| region | string \| null | only for services that need one |
| last4 | string | last four characters, the only part ever shown |
| fingerprint | string | SHA-256 of the secret, to refuse the same key twice; **not reversible**, never shown |
| order | integer | position among keys of the same service (0 is tried first) |
| state | `untested \| verified \| rejected` | result of the last "Verify connection" |
| verifiedAt | timestamp \| null | |
| storage | `os \| session` | `session` keys exist in memory only and are not in `cloud-keys.json` after exit |

Rules: label and fingerprint unique per service; removing a key deletes its secure-storage entry first, then the row. `state` says only what the last check said; a later failure does not rewrite it (spec edge case).

**Run-time key state** (not saved): per run and service, each key is `ready`, `limitedUntil(t)` or `setAside(reason)`. A new run starts every key at `ready`.

## Consent Record (`cloud-consent.json` in the app data folder)

| Field | Type | Notes |
|-------|------|-------|
| serviceId, step | string | the pair the agreement is for |
| agreedAt | timestamp | |
| agreedChars | integer | amount shown when the user agreed (estimate, R6) |
| noticeVersion | integer | bumped when the notice text changes; an older version asks again |

Rules: one record per (service, step). Granting again replaces it. A run needs a record for every cloud step it uses. Ask again when no record exists, `noticeVersion` is older, or the run's estimate is more than 5 × `agreedChars`.

## Engine Choice (`settings.json`)

`engines: { translate: "local" | <serviceId>, speak: "local" | <serviceId> }`. Default `"local"` for both. A service id is accepted only when a verified key for it exists at the moment of choosing; if the keys are later removed the run reports `CLOUD_NO_KEY` instead of falling back by itself (FR-009).

## Usage Record (in the project, `dub.cloudUsage`, optional)

| Field | Type | Notes |
|-------|------|-------|
| step | `translate \| speak` | |
| serviceId, serviceName | string | |
| chars | integer | text actually sent (summed over requests, not retries) |
| requests | integer | |
| keysUsed | string[] | key labels, in order of first use |
| switches | integer | times the run moved to another key |
| waitedSeconds | integer | total waiting on rate limits |
| runId | string | |

Never contains a key, a fingerprint, or any sent text. A resumed run adds to the same record.

## Estimate (computed, not stored)

`chars ≈ durationSeconds × 15` for each cloud step in the run (R6). Shown as "about N characters". `Preflight.cloud[].estimatedChars`.

## State transitions

- Key: `untested → verified | rejected` on "Verify connection"; any state → removed.
- Consent: absent → agreed → (withdrawn → absent). Notice version or amount change → "ask again" without deleting the record.
- Run with a cloud step: `queued → running → (waiting on limit → running) → completed`; `waiting` longer than 10 minutes → `paused` (state `failed` with code `CLOUD_KEYS_EXHAUSTED`, resumable).

## Validation rules

- Key text is trimmed of spaces and line breaks, must not be empty, and must be at most 512 characters.
- A service that needs a region refuses a key without one.
- A label may not be empty or longer than 40 characters.
- The engine for a step must be `local` or a service that lists that step.

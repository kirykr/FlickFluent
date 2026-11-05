# Contract: Tauri commands and events added by feature 003

Names are final. Errors use the shape from 001 (`code`, `messageKey`, `details`). No command ever returns, logs or emits a secret.

## Services and keys

| Command | Input | Output |
|---------|-------|--------|
| `cloud_services` | none | `CloudService[]` with `{ id, name, steps, needsRegion, termsUrl, languages }` |
| `cloud_keys_list` | none | `KeyInfo[]` = `{ id, serviceId, label, region, last4, order, state, verifiedAt, storage }` |
| `cloud_key_add` | `{ serviceId, label, secret, region?, sessionOnly? }` | `KeyInfo` (state `untested`). `secret` is trimmed, checked, stored, and dropped. Errors: `KEY_INVALID`, `KEY_DUPLICATE`, `KEY_STORE_UNAVAILABLE` (when the OS store refuses and `sessionOnly` is not set) |
| `cloud_key_verify` | `{ keyId }` | `{ ok: true, languages }` or `{ ok: false, reason, message }` with `reason` one of `rejected`, `noCredit`, `offline`, `unreachable`, `unexpected`. Updates the key's `state` |
| `cloud_key_remove` | `{ keyId }` | none. Deletes the secure-storage entry first |
| `cloud_keys_reorder` | `{ serviceId, keyIds }` | `KeyInfo[]` for that service |

## Engines and consent

| Command | Input | Output |
|---------|-------|--------|
| `cloud_engines_get` | none | `{ translate, speak }`, each `"local"` or a service id |
| `cloud_engines_set` | `{ step, engine }` | the new choice. Error `CLOUD_NO_KEY` if the service has no verified key |
| `cloud_notice` | `{ serviceId, step, estimatedChars }` | `{ serviceName, step, sends: key[], doesNotSend: key[], termsUrl, estimatedChars }` (i18n keys, no free text from a vendor) |
| `cloud_consent_list` | none | `{ serviceId, step, agreedAt, agreedChars }[]` |
| `cloud_consent_grant` | `{ serviceId, step, estimatedChars }` | the stored record |
| `cloud_consent_withdraw` | `{ serviceId, step }` | none |
| `cloud_usage` | `{ handle }` | the project's `CloudUsage[]` |

## Changes to feature 002 commands

- `dub_preflight` result gains `cloud: CloudPreflight[]`, one per step that will use a cloud engine: `{ step, serviceId, serviceName, estimatedChars, needsConsent, reason }` where `reason` is `first`, `withdrawn`, `noticeChanged` or `largerRun`; plus `hasKey: boolean`.
- `dub_start` and `dub_resume` fail with `CLOUD_CONSENT_REQUIRED` (details: the steps) if any cloud step in the run lacks a current consent. They never send anything before consent.
- `languages_check_pair` gains the fallback kind `cloudEngine` (`labelKey`, `serviceIds`) when a service with a verified key covers a step the local models cannot, and `setupCloudKey` when a service covers it but no verified key exists. The user's choice arrives as `engines` in the existing fallback choices; it does not change `settings.json`'s default.
- `languages_list` includes languages offered only by a service that has a verified key.

## Events

| Event | Payload | When |
|-------|---------|------|
| `dub:cloud` | `{ runId, kind, step, serviceId, keyLabel?, reason?, waitSeconds? }` | `kind` is `switched` (moved to another key), `setAside` (key out for this run, with `reason`), `waiting` (all keys limited, with `waitSeconds`), `resumed` (wait over) |

Sent at most once a second, like `dub:progress`. `dub:stage` and `dub:progress` are unchanged; a cloud stage is labelled by the UI from the run's engine choice, not by a new event.

## New error codes (stable)

`CLOUD_CONSENT_REQUIRED`, `CLOUD_NO_KEY`, `CLOUD_KEYS_EXHAUSTED` (details: `retryAfterSeconds`), `CLOUD_AUTH_FAILED` (every key rejected; details: key labels and reasons), `CLOUD_OFFLINE`, `CLOUD_BAD_REPLY`, `KEY_INVALID`, `KEY_DUPLICATE`, `KEY_STORE_UNAVAILABLE`. Each has a `message` key and a `nextStep` key in `src/i18n/en.json` (the 002 test that checks this covers them automatically).

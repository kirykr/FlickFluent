# Contract: cloud service adapter

One adapter per service, in `core/src/cloud/adapters/`. The pipeline, key rotation, waiting, consent and usage never see a vendor's format. Each adapter is tested against the local fake server (`core/tests/support/fake_cloud.rs`) before it is written (Constitution VIII).

## Trait (shape, not final code)

| Method | Input | Output |
|--------|-------|--------|
| `check(key)` | key, region | `Ok(Languages)` or a `CloudFailure` |
| `translate(key, source, target, texts)` | up to the batch limit | one string per input, same order, same count |
| `voices(key, language)` | | voices with gender (live list, or the curated list from the descriptor) |
| `speak(key, voice, text)` | | WAV bytes, 24 kHz, 16-bit, mono |

Every method returns either a result or a `CloudFailure { kind, retryAfter?, message }`.

## Failure kinds (the only way the rest of the code learns what went wrong)

| Kind | Used for | Rotation action |
|------|----------|-----------------|
| `RateLimited` | HTTP 429, 503 with `Retry-After` | next key, then wait (R4) |
| `Rejected` | HTTP 401, 403 | set the key aside this run |
| `NoCredit` | HTTP 402, quota or billing text | set the key aside this run |
| `Unavailable` | other 5xx, timeout, reset | retry once, then treat as `RateLimited` |
| `Offline` | no route, DNS failure | pause the run |
| `Unexpected` | anything else, bad JSON, wrong count | stop the step with the (scrubbed) message |

`message` is always passed through `scrub` so it cannot contain the key (R11).

## Rules every adapter must meet

1. The key goes in a request header, never in a URL or a log line.
2. Returns exactly one translation per input text, in order; a different count is `Unexpected`.
3. Sends only the texts given; adds no identifying data of its own beyond what the vendor requires (region header for Azure).
4. Honours `Retry-After` (seconds or HTTP date) and reports it in `CloudFailure.retryAfter`.
5. Base URLs come from the service descriptor, so tests can point at the fake server.
6. A translation by a language model MUST ask for exactly one string per input in a structured reply and MUST reject a reply with another count.
7. Language codes in and out are the app's codes; mapping to the vendor's codes (for example `zh` to `zh-CN`, `pt` to `pt-PT`) lives inside the adapter.

## Fake server behaviours the tests rely on

`ok`, `rate_limit(n, retry_after)`, `reject`, `no_credit`, `slow(ms)`, `drop`, `junk`, `wrong_count`, plus a request counter and a record of the headers received (so a test can prove the key was sent only in the header).

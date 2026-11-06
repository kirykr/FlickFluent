# Research: Hybrid Cloud Engines and API Keys

Decisions for feature 003. Dated 2026-11-05. Items marked **verify** were read from vendor documentation or search results and must be confirmed against the live service when the adapter is written (the verify step in Settings does this at run time for every user).

## R1. Where keys are stored

- **Decision**: `keyring-core` 1.0.1 with `apple-native-keyring-store` 1.0.2 (macOS Keychain) and `windows-native-keyring-store` 1.1.0 (Windows Credential Manager). All are `MIT OR Apache-2.0`, checked on crates.io today. One entry per key: service name `app.flickfluent.desktop`, account `cloud-key:<keyId>`. The secret is the only thing stored there.
- **Rationale**: These are the OS stores the constitution asks for (IX). `keyring` 4 split into a core crate plus one crate per store, so we depend on exactly the two stores we ship. Non-secret facts (label, service, region, order, last four characters, state) live in a plain `cloud-keys.json` in the app data folder, so the list can be shown without unlocking the store.
- **Alternatives**: `keyring` 3 as one crate (older layout, superseded). `tauri-plugin-stronghold` (own vault and its own password, heavier, a second secret to lose). An encrypted file with a key derived from the machine (still a key on disk, fails the "OS secure storage" rule). Plain file: rejected by FR-003.
- **When the store fails** (locked, missing, denied): the key is **not** saved. The Add dialog offers "use for this session only", which holds the secret in memory and clears it on exit. No file fallback, ever.
- **Verify**: the exact `Entry` API of `keyring-core` 1.0.1 (docs.rs did not show the method list); set the default store once at start-up through `set_default_store`.

## R2. Which services in the first release

- **Decision** (changed 2026-11-05 after the owner rejected the Azure Khmer voices): two services, both described in data (`config/cloud-services.json`):
  1. **Google Gemini API** (one Google AI Studio key): translation by a Gemini text model with a fixed output shape (a JSON list with one string per input line), and speech by Gemini text-to-speech. One key serves both steps, which is the clarified rule (Q1).
  2. **Azure AI services** (one multi-service key plus a region): Translator v3 and Speech text to speech. Kept for its wide language and voice lists (hundreds of voices), not for Khmer.
- **Why this changed**: the owner does not like the Azure Khmer voices (`km-KH-PisethNeural`, `km-KH-SreymomNeural`). Alternatives checked against vendor documentation on 2026-11-05:

  | Service | Khmer speech | Source |
  |---------|--------------|--------|
  | Gemini TTS (`gemini-3.8-flash-tts`, `gemini-3.8-flash-lite-tts`) | **Yes**, listed; Flash TTS covers 130+ languages, Flash-Lite 100+ | `aistudio.google.com/docs/speech-generation` |
  | Google Cloud Text-to-Speech (classic) | No: its voice list goes from Kannada to Korean | `docs.cloud.google.com/text-to-speech/docs/voices` |
  | ElevenLabs v3 | Not listed (70+ languages named, Khmer absent; list says "including") | `elevenlabs.io/docs/overview/models` |
  | OpenAI TTS | Not listed (57 languages, following Whisper) | OpenAI docs |
  | Azure Speech | Yes, two voices (rejected by the owner) | Microsoft |
  | MMS-TTS Khmer fine-tunes, local | Experimental single-speaker models; CC-BY-NC for some; not production-ready by their own cards | Hugging Face cards |

  Gemini is the only cloud option with documented Khmer that the owner has not ruled out. **Whether its Khmer voices sound good is not known from documents**: someone must listen (see the listening test in `quickstart.md`, step 9). If they are not acceptable either, Khmer speech stays unsolved in the cloud and the next move is a local model (a separate decision for the owner; the 002 research already lists MMS-TTS Khmer as a non-commercial opt-in).
- **Left out for now**: DeepL (its Khmer support could not be confirmed), ElevenLabs and OpenAI (no documented Khmer), classic Google Cloud Translation and Text-to-Speech (the Gemini key covers both steps with one product). Each is a new data entry and one adapter, with no screen change (SC-008).
- **Alternatives**: classic Google Cloud APIs (no Khmer voice); one service only (no cover if a vendor changes terms); "bring your own URL" (security and support burden, rejected).
- **Verify**: the live model ids (`gemini-3.8-flash-tts` and the text model used for translation) and the Khmer language entry through a real call; the model ids are data in `config/cloud-services.json`, not code, so a rename is a file edit.
- **Translation by a language model**: because a language model could return the wrong number of lines or add commentary, the request asks for a JSON list with exactly one string per input line (structured output) and the adapter rejects any reply whose count differs (`Unexpected`, step stops, finished batches kept). Names and numbers are passed through unchanged by instruction. This is the main quality risk of the Gemini translation path; the opt-in real-service check compares it with the local M2M100 result on the French fixture.

## R3. Where cloud calls run

- **Decision**: in the Rust core with `reqwest` (already a dependency), never in the Python sidecar.
- **Rationale**: keeps secrets out of a second process, its stdin and its stderr tail (which the core shows in errors); one place for rotation, waiting and redaction; HTTP is simple. The sidecar protocol is unchanged.
- **Alternatives**: calling from Python workers (secrets cross a process boundary and land in worker logs).

## R4. Rotation, waiting and failure rules

Same rules for both services, implemented once behind an adapter trait.

| Reply | Meaning | Action |
|-------|---------|--------|
| HTTP 429, or 503 with `Retry-After` | rate limited | try the next key; if every key is limited, wait the longest `Retry-After` (30 s if absent), then start again from the first key |
| 401, 403 | key rejected or no permission | set the key aside for this run, tell the user which key and why |
| 402, quota or billing text in the body | no credit | set aside for this run, same message |
| 5xx other, timeout, connection reset | service trouble | retry once on the same key after 2 s, then treat as rate limit |
| no network | offline | pause the run (resumable) with `CLOUD_OFFLINE` |
| anything else | unexpected | stop the step with `CLOUD_BAD_REPLY` and the service's own message (key scrubbed) |

- Waiting budget per request: **10 minutes total** (clarified), then the run pauses, resumable, and says how long the service asked for.
- A key set aside is tried again at the start of the next run (spec Story 3, scenario 5).
- The wait and the clock are injected (`Clock`, `Sleeper`) so tests run in milliseconds.

## R5. Work size and resume

- **Translation**: batches of at most 50 lines or 4,000 characters, whichever is less (both services accept far more; small batches keep a lost batch cheap). Each finished batch is written to `translated.partial.json` in the section folder, so after a stop at most one batch is redone (SC-005 asks for at most one line's work repeated across a rotation; a batch that fails is not paid for by the service, a batch that succeeded is kept, so no finished line is redone).
- **Speech**: one request per line; every clip is its own file in `clips/`, so a restart keeps all finished lines. The input hash of both stages gains the engine id, so switching engine redoes only what it must.

## R6. Amount of text and consent

- **Decision**: the amount shown before a run is an **estimate from the video's length**: `duration seconds × 10` characters for translation and the same for speech, labelled "about". The actual count is recorded afterwards in the usage record, and a run stops before a send that would take it past 5 times the agreed amount (the real text is checked per section; the projected total for the whole video is kept, so the next preflight asks for that). A consent stores the amount it was given for. A later run shows its own estimate and asks again only above 5 times the stored amount (clarified).
- **Why an estimate**: the transcript does not exist before the run begins, so an exact count is impossible at the moment the clarified rule needs it.
- **Verify**: the characters per second constant (15, then 10 after T066) against the two French fixtures at implementation; change the constant if it is off by more than 30%.
- **Alternative rejected**: pausing the run after transcription to ask with exact numbers. It matches the numbers better but breaks "before it starts" and strands the user mid-run.

## R7. Speech audio and fitting

- **Decision**: ask for WAV directly: Gemini returns `audio/wav` (RIFF header, 24 kHz, mono, 16-bit) from its non-streaming call; Azure uses `X-Microsoft-OutputFormat: riff-24khz-16bit-mono-pcm`. No decoder is needed. Fit a clip to its slot with the existing rule (speed up to 1.3× with ffmpeg `atempo`, flag `tooLong` beyond), done in the core with the ffmpeg already installed, so the cloud path produces the same `clips/<id>.wav` files and `audioMs` as the local path.
- **Alternatives**: asking the service to speak faster (SSML rate) — different behaviour per service and cannot be tested the same way.

## R8. Choosing a cloud voice

- **Decision**: per service, either a curated list in data or the service's own list. Gemini has 30 prebuilt voices named in its documentation without a stated gender, so `config/cloud-services.json` carries a curated `voices` list with a gender for each (a starting assignment from common usage; **verify by listening** before release). Azure and any later service with a `voices` endpoint use the live list, cached in the app data folder. The first voice of the speaker's gender is used (any voice if there is none of that gender). A user choice of voice is out of scope here.
- **Gemini voice and language**: the voice name is set per request and the spoken language follows the text, so one voice can speak Khmer and French; the same voice is therefore used for every language of a service.
- **Rationale**: data-driven (Constitution IV), no code per language.

## R9. Settings and project data

- **Decision**: `settings.json` gains `engines: { translate, speak }` (`"local"` or a service id; default `"local"`). A project's dub data gains an optional `cloudUsage` list (`serde(default)`, so old projects still open). Consent lives in `cloud-consent.json` in the app data folder, never inside a project (spec: not carried to another computer).
- **Rationale**: no project format bump, nothing secret in any shared file.

## R10. Testing without real services

- **Decision**: a small local HTTP server in `core/tests/support/` that imitates the four endpoints we use and can be told to answer 429, 401, 402, delay, drop, or return junk, and that counts requests. Each service descriptor holds its base URLs, so tests point them at the local server. A real-service check is opt-in (`FLICK_TEST_GEMINI_KEY`, `FLICK_TEST_AZURE_KEY`, `FLICK_TEST_AZURE_REGION`) and never runs in CI.
- **No-leak test (SC-002)**: a fake key with a unique marker runs through add, verify, run, error, rotation and removal; afterwards every file under the temp app data, project and work folders and the captured log is searched for the marker.
- **No-traffic test (SC-003)**: a full dub with the default engines; the fake server's request count must be zero.

## R11. Keeping secrets out of everything else

- **Decision**: a `Secret` wrapper type (no `Debug`, `Display` or `Serialize`; zeroed on drop), used for every key from the IPC input to the HTTP header. Errors are built from a service's own reply text passed through `scrub(text, secrets)`. The Tauri command that adds a key takes the secret as a string argument and that is the last place it exists as a plain string. Logging never prints request headers.

## R12. UI parts

- **Decision**: reuse what is installed in `src/components/ui` (card, dialog, input, select, badge, switch, alert, button, table, spinner). The consent notice is a `Dialog`. If an alert dialog is missing, install it from the shadcn registry rather than write one. Nothing here is AI chat, so AI Elements do not apply.
- Custom pieces go in `src/features/cloud/` using the theme tokens.

## R13. Language data for cloud engines

- **Decision**: `config/cloud-services.json` lists per service and step the languages known at the time of writing, with a `checkedOn` date; verifying a key replaces them with the live list (cached). The pair check reads cloud entries only for services that have a verified key and an agreed notice is not required for listing (consent is asked at run time), so the list can show "available with a cloud key" before the user commits.

# Contract: Core to Python sidecar protocol

Transport: the core starts the sidecar and exchanges UTF-8 JSON lines (one object per line) on stdin/stdout. stderr is free-form log text, captured by the core and never parsed. Protocol version: `1`. No network access is opened by the sidecar.

## Handshake
Sidecar prints on start: `{"event":"ready","protocol":1,"workers":["separate","transcribe","translate","synthesize","detect_language"],"python":"3.11.x"}`.
Core sends `{"id":"h1","cmd":"hello","protocol":1}`; sidecar replies `{"id":"h1","ok":true}` or `{"id":"h1","ok":false,"error":{"code":"PROTOCOL_MISMATCH"}}`. A mismatch stops the run with `SIDECAR_START_FAILED`.

## Request
```json
{ "id": "r12", "cmd": "run", "worker": "transcribe",
  "input":  { "audio": "<abs path>", "startMs": 0, "endMs": 300000, "language": "auto" },
  "output": { "dir": "<abs work dir>", "prefix": "sec0003" },
  "models": { "main": "<abs model dir>", "vad": "<abs model dir>" },
  "params": { "device": "cpu", "threads": 4, "computeType": "int8" } }
```
All paths are absolute and inside the work folder or the models folder; the sidecar rejects any other path (`PATH_NOT_ALLOWED`). Other commands: `cancel` `{id, target}`, `ping`, `shutdown`.

## Responses and events (sidecar to core)
- `{"id":"r12","event":"progress","progress":0.4}`: at least every 5 s while working; may be sent more often.
- `{"id":"r12","event":"item","kind":"line","data":{...}}`: partial results (transcript lines as found) so the UI can show them early.
- `{"id":"r12","ok":true,"result":{...},"files":["sec0003.json"],"stats":{"wallMs":..,"peakRamMb":..}}`: done. Every file named is already written as `*.part` and renamed by the sidecar before this message.
- `{"id":"r12","ok":false,"error":{"code":"...","message":"...","retryable":true}}`

## Worker input and result shapes
| Worker | Input | Output file(s) / result |
|--------|-------|-------------------------|
| `detect_language` | `audio`, `startMs`, `endMs` | `{ "language": "fr", "confidence": 0.93 }` |
| `separate` | `audio` (section window) | `vocals.wav`, `music.wav` (16-bit, 44.1 kHz, same length as window) |
| `transcribe` | `vocals.wav`, `language` | `sec.json`: lines `{startMs,endMs,text,language,speakerHint,confidence}` (times relative to the window; core shifts them) |
| `translate` | `lines[]` (batch up to 200), `source`, `target`, context window | `lines[].translated` in the same order |
| `synthesize` | `lines[]` with `text`, `voiceId`, `targetMs` | one `lineId.wav` per line (mono, 24 kHz or voice-native), plus `audioMs` |

## Rules
- **Cancel**: on `cancel` the worker stops at the next safe point within 5 s, removes its `.part` files, and answers `{"ok":false,"error":{"code":"CANCELLED"}}`.
- **Idempotent**: running the same request again overwrites the same output names; the core relies on this for resume.
- **Memory**: a worker holds one window at a time. If resident memory passes `params.ramCeilingMb`, it finishes the current item, reports `{"event":"restart_requested"}`, and the core restarts it.
- **Crash**: if the process exits or goes silent for 60 s without progress, the core kills it, restarts once, and retries that request. A second failure marks the section `failed` with `SIDECAR_CRASHED`.
- **Ordering**: one request at a time per sidecar in this feature. Parallel sidecars are a later optimisation.

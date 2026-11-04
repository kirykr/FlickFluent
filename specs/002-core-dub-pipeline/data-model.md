# Data Model: Core Dub Pipeline (Local)

Stored in the `.flick` project (format 2, see contracts/project-file-format-v2.md) unless marked "work folder". Times are milliseconds from the start of the source video.

## Project (extends 001)
| Field | Type | Notes |
|-------|------|-------|
| source | `{ path, durationMs, hasAudio, hasVideo, sizeBytes, fingerprint }` | `fingerprint` = hash of size + first/last 1 MB; used to detect a changed or moved file |
| sourceLanguage | string | ISO code or `auto` |
| detectedLanguage | string or null | set after detection |
| targetLanguage | string | ISO code |
| options | `{ muteOriginal, exportKind, exportName, quality }` | `exportKind` is `video` or `audio`; `quality` is `fast` or `standard` |
| speakers | Speaker[] | |
| lines | TranscriptLine[] | |
| run | PipelineRun or null | latest run |

## Speaker
| Field | Type | Notes |
|-------|------|-------|
| id | string | `spk1`... |
| gender | `male` / `female` / `unknown` | detected, may be changed by the user later (feature 004) |
| voiceId | string | catalogue voice id; chosen by language + gender |
| confidence | 0 to 1 | diarization confidence; low value is shown as a hint |

## TranscriptLine
| Field | Type | Notes |
|-------|------|-------|
| id | string | stable across reruns |
| sectionIndex | int | section it was found in |
| startMs, endMs | int | `endMs > startMs` |
| speakerId | string | |
| language | string | per-line language when detected (mixed-language flag) |
| text | string | original |
| translated | string or null | |
| audio | work-folder path or null | synthesized clip |
| audioMs | int or null | length of synthesized clip |
| flags | set of `overlap`, `tooLong`, `tooShort`, `mixedLanguage`, `lowConfidence` | `tooLong` when `audioMs > slot * 1.25`, `tooShort` when `< slot * 0.5` (slot = `endMs - startMs`); thresholds in config |
| status | `transcribed` / `translated` / `synthesized` / `failed` | |

## PipelineRun
| Field | Type | Notes |
|-------|------|-------|
| id | string | |
| startedAt, endedAt | timestamp | |
| state | `queued` / `running` / `cancelling` / `cancelled` / `failed` / `completed` | `running` found on open means an interrupted run |
| settingsHash | string | languages, models, options; a change invalidates affected stages |
| sections | Section[] | |
| stages | StageState[] | |
| error | `{ code, stage, sectionIndex, messageKey, nextStepKey, details }` or null | |

## Section
| Field | Type | Notes |
|-------|------|-------|
| index | int | |
| startMs, endMs | int | cut at silence |
| status per stage | map stage → `pending` / `running` / `done` / `failed` | |
| inputHash per stage | map stage → string | resume skips a `done` stage only if the hash matches |

## StageState
| Field | Type | Notes |
|-------|------|-------|
| stage | `extract` / `separate` / `transcribe` / `translate` / `synthesize` / `mix` / `export` | order fixed |
| status | as Section | whole-stage roll-up |
| progress | 0 to 1 | |
| etaSeconds | int or null | rolling rate over finished sections |
| startedAt, endedAt | timestamp | |

`extract`, `mix`, `export` run on the whole file in streaming mode, still checkpointed by output file.

## Language (derived at run time, not stored)
| Field | Type | Notes |
|-------|------|-------|
| code | string | ISO 639, normalised |
| name | string | from OS locale data in the UI |
| transcribe | bool | at least one installed or installable model |
| translateFrom, translateTo | bool | |
| speak | `{ voices: n, permissiveVoices: n }` | |
| quality | `good` / `fair` / `weak` per step | from catalogue |

## Model (extends 001 ModelRecord)
| Field | Type | Notes |
|-------|------|-------|
| id, name, purpose, licence, sizeBytes, url, sha256 | as 001 | |
| step | `runtime` / `ffmpeg` / `separate` / `transcribe` / `translate` / `speak` / `vad` / `speaker` | |
| languages | string[] | for `speak` the voice's language; for `translate` the supported set |
| gender | `male` / `female` / null | `speak` only |
| machineClass | `minimum` / `mid` / `fullLocal` | smallest class that runs it comfortably |
| licenceClass | `permissive` / `copyleftFile` / `nonCommercial` | non-commercial entries are off by default and show a notice |
| qualityByLanguage | map code → `good`/`fair`/`weak` | optional |
| requires | string[] | other model ids (a voice needs the runtime) |
| platforms | `{ windows: [x64, arm64], macos: [x64, arm64] }` | for runtime and ffmpeg |

## Benchmark Record (work folder and `docs/verification/002-benchmarks.md`)
| Field | Type | Notes |
|-------|------|-------|
| id, createdAt | | |
| machine | `{ os, osVersion, arch, cpuModel, cores, ramGb, freeDiskGb, accelerator, simulated }` | |
| fixture | `{ id, durationMs }` | |
| results | list of `{ stage, modelId, wallMs, peakRamMb, realTimeFactor }` | |
| verdict | `{ canRunMinimum, canRunFullLocal, notes }` | |

## State transitions
- Run: `queued → running → completed`; `running → cancelling → cancelled`; `running → failed`; app closed while `running` → read as `interrupted` on next open and offered for resume.
- Section stage: `pending → running → done`; any → `failed` (retry resets to `pending`).
- Line: `transcribed → translated → synthesized`; `failed` on error; a changed translated text resets to `translated`.

## Validation rules
- A run cannot start without a readable source with an audio track and a chosen target language.
- Source language equal to target needs explicit confirmation.
- `endMs > startMs`; lines in a section lie inside the section bounds; sections cover the file without gaps (a gap means no speech).
- Settings change after a run: only stages whose `inputHash` changed are rerun.

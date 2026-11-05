# Contract: UI to Core commands and events (feature 002)

Adds to the commands in feature 001. Same rules: every command returns `Result<T, AppError>`; `AppError` has `code`, `message` (already user-readable), `details`.

## Languages
| Command | Input | Output |
|---------|-------|--------|
| `languages_list` | `optIn?` (non-commercial model ids the user turned on) | `Language[]` (see data-model), built from the model catalogue |
| `languages_check_pair` | `source` (code or `auto`), `target`, `optIn?` | `PairCheck`: `{ ok, steps: [{ step, supported, quality, reason }], fallbacks: [{ kind, labelKey }] }` |
`fallbacks.kind` is one of `subtitlesOnly`, `genericVoice`, `pivotViaEnglish`, `optInNonCommercial`. Each fallback lists `modelIds` and `noticeKeys` for opt-in kinds. The user's choices are saved in `options.fallbacks` (`subtitlesOnly`, `genericVoice`, `pivotViaEnglish`, `optIn`) by `dub_configure`. Exports gain kind `subtitles` (`.srt`). The UI must call `languages_check_pair` and show its result before `dub_start`.

## Dub run
| Command | Input | Output |
|---------|-------|--------|
| `dub_probe_source` | `path` | `SourceInfo` `{ durationMs, hasAudio, hasVideo, container, sizeBytes }`; errors `UNREADABLE_FILE`, `NO_AUDIO_TRACK` |
| `dub_configure` | `handle`, `source`, `sourceLanguage`, `targetLanguage`, `options` | `ProjectHandle`; error `SAME_LANGUAGE` (needs `confirmSame: true`), `NO_FALLBACK_CHOSEN` |
| `dub_preflight` | `handle` | `Preflight` `{ missingModels: [{id, sizeBytes}], diskNeededBytes, diskFreeBytes, machineClass, warnings[] }`; nothing is downloaded |
| `dub_start` | `handle` (fallback choices are saved by `dub_configure` in `options.fallbacks`) | `RunId`; errors `MODELS_MISSING` (call `model_download_start` first), `NO_SPACE` `{neededBytes, freeBytes}`, `MACHINE_TOO_WEAK` `{missing[]}` |
| `dub_cancel` | `runId` | void; run goes `cancelling` then `cancelled`; finished work kept |
| `dub_resume` | `handle` | `RunId`; error `NOTHING_TO_RESUME`, `SOURCE_CHANGED` (fingerprint differs) |
| `dub_status` | `handle` | `PipelineRun` |
| `dub_export` | `handle`, `kind` (`video`/`audio`), `fileName`, `folder` | `ExportResult` `{ path, sizeBytes }`; errors `FILE_EXISTS` (needs `overwrite: true`), `NO_SPACE`, `RUN_NOT_COMPLETE` |
| `dub_discard_run` | `handle` | void; deletes the work folder |
`fileName` is checked against the same safe-name rule as `project_suggest_file_name`.

## Benchmark
| Command | Input | Output |
|---------|-------|--------|
| `bench_run` | `{ fixtureId, simulated? }` | `RunId`; progress through `bench:progress` |
| `bench_records` | none | `BenchmarkRecord[]` |
CLI: `flickfluent --bench [--json]` runs the same code and exits 0 on success. It needs the models for the machine class and offers to download them.

## Events
| Event | Payload | Rate |
|-------|---------|------|
| `dub:progress` | `{ runId, stage, stageProgress, overallProgress, sectionIndex, sectionCount, etaSeconds }` | at most once a second; at least once every 5 s while a stage is running |
| `dub:stage` | `{ runId, stage, status }` | on change |
| `dub:line` | `{ runId, line }` | when a transcript line first appears or its status changes (batched, at most 5 per second) |
| `dub:error` | `{ runId, code, stage, sectionIndex, message, nextStep }` | on failure |
| `dub:detected` | `{ runId, language }` | once, when auto-detect finds the spoken language |
| `dub:done` | `{ runId, state }` | once |
| `bench:progress` | `{ runId, stage, progress }` | at most once a second |

## Error codes (stable)
`UNREADABLE_FILE`, `NO_AUDIO_TRACK`, `NO_SPEECH_FOUND` (not fatal; run completes with the original audio mix), `SAME_LANGUAGE`, `NO_FALLBACK_CHOSEN`, `MODELS_MISSING`, `NO_SPACE`, `MACHINE_TOO_WEAK`, `SIDECAR_START_FAILED`, `SIDECAR_CRASHED` (core restarts it once per section, then fails the section), `STAGE_FAILED` `{stage, sectionIndex, reason}`, `SOURCE_CHANGED`, `NOTHING_TO_RESUME`, `FILE_EXISTS`, `RUN_NOT_COMPLETE`. Every error maps to a `message` and a `nextStep` key in `src/i18n/en.json`.

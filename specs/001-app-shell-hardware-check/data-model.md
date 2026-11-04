# Data Model: App Shell and Hardware Check

## SpecDefinition (file: `config/spec-definition.json`)
| Field | Type | Notes |
|-------|------|-------|
| version | integer | bumps when thresholds change |
| provisional | boolean | true until feature 002 benchmarks |
| minimum | SpecTier | below this = blocked |
| fullLocal | SpecTier | at or above = full local |
SpecTier: `cpuBits` (64), `cpuCores`, `ramGb`, `freeDiskGb`, `os` { windows, macos }, `accelerator` (`none` | `appleSilicon_or_discreteGpu`).
Hybrid = meets `minimum`, not `fullLocal`. Schema: `contracts/spec-definition.schema.json`.

## MachineCapabilityReport
| Field | Type | Notes |
|-------|------|-------|
| items[] | CheckItem | one per cpu, ram, disk, os, accelerator |
| mode | `fullLocal` \| `hybrid` \| `blocked` | derived |
| missing[] | CheckItem | items below minimum |
| generatedAt | timestamp | |
CheckItem: `key`, `required` (number/text), `detected` (number/text or `unknown`), `status` (`ok` \| `belowMinimum` \| `belowFull` \| `unknown`).
Rules: any `belowMinimum` gives `blocked`. Else any `belowFull` or `unknown` gives `hybrid`. Else `fullLocal`. Message text built only from `missing[]`.

## Project (file: `.flick`)
| Field | Type | Notes |
|-------|------|-------|
| manifest.formatVersion | integer | refuse to write if newer than app supports |
| manifest.appVersion | text | |
| manifest.id | UUID | |
| manifest.name | text (UTF-8) | display name, not the file name |
| manifest.createdAt / savedAt | timestamp | |
| project.data | object | empty now; later features add keys |
State: `clean` -> `dirty` (edits) -> `clean` (save). `locked` is a separate lock sidecar, not stored in the file.
Companion files: `<name>.flick.bak` (rolling backups, last 3), `<name>.flick.recover` (autosave every 30 s).

## RecentProjects (in Settings)
List of `{ path, name, openedAt }`, max 20, missing paths shown as unavailable.

## Settings (file: `settings.json` in OS config folder)
| Field | Type | Default |
|-------|------|---------|
| settingsVersion | integer | 1 |
| theme | `light` \| `dark` \| `system` | `system` |
| general | object (language of UI, default save folder) | UI `en` |
| hotkeys | map action -> key combo | defaults |
| analyticsEnabled | boolean | **false** |
| modelsFolder | path | app data folder |
| lastCompatMode | mode | from last check |
Validation: hotkeys must be unique per combo; conflict returns the clashing action.

## ModelRecord (catalogue entry + local state)
| Field | Type | Notes |
|-------|------|-------|
| id | text | stable key |
| name, purpose, licence | text | shown before download |
| sizeBytes | integer | shown before download |
| url, sha256 | text | |
| state | `notInstalled` \| `downloading` \| `paused` \| `verifying` \| `installed` \| `failed` | |
| bytesDone | integer | for resume |
| path | text | when installed |
Transitions: notInstalled -> downloading <-> paused; downloading -> verifying -> installed or failed (hash mismatch deletes the file and resets to notInstalled); installed -> notInstalled on delete.

## MemorySample
`appBytes`, `systemUsedBytes`, `systemTotalBytes`, `level` (`normal` \| `warning`, warning at 85% system use).

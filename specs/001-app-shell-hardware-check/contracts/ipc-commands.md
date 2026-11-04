# Contract: UI to Core commands and events

The UI calls the core only through these Tauri commands. Every command returns `Result<T, AppError>`; `AppError` has `code` (stable text), `message` (already user-readable, from the i18n catalogue key), and `details` (optional).

## Hardware
| Command | Input | Output |
|---------|-------|--------|
| `run_compat_check` | none (option `fakeMachine` only in test builds) | `MachineCapabilityReport` |
| `get_last_compat_report` | none | `MachineCapabilityReport` or null |
CLI mode: `flickfluent --check-hardware` prints the report as JSON and exits 0 (`fullLocal` or `hybrid`) or 10 (`blocked`). The Windows installer relies on this exit code.
Blocked message format (exact): `Your computer is not compatible with this app. You need {list} to run this app.` where `{list}` is the `missing[]` items joined as `8 GB RAM (found 4 GB)`.

## Projects
| Command | Input | Output |
|---------|-------|--------|
| `project_suggest_file_name` | `name` | file name safe on Windows and macOS, keeps non-Latin letters |
| `project_new` | `name`, `path` (chosen in a save dialog; the file is created at once) | `ProjectHandle` |
| `project_open` | `path` | `{ handle, recoveryAvailable }`; `handle.readOnly` is true if another process holds the lock; errors `FORMAT_TOO_NEW`, `DAMAGED` (`details.backupAvailable`) |
| `project_recover` | `path` | `ProjectHandle` restored from the recovery file, else the newest good backup; marked unsaved |
| `project_discard_recovery` | `path` | void |
| `project_save` | `handle`, optional `path` (save as) | `ProjectHandle`; error `READ_ONLY` if read-only |
| `project_set_notes` | `handle`, `notes` | `ProjectHandle` (placeholder content until later features add real data) |
| `project_close` | `handle` | void |
| `project_recent` | none | `RecentEntry[]` (`path`, `name`, `openedAt`, `available`) |
| `project_delete` | `path` | void; error `IN_USE` if open; also removes backups, recovery and lock files |
`ProjectHandle`: `id`, `name`, `path`, `readOnly`, `dirty`, `notes`.
Event `project:dirty` `{handle, dirty}`. Unsaved changes are written to a recovery file every 30 seconds by a background thread.

## Settings
| Command | Input | Output |
|---------|-------|--------|
| `settings_get` | none | `SettingsView` |
| `settings_set` | `patch` (partial `Settings`; `hotkeys` merges per action and `null` unassigns one) | `SettingsView`; errors `HOTKEY_CONFLICT` `{action, combo}` (the action that already owns the combo) and `INVALID_SETTING`; all or nothing |
| `settings_reset` | none | `SettingsView`; projects are not touched |
`SettingsView` is `{ settings, notice }`. `notice` is a message key shown once, for example `settings.corrupted` when the file had to be replaced (the old file is kept as `settings.json.corrupt`).
Event `settings:changed` carries the new `Settings`.
Hotkey combos are text like `Mod+Shift+S`; `Mod` is Ctrl on Windows and Cmd on macOS. Core normalises case and modifier order.

## Models
| Command | Input | Output |
|---------|-------|--------|
| `models_catalogue` | none | `ModelRecord[]` (`id`, `name`, `purpose`, `licence`, `sizeBytes`, `state`, `bytesDone`, `errorCode`) |
| `model_download_start` | `id` | void. Fails at once with `NO_SPACE` `{neededBytes, freeBytes}` if the disk is too full; network problems arrive later as a `failed` state |
| `model_download_pause` | `id` | void; the `.part` file is kept |
| `model_download_resume` | `id` | void; continues from the kept bytes with an HTTP Range request (restarts if the server ignores Range) |
| `model_download_cancel` | `id` | void; deletes the `.part` file |
| `model_delete` | `id` | void; error `IN_USE` while downloading |
`state` is `notInstalled`, `downloading`, `paused`, `verifying`, `installed` or `failed`. After an app restart an unfinished download reads as `paused` (the `.part` file on disk is the state).
Event `model:progress` carries the full `ModelRecord`, at most about five times a second.
Download errors: `NO_SPACE`, `OFFLINE` (progress kept), `SERVER_ERROR` `{status}`, `HASH_MISMATCH` (file deleted, never installed).
Layout on disk: `<models folder>/<id>/<file>`, `<file>.part` while downloading, and `index.json`. The models folder is `modelsFolder` in settings, else the app data folder.

## Memory
Event `memory:sample` (every 2 s) with `MemorySample`.

## Error codes (stable)
`FORMAT_TOO_NEW`, `DAMAGED`, `READ_ONLY`, `HOTKEY_CONFLICT`, `NO_SPACE`, `OFFLINE`, `HASH_MISMATCH`, `IO_ERROR`, `IN_USE`, `INVALID_SETTING`, `SERVER_ERROR`, `NOT_FOUND`, `UNKNOWN_HARDWARE_VALUE` (warning, not error).

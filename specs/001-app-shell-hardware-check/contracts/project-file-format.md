# Contract: `.flick` project file, format version 1

## Container
ZIP archive, UTF-8 entry names, extension `.flick`.

```text
manifest.json     required
project.json      required (object; empty `{}` in feature 001)
assets/...        reserved for later features
```

## manifest.json
```json
{
  "formatVersion": 1,
  "appVersion": "0.1.0",
  "id": "3f1b8f0e-0000-4000-8000-000000000000",
  "name": "សាកល្បង Project",
  "createdAt": "2026-11-04T10:00:00Z",
  "savedAt": "2026-11-04T10:05:00Z"
}
```

## Rules
1. Reader MUST reject `formatVersion` greater than it supports with `FORMAT_TOO_NEW` and MUST NOT modify the file.
2. Reader MUST ignore unknown keys and unknown `assets/` entries and keep them on save.
3. Writer MUST write to a temporary file in the same folder, flush to disk, then rename over the target.
4. Writer MUST keep the last 3 previous versions as `<name>.flick.bak.1` to `.3` before overwriting.
5. While dirty, the app writes `<name>.flick.recover` every 30 seconds and deletes it on a clean save or close.
6. Open uses a lock sidecar `<name>.flick.lock` holding the process ID; a live lock opens read-only; a stale lock is ignored.
7. Dates are UTC ISO 8601. The project name is only in `manifest.name`, never required to match the file name.
8. Migration: when the app supports a newer `formatVersion` it migrates on open into memory and writes the new version only on save, after making a backup.

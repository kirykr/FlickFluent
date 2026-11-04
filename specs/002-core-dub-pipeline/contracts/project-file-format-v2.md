# Contract: `.flick` project file, format 2

Extends format 1 from feature 001 (ZIP with `manifest.json` and `project.json`).

## Changes
- `manifest.json`: `formatVersion` becomes `2`. Everything else as in format 1.
- `project.json` gains the fields of **Project** in data-model.md: `source`, `sourceLanguage`, `detectedLanguage`, `targetLanguage`, `options`, `speakers[]`, `lines[]`, `run`.
- `lines[].audio` and intermediate audio are **not** stored in the ZIP. They live in the work folder `<project file name without extension>.flick-work/` beside the project file, with a small `work-manifest.json` that records the project id so a moved or renamed project can find or ignore its folder.
- Export results are written where the user chooses, never inside the project.

## Work folder layout
```text
<name>.flick-work/
├── work-manifest.json     # { projectId, formatVersion }
├── run-state.json         # run, sections, stage states, input hashes (atomic writes)
├── audio.wav
├── audio-full.wav
├── sections/0000/{vocals.wav, music.wav, lines.json, translated.json}
├── speech/{lineId.wav}
└── out/dub-mix.wav
```

## Migration 1 to 2
- Opening a format 1 file: the core adds empty v2 fields, writes a backup of the old file named `<name>.flick.v1.bak`, and saves as format 2 on the next save. No data from format 1 is lost.
- Opening a format 2 file in a build that only knows format 1: error `FORMAT_TOO_NEW` (already defined in 001).
- Format 3 and later must keep reading format 2.

## Rules
- `project.json` is written with the 001 atomic save. `run-state.json` is written separately and more often, with the same atomic helper, so a crash cannot damage the project file.
- Work folder deletion happens only through `dub_discard_run` or `project_delete`.
- Disk use of the work folder is shown to the user and counted in the preflight check.
- A project that is opened without its work folder is valid: transcript and settings are kept, audio stages simply rerun.

# Contract: editor commands and events

Tauri commands between the React UI and the Rust core. The UI is a view; every rule is in the core. Errors are `AppError` with a code and an i18n key, as elsewhere. All commands take the open project's handle (as `dub_*` commands do). Fields are `camelCase`.

## State

- `editor_open() -> EditorState`: `{ lines, speakers, changedIds, canUndo, canRedo, captionTemplate }`. Fills missing `speechHash` for old projects (R1).
- `editor_state() -> EditorState`: the same, without side effects.

## Edits (each is one undo step, saved at once)

- `editor_update_line({ id, startMs?, endMs?, translated?, speakerId?, speed?, volumeDb?, pitchSemitones? }) -> EditResult`
- `editor_insert_line({ afterId?, startMs, endMs, speakerId, translated }) -> EditResult`
- `editor_delete_lines({ ids }) -> EditResult`
- `editor_merge_lines({ ids }) -> EditResult`: same speaker, neighbours only, else `editor.error.cannotMerge`.
- `editor_split_line({ id, textOffset }) -> EditResult`: time divided in proportion to the text.
- `editor_duplicate_line({ id }) -> EditResult`
- `editor_bulk_text({ texts: [{ id, translated }] }) -> EditResult`: only lines whose text differs change.
- `editor_set_speaker_gender({ speakerId, gender }) -> EditResult`
- `editor_undo() -> EditResult`, `editor_redo() -> EditResult`

`EditResult` = `{ lines (only those changed), removedIds, changedIds, canUndo, canRedo }`.

## Search and replace

- `editor_search({ query, field: "original" | "translated" | "both", matchCase, wholeWord }) -> { matches: [{ lineId, field, start, end }] }`
- `editor_replace({ matches, replacement }) -> EditResult`: applies exactly the matches given, as one undo step.

## Fitting

- `editor_fit({ mode: "smart" | "endSync", ids?: string[] }) -> FitReport`: `ids` absent means all flagged lines (Smart Fit) or all lines (End Sync). Smart Fit re-runs only the post chain (R4). No engine call, no cloud characters.

## Synthesis

- `editor_resynthesize({ ids?: string[] }) -> { runId }`: `ids` absent means every changed line. Voices, models and engine come from the editor runtime (research R17). Cloud characters are checked by the shared guard and added to the project's cloud usage (R18). With the work folder missing it returns `EDIT_WORK_FOLDER_MISSING`. Needs consent for cloud lines (`CLOUD_CONSENT_REQUIRED`, see 003) and models for local lines (`MODELS_MISSING`). Progress and results arrive as events.
- `editor_stop()`: stops the running re-synthesis, import or export. Finished work is kept (FR-015).

## Subtitles

- `subtitles_export({ source: "original" | "translated" | "both", format: "srt" | "vtt", path }) -> { cueCount }`: WebVTT carries the caption template (R8); SRT is plain.
- `subtitles_import_preview({ path }) -> ImportPreview`
- `subtitles_import_apply({ previewId, choice: "replaceText" | "replaceLines" | "fillByOverlap" }) -> EditResult`: one undo step.
- `caption_templates_list() -> CaptionTemplate[]`
- `caption_template_set({ id }) -> void`

## Events (channel `editor:event`)

| Event | Payload |
|-------|---------|
| `progress` | `{ runId, kind: "resynthesize" \| "import" \| "export", done, total, etaSeconds? }` at least every 5 s while work runs (as in 002) |
| `lineDone` | `{ runId, line }` after each swap |
| `cloud` | the 003 cloud note (key switch, waiting) |
| `finished` | `{ runId, state: "completed" \| "stopped" \| "failed", error?, remainingChangedIds }` |

## Error codes (new, listed in `core/src/pipeline/errors.rs` with i18n keys)

`EDIT_END_BEFORE_START`, `EDIT_OUT_OF_RANGE`, `EDIT_CANNOT_MERGE`, `EDIT_NOTHING_TO_UNDO`, `EDIT_WORK_FOLDER_MISSING`, `DUB_EDITS_WOULD_BE_LOST` (from `dub_start`, see below), `SUBTITLE_PARSE_FAILED` (details: first problem and its line), `SUBTITLE_ENCODING_UNKNOWN`, `SUBTITLE_PREVIEW_EXPIRED`. Existing codes are reused for consent, models, disk space, cancelled and source missing.

## Rules the tests enforce

1. A rejected edit changes nothing and leaves the undo stack alone.
2. After any edit the project file on disk equals the in-memory state (SC-007).
3. `changedIds` equals the lines whose hash differs, and only those (FR-004).
4. `editor_resynthesize` sends no request and writes no file for unchanged lines (FR-005, SC-001).
5. `editor_stop` loses no swapped clip, and a second `editor_resynthesize` does exactly the rest (SC-006).
6. No command logs or returns a key; cloud rules from 003 hold.

## Change to an existing command

- `dub_start({ ..., discardEdits?: boolean })`: when the project has edited lines and the call starts a fresh run, it fails with `DUB_EDITS_WOULD_BE_LOST` and `details.editedLines` unless `discardEdits` is true. A resume is not affected: edited sections are kept (research R16). `preflight` gains `editsWillBeLost: number` so the UI can ask before calling.

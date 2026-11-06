# Research: Transcript Editor and Timeline

Decisions that shape the plan. Each has the choice, the reason and what else was weighed. Facts about the existing code were read from the repository on 2026-11-06.

## R1. What "changed" means

- **Decision**: a line is *changed* when the hash of what its speech is made from differs from the hash stored when the clip was made. The inputs are: translated text (or original text if untranslated), language, voice id and speaker id, speaker gender, engine id (`local` model id or `cloud:<service>`), and the line's speed, volume and pitch. The hash is `speechHash` on the line. There is no stored "dirty" flag.
- **Times are not in the hash** (spec FR-004). A time edit changes the slot, not the sound the voice made; the user fixes fit with Smart Fit or End Sync and the flags are recomputed. Putting the slot in the hash would re-make every line the user nudges.
- **Why**: FR-004 and acceptance 1.6 (edit and edit back is not changed) fall out for free, a crash cannot leave a wrong flag, and it reuses the way 002 decides to skip a stage (input hashes).
- **Old projects**: a line with a clip and an empty `speechHash` counts as unchanged and gets its hash filled in the first time the editor opens it. No mass regeneration, no project format bump (same approach as 003 R9: new fields have defaults).
- **Alternatives**: a boolean per line set on every edit (wrong after edit-and-revert, wrong after a crash); comparing text only (misses speaker, gender and voice settings).

## R2. Re-synthesizing only changed lines

- **Decision**: a new core function `resynthesize(project, line_ids, cancel)` runs the existing synthesize worker (local) or the cloud speak engine (003) on just those lines, writes each clip beside the old one under a new name, and swaps the line's `audio` and `speechHash` only when the new clip is complete. A cancelled run keeps every swap already made.
- **Why**: the 002 `synthesize` stage works one section at a time and skips by section hash, so it cannot do "just line 7". The worker and the cloud engine already take a list of lines, so a list of one is cheap. Swap-when-complete keeps FR-015 and SC-006.
- **Edit during a run**: the run works on a snapshot of `(id, speechHash)`. When a clip finishes, the swap happens only if the line's current expected hash still equals the snapshot's. If the user edited the line meanwhile, the clip is kept as a file but the line stays *changed*.
- **Cloud**: the consent and amount guard from 003 (`guard_cloud_amount`) is called with the characters of the changed lines only (FR-017).
- **Alternatives**: re-run the whole section stage with a per-line cache (rewrites 002's resume logic, riskier); a second worker protocol (not needed).

## R3. Rebuilding the mix without remixing the film

- **Decision**: after clips change, re-render only the sections that hold a changed line (the existing `mix` builds one section at a time) and splice them into the old `dub-mix.wav` with the ffmpeg concat demuxer using `inpoint` and `outpoint` on the same PCM file for the untouched stretches. Section bounds do not change, so the splice points are the section bounds.
- **Why**: 002's `mix` deletes its section pieces after joining them (a 3-hour film at stereo 44.1 kHz would keep about 1.9 GB of pieces). Splicing keeps one extra piece in the work folder at a time and makes SC-001 (15 s) depend on one section, not the film.
- **Built differently (2026-11-07)**: the concat demuxer with `-c copy` cuts on packet boundaries and made the result 684 frames too long. The splice is one ffmpeg call with `atrim` by sample number on the old mix and the `concat` filter, lossless for PCM, and the test compares it sample for sample with a full remix.
- **Lines that move across a section bound** (an edit of start or end): both sections are re-rendered; a line is assigned to the section that contains its start.
- **Alternatives**: keep all pieces (disk); remix everything (too slow on a long film).

## R4. Smart Fit, speed, volume and pitch

- **Decision**: per-line speech has two parts. The *raw clip* is what the engine returned (`rawMs` recorded). A *post chain* made with ffmpeg turns it into the *line clip* the mix uses: `atempo` for speed, `volume` for gain, and pitch by `asetrate` with `aresample` and a matching `atempo` so duration stays put. Smart Fit only sets the speed and re-runs the post chain; it never calls the voice engine.
- **Why**: engine-side speed (the sidecar's `speed` argument) needs a full re-synthesis and is not offered by every cloud voice. ffmpeg is already a dependency (Constitution V) and 003 already uses `atempo` for cloud clips. Changing speed, volume or pitch then costs milliseconds and no cloud characters.
- **Safe range**: speed 0.85 to 1.30 of the raw clip, volume -12 to +6 dB, pitch -4 to +4 semitones. The upper speed limit equals `MAX_SPEEDUP` (1.3) that 002 and 003 already use; the numbers live in one place (`core/src/editor/limits.rs`) and in the data model. The raw clip already includes any engine-side speed-up done in 002 (up to 1.3), so the effective speed against the unspoken original can exceed 1.3. This is accepted and the fit result reports the effective total.
- **Pitch quality**: `asetrate` shifts formants too, so it sounds natural only for small steps; hence the 4 semitone limit. A better shifter (`rubberband`) is not in the LGPL ffmpeg builds we ship (checked: filter absent here), so it is out.
- **Alternatives**: rewrite text shorter (out of scope per spec); engine speed (see above).

## R5. End Sync

- **Decision**: End Sync is pure timeline arithmetic. For a line whose fitted clip length is `d` and original end is `e`, the dub start becomes `max(previous dub end + 80 ms, e - d)`, but never later than the original start and never earlier than the previous line's dub end. The result is stored as `dubStartMs`; the mix uses `dubStartMs` when present, else `startMs`. The transcript's `startMs` and `endMs` are untouched, so subtitles keep their times.
- **Why**: FR-007 asks the dubbed line to end with the original, which only differs from "start at the original start" when the dub is shorter than the slot; for a longer dub nothing can be done except Smart Fit. Keeping `dubStartMs` apart from `startMs` stops End Sync from changing subtitles.
- **Alternatives**: move `startMs` (breaks subtitle times).

## R6. Subtitle files

- **Decision**: formats SRT and WebVTT. Parsing uses `subtp` 0.2.0 (MIT OR Apache-2.0) if it handles BOMs, CRLF, millisecond times, cue ids and styling tags in tests written first; if it fails those, a small parser is written (the formats are simple) and `subtp` is dropped. Writing is our own (`subtitles_text` in `core/src/pipeline/stages.rs` already writes SRT and moves into the new `subtitles` module). Text encoding: BOM first, then UTF-8, then `chardetng` 1.0.0 (Apache-2.0 OR MIT) to guess, decoded with `encoding_rs` 0.8.44 ((Apache-2.0 OR MIT) AND BSD-3-Clause). The encoding used is named to the user.
- **Why**: Constitution V (reuse); the encoding libraries are by the Firefox author and standard for this job.
- **Tags**: `<i>`, `<b>`, `<c.x>`, `{\an8}` and similar are dropped on import; line breaks inside a cue are kept as a space-joined text (cue text is one line of the transcript).
- **Alternatives**: `subtitler` (17 formats, much more than needed, larger dependency tree); our own everything (reinvents).

## R7. Importing with a different number of cues

- **Decision**: the import preview shows counts and a list of the first differences. The user picks one of: *replace lines* (cues become lines, speakers by time overlap with old lines, else the first speaker), *fill translated text by overlap* (each existing line takes the text of the cue that overlaps it most; lines with no cue stay as they are), or *cancel*. Nothing changes before the choice (FR-010).
- **When counts and times match**: the same preview offers *replace text* first and shows "all N cues match" so the common round trip (SC-003) is one click.

## R8. Caption templates

- **Decision**: templates are data in `config/caption-templates.json` (font family, size as a share of video height, text colour, outline colour and width, vertical position, bottom margin), three built in: *Classic* (white, black outline, bottom), *Soft box* (white on a dark box), *Top note* (yellow, top). The preview draws them with CSS over the video frame. Export: WebVTT carries the template (a `STYLE` block with `::cue` rules and `line:` and `position:` cue settings); SRT has no style in the format, so SRT export is plain text and the export dialog says so.
- **Why**: FR-011 asks the preview and export to use the style; SRT cannot carry one. Offering WebVTT as the styled format is the honest reading and is recorded in the spec assumptions. Burning styled subtitles into the video is feature 006.
- **Alternatives**: ASS export (richer, more code and a format the spec leaves out); a template editor (out of scope).

## R9. Undo and redo

- **Decision**: an `EditSession` in the core keeps the working lines and two stacks of inverse patches. Every edit is applied, patched into the project file with the existing atomic save, and pushed on the undo stack; undo applies the inverse. The stacks are in memory and end when the project closes (spec assumption). Bulk actions (replace all, bulk dialog, Smart Fit on all, subtitle import) are one entry each.
- **Why**: keeps the logic in Rust where it is tested first (Constitution VIII) and the UI stays a view. Autosave on every edit meets FR-016 and SC-007.
- **Alternatives**: undo in the React state (loses the guarantee that disk matches the view; harder to test).

## R10. Search and replace

- **Decision**: plain text search in the core over original and translated text, with case and whole-word options; no regular expressions at first. Matches come back as `(lineId, field, start, end)` and the UI applies a chosen subset in one `ReplaceMatches` edit.
- **Why**: thousands of lines scan in milliseconds, so no index is needed (SC-005). Regex adds a way to freeze the app and is not in the spec.

## R11. A table for thousands of lines

- **Decision**: virtualise the table rows with `@tanstack/react-virtual` 3.x (MIT), inside the existing shadcn `Table`. Only visible rows are in the page.
- **Why**: a 3-hour film has several thousand lines (SC-005); the editor must not freeze. Reuse over custom code (Constitution V, CLAUDE.md).

## R12. UI parts

- **Decision**: reuse the shadcn parts already in `src/components/ui` (table, dialog, input, textarea, select, slider, tabs, popover, command, checkbox, tooltip, progress, toggle-group, dropdown-menu, badge). The editor shell and sample table in `src/features/editor` (feature 001 placeholders: `TranscriptionEditor`, `AudioTimeline`, `PreviewMonitor`) become real; nothing from `src/components/ai-elements` applies, since there is no chat or model output display. New feature code lives in `src/features/editor`; shared pieces in `src/components/custom/` if any.

## R13. Memory indicator and stopping

- **Decision**: the status-bar memory badge already exists (`core/src/memory`, feature 001). The editor shows the same sample in its own header (FR-014) through the same event, no new sampler. Stopping uses the 002 `CancelToken` and progress events; the editor operations that can take long (re-synthesis, import, export) report progress the same way.

## R14. Gender per speaker

- **Decision**: `Speaker.gender` already exists from 002 (`Male`, `Female`, `Unknown`). A user correction sets it and adds `genderSource: "user"`, so a later run does not overwrite it. A change adds the speaker's lines to the changed set through the hash (R1) because gender is in the hash.

## R15. Performance budget (SC-001, SC-005)

- One line re-synthesis on the full-local machine class: local voice about 1 to 3 s (002 benchmarks: whole 2-minute clip synthesised in single-digit seconds on the benchmark Mac), ffmpeg post chain about 100 ms, one-section re-mix and splice a few seconds. Fits 15 s; measured in a task, with the benchmark machine of 002.
- Search over 6,000 lines: a linear scan, well under 100 ms; the 2 s budget is for the UI to show matches.

## R16. Editor edits and the 002 run (resume and fresh)

- **Problem**: the 002 stages read their lines from each section's `sec.json` and `translated.json` in the work folder, skip by hashes of those files and by the existence of `clips/<id>.wav`, and then put the lines they loaded into the project. An edit that only changed the project would be undone by a resume, and a hash change would re-translate and re-synthesize the section.
- **Decision**: a section the editor has changed is marked `edited` in the run record (`Section.edited`, default false). On resume, an `edited` section counts as complete: none of its stages run and its lines come from the project, not from the section files. A **fresh** run replaces everything as before, but `dub_start` first counts the edited lines (`preflight.editsWillBeLost`) and refuses with `DUB_EDITS_WOULD_BE_LOST` until the call carries `discardEdits: true`; the UI asks the user first (FR-020). The editor never rewrites `sec.json` or `translated.json`.
- **Why**: the smallest rule that cannot lose an edit. Writing edits through to the section files would change their hashes and trigger the very re-runs that would undo them.
- **Alternatives**: write-through to section files (hash churn, two sources of truth); keep edits only on `userEdited` lines inside a re-run section (needs line identity across a re-transcription, which 002 does not give).

## R17. How the editor gets voices, models and engines

- **Decision**: a core `EditorRuntime` (`core/src/editor/runtime.rs`) builds, for the open project, what a run builds: the sidecar config, ffmpeg, models folder, catalogue and machine class, and a `ModelPlan` from the project's saved options and the current settings (`plan_models_full`), so `voice_for(speaker)` and the engine id (`local:<model>` or `cloud:<service>`) are the same as a run would pick. The Tauri command layer builds the inputs with the same function `commands/dub.rs` uses for `dub_start` (moved to a shared helper, not copied).
- **Why**: the hash (R1) and re-synthesis both need a voice per speaker, and `Speaker.voice_id` is empty in saved projects (voices are chosen per run). Using the same planner keeps the editor and a run from disagreeing.
- **If the plan changes** (a different engine chosen in settings after the dub): lines whose hash input changes show as changed, which is correct: their voice would differ.

## R18. The cloud amount guard in the editor

- **Decision**: move the check out of the run's private method into `core/src/cloud/guard.rs` as a function of the consent book, the characters already used, the characters about to be sent, the section end and the video length; the run's `guard_cloud_amount` and the editor call it. The projection file (`cloud-needed.json`) stays in the work folder. The editor adds the characters it sends to `dub.cloud_usage` with the same merge the run uses (`merged_usage`, also moved to a shared function), so the 5× rule counts edits and the usage screen shows them.
- **Why**: FR-017; without it an editing session could send unlimited text to a cloud service after one agreement.

## R19. Caption fonts for every script

- **Decision**: each template's `fontFamily` is a list that ends with script fallbacks that exist on both systems: `"Noto Sans Khmer", "Khmer UI", "Khmer Sangam MN", sans-serif` (Windows ships Khmer UI, macOS ships Khmer Sangam MN). Nothing is bundled, so the installer stays small and no font licence enters the app. WebVTT carries the same list in its `STYLE` block; which font a player uses is up to the player.
- **Why**: Khmer is a target language (owner decisions in 003) and a Latin-only font shows empty boxes. A data change in the template file is enough for any further script (Constitution IV).
- **Check**: a test that every shipped template names a Khmer fallback, and a by-hand look at Khmer text in the preview (T074).

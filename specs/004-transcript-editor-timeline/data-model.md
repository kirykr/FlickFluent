# Data Model: Transcript Editor and Timeline

All new fields have defaults, so existing `project.json` files load unchanged and `SUPPORTED_FORMAT_VERSION` stays 2 (R1, R9 of 003). Names are `camelCase` in files and messages.

## TranscriptLine (existing in `core/src/project/dub.rs`, extended)

| Field | Type | Meaning |
|-------|------|---------|
| `id`, `sectionIndex`, `startMs`, `endMs`, `speakerId`, `language`, `text`, `translated`, `audio`, `audioMs`, `flags`, `status` | as in 002 | unchanged |
| `rawAudio` | string? | path (relative to the work folder) of the clip as the engine returned it |
| `rawMs` | integer? | length of the raw clip |
| `speed` | number | post-chain speed, 1.0 default, 0.85 to 1.30 |
| `volumeDb` | number | gain in dB, 0 default, -12 to +6 |
| `pitchSemitones` | number | 0 default, -4 to +4 |
| `dubStartMs` | integer? | where the mix places the clip; absent means `startMs` (End Sync sets it) |
| `speechHash` | string | hash of what the current clip was made from (R1); empty on old projects |
| `userEdited` | boolean | the user touched this line (kept when a later run would otherwise overwrite it) |

**Derived, never stored**
- `changed` = a clip is wanted and `speechHash` is empty-with-no-clip, or differs from the hash of the line's current inputs (text, language, voice, speaker, gender, engine, speed, volume, pitch). Start and end times are **not** inputs (research R1). Voice and engine come from the editor runtime (research R17). Old projects: empty hash with a clip counts as unchanged.
- `needsPost` = the raw clip is current (raw hash matches) but `speed`, `volumeDb` or `pitchSemitones` differ from what the line clip was made with: only the ffmpeg chain re-runs.

**Validation**
- `endMs` > `startMs`; refused otherwise with `editor.error.endBeforeStart`.
- `speed`, `volumeDb`, `pitchSemitones` clamped to the limits and refused outside them.
- Text may be empty: the line then has no speech and gets a flag, and is not deleted.
- Overlap with a neighbour sets the `overlap` flag; the editor never moves a line to avoid it.

**Flags** (existing `tooLong`, `tooShort`, `lowConfidence`, `overlap`): recomputed after every change to a clip or to times, with `tooLong` judged against the line's window, as in 002.

## Section record (existing `Section` in the run, extended)

| Field | Meaning |
|-------|---------|
| `edited` | `false` by default. Set by the editor when any accepted edit touches a line of the section. A resume treats an `edited` section as complete and never runs its stages (research R16). A fresh run resets it. |

## Speaker (existing, extended)

| Field | Meaning |
|-------|---------|
| `gender` | `male`, `female`, `unknown` (existing) |
| `genderSource` | `detected` or `user`; a user value is not overwritten by a later run |

## EditOp (core, not stored)

One undoable step. Variants: `UpdateLine`, `InsertLine`, `DeleteLines`, `MergeLines`, `SplitLine`, `DuplicateLine`, `ReplaceMatches`, `BulkText`, `SetSpeakerGender`, `SetLineVoice`, `ApplyFit`, `ImportSubtitles`. Each carries enough to build its inverse (the old values of the lines it touched). A bulk action is one op.

## FitReport (message, not stored)

| Field | Meaning |
|-------|---------|
| `mode` | `smart` or `endSync` |
| `lines` | per line: `id`, `speedBefore`, `speedAfter`, `dubStartBefore`, `dubStartAfter`, `fits` (bool), `reason` (`needsMoreThanLimit`, `slotTooShort`, `noRoomBefore`, none) |
| `fitted`, `stillFlagged` | counts for the summary the user sees |

## SubtitleCue and SubtitleFile (messages)

- `SubtitleCue`: `startMs`, `endMs`, `text` (one line, tags removed).
- `SubtitleFile`: `format` (`srt` or `vtt`), `encoding` (name used), `cues`.
- `ImportPreview`: `previewId`, `format`, `encoding`, `cueCount`, `lineCount`, `allMatch` (same count and times within 1 ms), `differences` (first 20), `choices` (which of *replaceText*, *replaceLines*, *fillByOverlap* apply).

## CaptionTemplate (data file `config/caption-templates.json`)

| Field | Meaning |
|-------|---------|
| `id`, `nameKey` | stable id and i18n key for the name |
| `fontFamily` | a CSS-style list of families ending with script fallbacks present on both systems, including `Noto Sans Khmer`, `Khmer UI`, `Khmer Sangam MN` (research R19) |
| `sizePercent` | text height as a share of the video height |
| `color`, `outlineColor`, `outlineWidth` | text and outline |
| `background` | none, or a colour with opacity for a box |
| `position` | `bottom` or `top`; `marginPercent` from that edge |

The chosen template id is stored in the project (`dub.captionTemplate`, default `classic`).

## Work folder additions (`<project>-work`)

- `clips/<lineId>.raw.wav`: raw engine output (003 already writes this name for cloud lines). When speed, volume and pitch are neutral, `rawAudio` and `audio` may point at the same file; no second copy is made.
- `clips/<lineId>.<hash8>.wav`: a new line clip written under a new name, swapped in when complete, the old file removed after the swap.
- `edit-session.json`: not needed; the project file is the source of truth (R9).
- `mix/splice.txt`: the concat list for the splice, deleted after use.

## State transitions of a line's speech

```
no clip --synthesize--> raw+line clip (hash H1)
H1 --edit text/speaker/gender/voice--> changed --resynthesize--> H2
H1 --edit speed/volume/pitch--> needsPost --post chain--> H3 (raw clip kept)
H2 --edit back to H1 inputs--> unchanged only if a clip for H1 still exists; otherwise changed
```

## Additions made while building (2026-11-06)

- `Section.mixStale` (default false): set when an edit changes what the mix should hold for the section (a time, a clip, a deletion, an insertion); cleared when the editor has rebuilt that section's part of `dub-mix.wav`. Lets a closed and reopened project still know which sections to rebuild.
- `LineFlag.emptyText`: the line's text was emptied; it is kept without speech.
- `speechHash` has the form `<raw>.<post>`: `raw` covers text, language, voice, speaker, gender and engine; `post` covers speed, volume and pitch. A different `raw` means changed; the same `raw` with a different `post` means `needsPost`.
- Error code `EDIT_NOT_FOUND` (a line or speaker id that is not in the project).
- Raw clips are named `clips/<lineId>.<raw8>.raw.wav` (the first eight characters of a hash of the raw part), not `<lineId>.raw.wav`, so a new raw clip never overwrites the old one before the swap. With neutral speed, volume and pitch the line clip is the raw clip (same file).
- Undo and redo restore what the user changed, not the clip: the clip fields (`audio`, `rawAudio`, their lengths and `speechHash`) stay as the last made clip, so a line whose inputs go back shows as changed instead of pointing at a deleted file.

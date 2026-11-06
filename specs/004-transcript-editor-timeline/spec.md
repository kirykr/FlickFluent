# Feature Specification: Transcript Editor and Timeline

**Feature Branch**: `004-transcript-editor-timeline`

**Created**: 2026-11-06

**Status**: Draft

**Input**: User description: "Transcript editor and timeline. The user edits the lines of a finished dub, re-synthesizes only what changed, fits the dubbed speech to the original timing, and imports and exports subtitles with caption styles."

Covers master-spec Story 5, FR-011, FR-013, FR-014, FR-029 and FR-030. Builds on feature 002 (transcript lines, flags `tooLong`, `tooShort`, `lowConfidence`, `overlap`, per-line audio clips, dub windows, stage hashes, resume). Works with local and cloud engines (feature 003).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit lines and re-synthesize only what changed (Priority: P1)

After a dub finishes, the user opens the transcript as a table: start, end, speaker, original text, translated text and status per line. They correct a mistranslation, change a speaker, insert a missing line, delete a noise line, and use search and replace across the translated text. A line they changed is marked as changed. When they press synthesize, only the changed lines get new speech, and the mixed audio is rebuilt.

**Why this priority**: Automatic output is rarely perfect. Without editing and a cheap redo, the dub cannot be used. This is the smallest slice that makes a result usable, and the rest builds on it.

**Independent Test**: Finish a dub of the 2-minute fixture. Edit one line, delete one, insert one, press synthesize. Confirm that only those lines were sent to the voice engine, every other clip file is untouched, and the new mix plays the edit.

**Acceptance Scenarios**:

1. **Given** a finished dub, **When** the user edits the translated text of one line, **Then** that line shows as changed and no other line does.
2. **Given** one changed line, **When** the user synthesizes, **Then** only that line is regenerated and the final audio is rebuilt from the new and the kept clips.
3. **Given** a line the user inserts between two others with a start, end, speaker and text, **When** the user synthesizes, **Then** it gets its own speech and is placed at its times.
4. **Given** a word that appears in many lines, **When** the user runs search and replace over translated text, **Then** the app shows every match, replaces the ones the user accepts, and marks only those lines as changed.
5. **Given** a deleted line, **When** the user synthesizes, **Then** its speech is gone from the result and the other lines are not regenerated.
6. **Given** the user edits the text of a line and changes it back to what it was, **When** they synthesize, **Then** the line is not regenerated.
7. **Given** a cloud engine is in use, **When** the user synthesizes changed lines, **Then** the consent and amount checks of feature 003 apply to the characters of the changed lines only, and the project's cloud usage grows by the characters sent.
8. **Given** a project with edited lines whose run was stopped before the end, **When** the user resumes, **Then** the edited sections are not translated or synthesized again and the edits are still there.
9. **Given** a project with edited lines, **When** the user starts a new run, **Then** the app says how many edited lines it would replace and starts only after the user confirms.

---

### User Story 2 - Fit speech to the original timing (Priority: P1)

The user sees which lines run past their time slot and which are far too short. They run Smart Fit on one line or on all flagged lines: the speed of each line is adjusted within a safe range so it fits its slot. Lines that cannot fit within the safe range stay flagged and say why. They can also set speed, volume and pitch for a single line by hand, and run End Sync so a dubbed line ends when the original line ends where there is room to start it earlier.

**Why this priority**: Speech that overruns its slot or collides with the next line is the main way a dub sounds wrong. Feature 002 only flags this; this feature fixes it.

**Independent Test**: Take a line flagged `tooLong`. Run Smart Fit. Confirm the line now fits its slot or stays flagged with a reason. Set speed on another line by hand and confirm only that line is regenerated.

**Acceptance Scenarios**:

1. **Given** dubbed speech longer than its slot, **When** the user runs Smart Fit, **Then** the speed is raised within the safe range until it fits, and the flag clears.
2. **Given** speech that would need more than the safe range, **When** Smart Fit runs, **Then** the speed stops at the limit, the line stays flagged `tooLong`, and the app says that the text must be shortened or the slot widened.
3. **Given** speech much shorter than its slot, **When** Smart Fit runs, **Then** the speed is lowered within the safe range, never below it, and a line already inside its slot is left alone.
4. **Given** a line with a free gap before it, **When** the user runs End Sync, **Then** the dubbed line starts earlier so it ends with the original line, and never overlaps the previous line.
5. **Given** a line, **When** the user sets its speed, volume or pitch, **Then** only that line is regenerated or re-mixed, and the setting is kept with the project.
6. **Given** Smart Fit on all flagged lines, **When** it finishes, **Then** the user sees how many lines now fit and how many still do not.

---

### User Story 3 - Import and export subtitles (Priority: P2)

The user exports the transcript as a subtitle file, original text, translated text, or both, in a standard subtitle format. They can import a subtitle file into a project (for example a human translation) and its text and times replace or fill the lines. A caption template gives the exported and previewed subtitles a style.

**Why this priority**: Many users want subtitles instead of, or next to, a dub, and a human-made subtitle file is the best way to correct a translation in bulk. It does not block editing or fitting.

**Independent Test**: Export the translated subtitles of a finished dub, import the file into a new project of the same video, and confirm that text and times match line for line. Apply a caption template and confirm the preview and the export use it.

**Acceptance Scenarios**:

1. **Given** a transcript, **When** the user exports subtitles, **Then** a file in SRT or WebVTT is written with one cue per line and the line's times.
2. **Given** an exported file, **When** it is imported in another project, **Then** text and times match the lines they came from.
3. **Given** an imported file whose cue count differs from the lines, **When** the user imports it, **Then** the app shows the difference and asks whether to replace the lines, fill the translated text by time overlap, or cancel; nothing changes before the user chooses.
4. **Given** a file that is not a valid subtitle file, **When** the user imports it, **Then** the app says so, names the first problem, and changes nothing.
5. **Given** a caption template, **When** the user applies it, **Then** the subtitle preview and the export use its font, size, colour, outline and position.
6. **Given** a subtitles-only run from feature 002, **When** the user opens the editor, **Then** editing, search and replace and subtitle export work without any speech.

---

### User Story 4 - Bulk tools, speakers and stopping (Priority: P3)

The user merges two lines, splits one at a chosen point, duplicates a line, or edits all lines in one dialog. They see the gender the app detected for each speaker and can correct it. A live indicator shows how much memory the app is using. Re-synthesis, subtitle import and export can each be stopped at any time with progress shown, and finished work is kept.

**Why this priority**: These save time on large transcripts and make long operations safe, but a user can finish a dub without them.

**Independent Test**: Split one line, merge two others, change a speaker's gender, start re-synthesis of many lines and stop it half way. Confirm the finished lines are kept and the rest stay marked as changed.

**Acceptance Scenarios**:

1. **Given** a line, **When** the user splits it at a point in its text, **Then** two lines appear with the text divided there and the time divided in proportion, both marked as changed.
2. **Given** two neighbouring lines of one speaker, **When** the user merges them, **Then** one line appears with both texts and the combined time, marked as changed.
3. **Given** a line, **When** the user duplicates it, **Then** a copy appears after it with the same text and a time slot that does not overlap, marked as changed.
4. **Given** the bulk dialog, **When** the user edits all lines as text and confirms, **Then** the lines update, and only lines whose text changed are marked as changed.
5. **Given** a speaker with a detected gender, **When** the user changes it, **Then** lines of that speaker are marked as changed because their voice will change.
6. **Given** re-synthesis of many lines is running, **When** the user stops it, **Then** the lines already done keep their new speech, the rest stay marked as changed, and a later synthesize continues with the rest.
7. **Given** the editor is open, **When** the app is working, **Then** a memory indicator shows the current use and updates while the work runs.

---

### Edge Cases

- The user edits a line while a re-synthesis is running: the edit is accepted, the line stays marked as changed, and it is not lost when the run ends.
- Two lines overlap after an edit of start or end times: the app shows the overlap flag and offers to fix it; it never silently moves a line.
- A line's end is set before its start: the edit is refused with a plain message.
- A line is edited to empty text: the line is kept without speech and flagged, not deleted.
- The source video is missing when the user opens the project to edit: text editing and subtitle export still work; synthesis and export of audio say which file is missing.
- Synthesis needs a model that was removed: the app names it and offers to download it before starting.
- A subtitle file has overlapping cues, cues in a different order, or styling tags: the text is kept, tags are dropped, and cues are sorted by start time.
- A subtitle file is in another text encoding: the app reads common encodings and names the one it used; if it cannot, it says so.
- A very long film has thousands of lines: the table stays responsive and search and replace over all lines finishes without freezing the app.
- The app closes with unsynthesized edits: on next open, the edits and their changed marks are still there.
- Undo: the user undoes and redoes edits made in the editor until the project is closed.
- The work folder (clips, music stems) is missing or moved: text editing, search and replace, fitting arithmetic and subtitle import and export still work; synthesis says the work folder is missing and offers to run the dub again.
- A caption in Khmer or another script with joined glyphs: the template shows it with a font that has the glyphs, in the preview and in the WebVTT style.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST show a finished dub's lines in a table with start, end, speaker, original text, translated text, flags and status, and let the user edit every field except the original text.
- **FR-002**: Users MUST be able to insert, edit and delete lines, and the app MUST refuse an end time before its start with a plain message.
- **FR-003**: The app MUST provide search and replace over original and translated text, with a preview of matches and the choice to replace one, some or all.
- **FR-004**: The app MUST mark a line as changed when its translated text, speaker, speaker gender, voice or engine, or its speed, volume or pitch differ from what its current speech was made from, and MUST clear the mark when the line is changed back. A change of start or end time does not mark a line: it is handled by the fit tools and the `tooLong`, `tooShort` and `overlap` flags, which are recomputed after every time change.
- **FR-005**: The app MUST regenerate speech only for lines marked as changed, keep the speech files of all other lines untouched, and rebuild the final audio from the new and kept speech.
- **FR-006**: The app MUST provide Smart Fit: raise or lower a line's speed within a safe range so its speech fits its time slot, and leave lines that cannot fit flagged with the reason.
- **FR-007**: The app MUST provide End Sync: start a dubbed line earlier, where there is free time, so that it ends with the original line, without overlapping the previous line.
- **FR-008**: The app MUST let the user set speed, volume and pitch per line, and keep these settings with the project.
- **FR-009**: The app MUST export subtitles from original text, translated text or both, in SRT and WebVTT, and import them from the same formats.
- **FR-010**: The app MUST show the user what an import would change and wait for their choice before any line changes.
- **FR-011**: The app MUST offer caption templates (font, size, colour, outline, position) that style the subtitle preview and the export, and MUST provide at least three built-in templates. Every built-in template MUST show text in any language the app supports, including Khmer, using fonts present on both Windows and macOS.
- **FR-012**: The app MUST provide merge, split and duplicate of lines, and editing of all lines in one dialog.
- **FR-013**: The app MUST show the detected gender of each speaker, let the user correct it, and mark that speaker's lines as changed when it is corrected.
- **FR-014**: The app MUST show a live memory-use indicator while the editor is open.
- **FR-015**: Re-synthesis, subtitle import and subtitle export MUST each be stoppable at any time, with progress shown, and finished work MUST be kept.
- **FR-016**: Edits and changed marks MUST survive closing and reopening the project, and the user MUST be able to undo and redo edits.
- **FR-017**: When a cloud engine is used for changed lines, the consent and amount checks of feature 003 MUST apply to the characters of those lines only, and the characters sent MUST be added to the project's cloud usage record.
- **FR-018**: Editing, search and replace and subtitle import and export MUST work on a subtitles-only project, with no speech models needed.
- **FR-019**: Every action in the editor MUST work on Windows and macOS and every label and message MUST be available in each language the app supports.
- **FR-020**: Once the user has changed a section's lines, a later resume of the run MUST keep that section as the user left it and MUST NOT translate or synthesize it again. Starting a new run over a project with edited lines MUST first tell the user how many edited lines it would replace and MUST proceed only after the user confirms.

### Key Entities

- **Transcript line**: One spoken line with start, end, speaker, original text, translated text, flags, status, its speech clip, and its speed, volume and pitch settings.
- **Changed mark**: The fact that a line's current speech no longer matches its text, speaker, times or voice settings. It is derived from what the speech was made from, not set by hand.
- **Fit result**: For a line after Smart Fit or End Sync: the speed used, the new start, and whether it now fits its slot or why it cannot.
- **Subtitle file**: A list of cues with start, end and text, in SRT or WebVTT, with an encoding.
- **Caption template**: A named style for subtitles: font, size, colour, outline and position.
- **Speaker**: A voice identity in the project with a detected and a corrected gender.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After editing one line of the 2-minute fixture and pressing synthesize, the result is ready in under 15 seconds on a full-local machine, and the speech files of all other lines are byte-for-byte unchanged.
- **SC-002**: Smart Fit brings at least 90% of the lines flagged `tooLong` that need no more than the safe-range speed-up inside their slot, and every other flagged line stays flagged with its reason. Measured on the lines of a real run of the French fixtures.
- **SC-003**: Exporting subtitles and importing them into a new project reproduces text and times for 100% of lines, with times equal to within 1 millisecond.
- **SC-004**: A user who has never seen the editor can fix one mistranslation and hear the change in under 2 minutes.
- **SC-005**: Search and replace over a 3-hour film (several thousand lines) shows its matches in under 2 seconds and does not freeze the window.
- **SC-006**: Stopping a re-synthesis loses no finished line, and the next synthesize does only the remaining lines, in 100% of tried stops.
- **SC-007**: Closing and reopening a project with unsynthesized edits restores every edit and every changed mark.

## Assumptions

- The editor works on a project that already has a finished run from feature 002; it does not start a first run.
- "Safe range" for speed is the same limit feature 002 uses for fitting (up to 1.3 times faster), and a lower limit for slowing down chosen at planning; the exact numbers are set in the plan and kept in one place.
- Smart Fit changes speed only. It does not rewrite or shorten text. Shortening text with a translation engine is out of scope.
- End Sync moves the start of a dubbed line only into free time before it, never into another line's time.
- Subtitle formats for this feature are SRT and WebVTT. Other formats (ASS, SSA, TTML) and a custom template editor are out of scope; the built-in templates are the choice.
- A caption template styles the preview and the exported WebVTT file. SRT has no way to carry a style, so an SRT export is plain text and the export dialog says so.
- Burning subtitles into the exported video belongs to feature 006 (export options); this feature gives the preview and the exported subtitle file the style.
- Undo and redo cover editor actions until the project is closed; after reopening, the saved state is the starting point.
- Automatic gender detection exists from feature 002; this feature shows it and lets the user correct it.
- Voice cloning and speaker profiles with control instructions belong to feature 005.
- Windows has not been run for features 002 and 003; this feature's Windows verification is carried the same way and the feature is not closed before it passes (Constitution I).

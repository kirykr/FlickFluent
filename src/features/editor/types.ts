import type { TranscriptLine } from "../dub/types";

export type SpeakerGender = "male" | "female" | "unknown";

export interface Speaker {
  id: string;
  gender: SpeakerGender;
  voiceId: string;
  confidence: number;
  genderSource?: "detected" | "user";
}

/** `editor_open` and `editor_state`. */
export interface EditorState {
  lines: TranscriptLine[];
  speakers: Speaker[];
  changedIds: string[];
  /** Raw clip current; speed, volume or pitch still to be applied. */
  needsPostIds: string[];
  canUndo: boolean;
  canRedo: boolean;
  captionTemplate: string;
}

/** What an edit returns: only the lines that changed or appeared. */
export interface EditResult {
  lines: TranscriptLine[];
  removedIds: string[];
  changedIds: string[];
  needsPostIds: string[];
  canUndo: boolean;
  canRedo: boolean;
}

export interface LineEdit {
  id: string;
  startMs?: number;
  endMs?: number;
  translated?: string;
  speakerId?: string;
  speed?: number;
  volumeDb?: number;
  pitchSemitones?: number;
}

export interface LineInsert {
  afterId?: string | null;
  startMs: number;
  endMs: number;
  speakerId: string;
  translated: string;
}

export type SearchField = "original" | "translated" | "both";

export interface SearchQuery {
  query: string;
  field: SearchField;
  matchCase: boolean;
  wholeWord: boolean;
}

/** UTF-16 offsets into the field's text, `end` exclusive. */
export interface SearchMatch {
  lineId: string;
  field: "original" | "translated";
  start: number;
  end: number;
}

/** The `editor:event` channel. */
export type EditorEvent =
  | {
      type: "progress";
      runId: string;
      kind: "resynthesize" | "import" | "export";
      done: number;
      total: number;
      etaSeconds: number | null;
    }
  | { type: "lineDone"; runId: string; line: TranscriptLine }
  | {
      type: "cloud";
      runId: string;
      kind: string;
      step: string;
      serviceId: string;
      keyLabel: string | null;
      reason: string | null;
      waitSeconds: number | null;
    }
  | {
      type: "finished";
      runId: string;
      state: "completed" | "stopped" | "failed";
      error: { code: string; messageKey: string; details?: Record<string, unknown> | null } | null;
      remainingChangedIds: string[];
    };

export type FitMode = "smart" | "endSync";
export type FitReason = "needsMoreThanLimit" | "slowestReached" | "noRoomBefore";

export interface FitLine {
  id: string;
  speedBefore: number;
  speedAfter: number;
  dubStartBefore: number | null;
  dubStartAfter: number | null;
  fits: boolean;
  reason: FitReason | null;
}

export interface FitReport {
  mode: FitMode;
  lines: FitLine[];
  fitted: number;
  stillFlagged: number;
}

export type SubtitleSource = "original" | "translated" | "both";
export type SubtitleFormat = "srt" | "vtt";
export type ImportChoice = "replaceText" | "replaceLines" | "fillByOverlap";

export interface ImportDifference {
  index: number;
  kind: "time" | "missingCue" | "extraCue";
  lineStartMs: number | null;
  cueStartMs: number | null;
}

export interface ImportPreview {
  previewId: string;
  format: SubtitleFormat;
  encoding: string;
  cueCount: number;
  lineCount: number;
  allMatch: boolean;
  differences: ImportDifference[];
  choices: ImportChoice[];
}

export interface CaptionTemplate {
  id: string;
  nameKey: string;
  fontFamily: string;
  sizePercent: number;
  color: string;
  outlineColor: string;
  outlineWidth: number;
  background: string | null;
  position: "bottom" | "top";
  marginPercent: number;
}

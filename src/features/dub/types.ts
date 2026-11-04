export type Stage =
  "extract" | "separate" | "transcribe" | "translate" | "synthesize" | "mix" | "export";
export type StageStatus = "pending" | "running" | "done" | "failed";
export type RunState = "queued" | "running" | "cancelling" | "cancelled" | "failed" | "completed";
export type LineFlag = "overlap" | "tooLong" | "tooShort" | "mixedLanguage" | "lowConfidence";
export type Quality = "fast" | "standard";
export type ExportKind = "video" | "audio";

export interface SourceInfo {
  durationMs: number;
  hasAudio: boolean;
  hasVideo: boolean;
  container: string;
  sizeBytes: number;
}

export interface MissingModel {
  id: string;
  name: string;
  sizeBytes: number;
}

export interface Blocker {
  step: string;
  reason: "machine" | "language";
  detail: string;
}

export interface Preflight {
  missingModels: MissingModel[];
  downloadBytes: number;
  diskNeededBytes: number;
  diskFreeBytes: number | null;
  machineClass: "minimum" | "mid" | "fullLocal";
  warnings: string[];
  blockers: Blocker[];
}

export interface DubOptions {
  muteOriginal: boolean;
  exportKind: ExportKind;
  exportName: string;
  quality: Quality;
}

export interface TranscriptLine {
  id: string;
  sectionIndex: number;
  startMs: number;
  endMs: number;
  speakerId: string;
  language: string;
  text: string;
  translated: string | null;
  audio: string | null;
  audioMs: number | null;
  flags: LineFlag[];
  status: string;
}

export interface RunError {
  code: string;
  stage: Stage | null;
  sectionIndex: number | null;
  messageKey: string;
  nextStepKey: string;
  details?: Record<string, unknown> | null;
}

export interface PipelineRun {
  id: string;
  state: RunState;
  error: RunError | null;
  sections: { index: number }[];
}

export interface DubData {
  source: { path: string; durationMs: number } | null;
  sourceLanguage: string;
  detectedLanguage: string | null;
  targetLanguage: string;
  options: DubOptions;
  lines: TranscriptLine[];
}

export interface DubStatus {
  run: PipelineRun | null;
  interrupted: boolean;
  running: boolean;
  dub: DubData;
}

export interface ProgressEvent {
  runId: string;
  stage: Stage;
  stageProgress: number;
  overallProgress: number;
  sectionIndex: number;
  sectionCount: number;
  etaSeconds: number | null;
}

export interface ExportResult {
  path: string;
  sizeBytes: number;
}

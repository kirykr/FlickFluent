export type Stage =
  "extract" | "separate" | "transcribe" | "translate" | "synthesize" | "mix" | "export";
export type StageStatus = "pending" | "running" | "done" | "failed";
export type RunState = "queued" | "running" | "cancelling" | "cancelled" | "failed" | "completed";
export type LineFlag =
  "overlap" | "tooLong" | "tooShort" | "mixedLanguage" | "lowConfidence" | "emptyText";
export type Quality = "fast" | "standard";
export type ExportKind = "video" | "audio" | "subtitles";

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

import type { CloudPreflight, CloudUsage, Engines } from "../cloud/types";

export interface Preflight {
  missingModels: MissingModel[];
  downloadBytes: number;
  diskNeededBytes: number;
  diskFreeBytes: number | null;
  machineClass: "minimum" | "mid" | "fullLocal";
  warnings: string[];
  blockers: Blocker[];
  /** One entry per step that will use a cloud service. Empty when everything is local. */
  cloud: CloudPreflight[];
  /** Edited lines a fresh run would replace. */
  editsWillBeLost?: number;
}

export interface FallbackChoices {
  subtitlesOnly: boolean;
  genericVoice: boolean;
  pivotViaEnglish: boolean;
  /** Non-commercial models the user turned on after reading their notice. */
  optIn: string[];
  /** Cloud service chosen for a step in this run; "local" leaves it to the settings. */
  engines?: Engines;
}

export const NO_FALLBACKS: FallbackChoices = {
  subtitlesOnly: false,
  genericVoice: false,
  pivotViaEnglish: false,
  optIn: [],
};

export interface DubOptions {
  muteOriginal: boolean;
  exportKind: ExportKind;
  exportName: string;
  quality: Quality;
  fallbacks: FallbackChoices;
}

export type QualityLevel = "good" | "fair" | "weak";

/** One language and what can be done with it, from the model catalogue. */
export interface Language {
  code: string;
  transcribe: boolean;
  translateFrom: boolean;
  translateTo: boolean;
  speak: { voices: number; permissiveVoices: number };
  /** Cloud services with a verified key that offer this language. */
  cloudServices?: string[];
  quality: {
    transcribe: QualityLevel | null;
    translate: QualityLevel | null;
    speak: QualityLevel | null;
  };
}

export type StepKind = "transcribe" | "translate" | "speak";
export type FallbackKind =
  | "subtitlesOnly"
  | "genericVoice"
  | "pivotViaEnglish"
  | "optInNonCommercial"
  | "cloudEngine"
  | "setupCloudKey";

export interface PairStep {
  step: StepKind;
  supported: boolean;
  quality: QualityLevel | null;
  reason: string | null;
}

export interface PairFallback {
  kind: FallbackKind;
  labelKey: string;
  modelIds: string[];
  noticeKeys: string[];
  /** Cloud services that would do the step (cloud kinds only). */
  serviceIds?: string[];
  /** The step a cloud fallback is for. */
  step?: StepKind | null;
}

export interface PairCheck {
  ok: boolean;
  steps: PairStep[];
  fallbacks: PairFallback[];
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
  /** Fields from the editor (feature 004). Absent on a line from before it. */
  rawAudio?: string | null;
  rawMs?: number | null;
  speed?: number;
  volumeDb?: number;
  pitchSemitones?: number;
  dubStartMs?: number | null;
  speechHash?: string;
  userEdited?: boolean;
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
  /** What each cloud step used in the last run. Empty for a local run. */
  cloudUsage?: CloudUsage[];
}

export interface DubStatus {
  run: PipelineRun | null;
  interrupted: boolean;
  running: boolean;
  dub: DubData;
  /** The run made dubbed sound. False for a run that only made subtitles. */
  hasDubAudio: boolean;
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

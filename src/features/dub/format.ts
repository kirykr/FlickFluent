import type { AppErrorShape } from "../projects/types";
import { formatBytes } from "../models/format";
import type { LineFlag, Stage } from "./types";

/** `3725000` -> `1:02:05`; under an hour `65000` -> `1:05`. */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** Whole minutes, rounded up, for "about 3 minutes left". Never says 0 while work remains. */
export function etaMinutes(seconds: number | null): number | null {
  if (seconds === null || seconds < 0) return null;
  return Math.max(1, Math.ceil(seconds / 60));
}

export function percent(fraction: number): number {
  return Math.min(100, Math.max(0, Math.round(fraction * 100)));
}

export const STAGE_KEYS: Record<Stage, string> = {
  extract: "dub.stage.extract",
  separate: "dub.stage.separate",
  transcribe: "dub.stage.transcribe",
  translate: "dub.stage.translate",
  synthesize: "dub.stage.synthesize",
  mix: "dub.stage.mix",
  export: "dub.stage.export",
};

export const FLAG_KEYS: Record<LineFlag, string> = {
  overlap: "dub.flag.overlap",
  tooLong: "dub.flag.tooLong",
  tooShort: "dub.flag.tooShort",
  mixedLanguage: "dub.flag.mixedLanguage",
  lowConfidence: "dub.flag.lowConfidence",
};

/** Message key and numbers for a dub error, as the core sent it. */
export function dubError(
  e: AppErrorShape & { details?: Record<string, unknown> | null },
  stage?: string | null,
): {
  key: string;
  params: Record<string, string>;
} {
  let key = e.messageKey && e.messageKey.startsWith("dub.") ? e.messageKey : "dub.error.generic";
  // "The {{stage}} step failed" needs a stage; without one, say that a step failed.
  if (
    key === "dub.error.stageFailed" &&
    !stage &&
    !(e.details as { stage?: string } | null)?.stage
  ) {
    key = "dub.error.stageFailedUnknown";
  }
  const d = (e.details ?? {}) as {
    neededBytes?: number;
    freeBytes?: number;
    missing?: string[];
    stage?: string;
  };
  const params: Record<string, string> = {};
  if (typeof d.neededBytes === "number") params.needed = formatBytes(d.neededBytes);
  if (typeof d.freeBytes === "number") params.free = formatBytes(d.freeBytes);
  if (Array.isArray(d.missing)) params.missing = d.missing.join(", ");
  if (stage ?? d.stage) params.stage = (stage ?? d.stage) as string;
  return { key, params };
}

/** The file name without folders or extension, as a default export name. */
export function baseName(path: string): string {
  const file = path.split(/[\\/]/).pop() ?? path;
  const dot = file.lastIndexOf(".");
  return dot > 0 ? file.slice(0, dot) : file;
}

/** The file name with its extension, without folders. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

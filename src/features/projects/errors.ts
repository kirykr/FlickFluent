import type { AppErrorShape } from "./types";

/** The core sends a plain object; anything else (a string, an Error) is wrapped. */
export function asAppError(e: unknown): AppErrorShape {
  if (e && typeof e === "object" && "code" in e) return e as AppErrorShape;
  return { code: "UNKNOWN" };
}

const KEYS: Record<string, string> = {
  FORMAT_TOO_NEW: "projects.error.formatTooNew",
  DAMAGED: "projects.error.damaged",
  READ_ONLY: "projects.error.readOnly",
  IO_ERROR: "projects.error.io",
  IN_USE: "projects.error.inUse",
};

export function errorKey(e: AppErrorShape): string {
  return KEYS[e.code] ?? "projects.error.generic";
}

/** A damaged file with a backup is not a dead end: the user can restore from the backup. */
export function canRestoreBackup(e: AppErrorShape): boolean {
  return e.code === "DAMAGED" && e.details?.backupAvailable === true;
}

import type { AppErrorShape } from "../projects/types";
import type { ModelRecord } from "./types";

/** Human size with binary units, one decimal for MB and up: 1090 -> "1.1 KB". */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = n / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(i === 0 ? 1 : 1)} ${units[i]}`;
}

export function percent(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.floor((done / total) * 100));
}

/** Total size of everything installed. */
export function installedBytes(models: ModelRecord[]): number {
  return models.filter((m) => m.state === "installed").reduce((sum, m) => sum + m.sizeBytes, 0);
}

const KEYS: Record<string, string> = {
  NO_SPACE: "models.error.noSpace",
  OFFLINE: "models.error.offline",
  HASH_MISMATCH: "models.error.hashMismatch",
  SERVER_ERROR: "models.error.server",
  IN_USE: "models.error.inUse",
};

/** The message key for a download error code, with the numbers some messages need. */
export function modelError(
  code: string,
  details?: AppErrorShape["details"] | Record<string, unknown> | null,
): { key: string; params: Record<string, string> } {
  const d = (details ?? {}) as { neededBytes?: number; freeBytes?: number };
  const params: Record<string, string> = {};
  if (code === "NO_SPACE") {
    params.needed = formatBytes(d.neededBytes ?? 0);
    params.free = formatBytes(d.freeBytes ?? 0);
  }
  return { key: KEYS[code] ?? "models.error.generic", params };
}

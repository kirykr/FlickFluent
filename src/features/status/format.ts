import { formatBytes } from "../models/format";

export interface MemorySample {
  appBytes: number;
  systemUsedBytes: number;
  systemTotalBytes: number;
  level: "normal" | "warning";
}

/** Share of the computer's memory in use, 0 to 100, or 0 when the total is unknown. */
export function systemPercent(s: MemorySample): number {
  if (s.systemTotalBytes <= 0) return 0;
  return Math.min(100, Math.round((s.systemUsedBytes / s.systemTotalBytes) * 100));
}

export function appMemory(s: MemorySample): string {
  return formatBytes(s.appBytes);
}

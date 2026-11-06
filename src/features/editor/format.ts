/** `1:05.250` for 65,250 ms. Hours are shown only when needed. */
export function formatMs(ms: number): string {
  const total = Math.max(0, Math.round(ms));
  const millis = total % 1000;
  const seconds = Math.floor(total / 1000) % 60;
  const minutes = Math.floor(total / 60000) % 60;
  const hours = Math.floor(total / 3_600_000);
  const tail = `${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${tail}` : `${minutes}:${tail}`;
}

/** Read `1:05.250`, `1:05`, `65.25` or `65` back to milliseconds. `null` when it is not a time. */
export function parseMs(text: string): number | null {
  const m = text.trim().match(/^(?:(?:(\d+):)?(\d+):)?(\d+)(?:[.,](\d{1,3}))?$/);
  if (!m) return null;
  const [, h, min, s, frac] = m;
  const millis = frac ? Number(frac.padEnd(3, "0")) : 0;
  return ((Number(h ?? 0) * 60 + Number(min ?? 0)) * 60 + Number(s)) * 1000 + millis;
}

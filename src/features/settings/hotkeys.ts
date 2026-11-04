// Turns keyboard events into the same combo text the core stores, like `Mod+Shift+S`.
// `Mod` is Ctrl on Windows and Cmd on macOS, so one default works on both.

type KeyEventLike = Pick<
  KeyboardEvent,
  "key" | "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey"
>;

const NAMED = new Set([
  "Enter",
  "Escape",
  "Tab",
  "Backspace",
  "Delete",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
]);

export function isMac(): boolean {
  return typeof navigator !== "undefined" && /mac/i.test(navigator.userAgent);
}

function keyName(e: KeyEventLike): string | null {
  // Use the physical key for letters and digits, so Shift does not turn "1" into "!".
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3);
  if (/^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
  if (e.key === " ") return "Space";
  if (NAMED.has(e.key) || /^F([1-9]|1[0-2])$/.test(e.key)) return e.key;
  return e.key.length === 1 ? e.key.toUpperCase() : null;
}

/** The combo for this event, or null for a lone modifier key or a combo with no modifier. */
export function comboFromEvent(e: KeyEventLike, mac: boolean = isMac()): string | null {
  const key = keyName(e);
  if (!key) return null;
  const mod = mac ? e.metaKey : e.ctrlKey;
  const parts: string[] = [];
  if (mod) parts.push("Mod");
  if (mac && e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  // On Windows the Windows key is not a usable modifier.
  if (!mac && e.metaKey) return null;
  return parts.length ? [...parts, key].join("+") : null;
}

/** The action whose shortcut matches this event, if any. */
export function actionForEvent(
  hotkeys: Record<string, string>,
  e: KeyEventLike,
  mac: boolean = isMac(),
): string | null {
  const combo = comboFromEvent(e, mac);
  if (!combo) return null;
  return Object.entries(hotkeys).find(([, c]) => c === combo)?.[0] ?? null;
}

const MAC_SYMBOLS: Record<string, string> = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };

/** Show a combo the way the user's keyboard names it: ⌘⇧S on a Mac, Ctrl+Shift+S on Windows. */
export function displayCombo(combo: string, mac: boolean = isMac()): string {
  const parts = combo.split("+");
  if (mac) return parts.map((p) => MAC_SYMBOLS[p] ?? p).join("");
  return parts.map((p) => (p === "Mod" ? "Ctrl" : p)).join("+");
}

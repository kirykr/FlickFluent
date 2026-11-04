import { describe, expect, it } from "vitest";
import { actionForEvent, comboFromEvent, displayCombo } from "./hotkeys";

const ev = (o: Partial<Record<string, string | boolean>>) =>
  ({
    key: "",
    code: "",
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    ...o,
  }) as Parameters<typeof comboFromEvent>[0];

describe("hotkeys", () => {
  it("treats Ctrl as Mod on Windows and Cmd as Mod on macOS", () => {
    const win = ev({ key: "s", code: "KeyS", ctrlKey: true });
    const mac = ev({ key: "s", code: "KeyS", metaKey: true });
    expect(comboFromEvent(win, false)).toBe("Mod+S");
    expect(comboFromEvent(mac, true)).toBe("Mod+S");
  });

  it("keeps the modifier order the core uses", () => {
    const e = ev({ key: "S", code: "KeyS", ctrlKey: true, altKey: true, shiftKey: true });
    expect(comboFromEvent(e, false)).toBe("Mod+Alt+Shift+S");
  });

  it("uses the physical key so Shift+1 stays Shift+1", () => {
    const e = ev({ key: "!", code: "Digit1", ctrlKey: true, shiftKey: true });
    expect(comboFromEvent(e, false)).toBe("Mod+Shift+1");
  });

  it("ignores a lone modifier, an unmodified key, and the Windows key", () => {
    expect(comboFromEvent(ev({ key: "Control", code: "ControlLeft", ctrlKey: true }), false)).toBe(
      null,
    );
    expect(comboFromEvent(ev({ key: "s", code: "KeyS" }), false)).toBe(null);
    expect(comboFromEvent(ev({ key: "s", code: "KeyS", metaKey: true }), false)).toBe(null);
  });

  it("finds the action for a pressed shortcut", () => {
    const map = { "project.save": "Mod+S", "project.open": "Mod+O" };
    const e = ev({ key: "o", code: "KeyO", ctrlKey: true });
    expect(actionForEvent(map, e, false)).toBe("project.open");
    expect(actionForEvent(map, ev({ key: "x", code: "KeyX", ctrlKey: true }), false)).toBe(null);
  });

  it("shows Mod as Ctrl or the Cmd symbol", () => {
    expect(displayCombo("Mod+Shift+S", false)).toBe("Ctrl+Shift+S");
    expect(displayCombo("Mod+S", true)).toBe("⌘S");
    expect(displayCombo("Ctrl+Shift+K", true)).toBe("⌃⇧K");
    expect(displayCombo("Mod+Alt+Shift+K", true)).toBe("⌘⌥⇧K");
    expect(displayCombo("Ctrl+Shift+K", false)).toBe("Ctrl+Shift+K");
  });
});

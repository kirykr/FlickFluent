import { describe, expect, it } from "vitest";
import { formatMs, parseMs } from "./format";

describe("editor time format", () => {
  it("formats minutes, seconds and milliseconds, with hours only when needed", () => {
    expect(formatMs(0)).toBe("0:00.000");
    expect(formatMs(65_250)).toBe("1:05.250");
    expect(formatMs(3_725_007)).toBe("1:02:05.007");
  });

  it("reads what it writes, and the shorter forms people type", () => {
    for (const ms of [0, 999, 65_250, 3_725_007]) expect(parseMs(formatMs(ms))).toBe(ms);
    expect(parseMs("65")).toBe(65_000);
    expect(parseMs("65.5")).toBe(65_500);
    expect(parseMs("1:05")).toBe(65_000);
    expect(parseMs("1:05,25")).toBe(65_250);
  });

  it("refuses text that is not a time", () => {
    for (const bad of ["", "abc", "1:2:3:4", "-5", "1.2345"]) expect(parseMs(bad)).toBeNull();
  });
});

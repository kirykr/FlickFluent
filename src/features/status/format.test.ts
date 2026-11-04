import { describe, expect, it } from "vitest";
import { appMemory, systemPercent, type MemorySample } from "./format";

const sample = (over: Partial<MemorySample> = {}): MemorySample => ({
  appBytes: 300 * 1024 * 1024,
  systemUsedBytes: 6 * 1024 ** 3,
  systemTotalBytes: 16 * 1024 ** 3,
  level: "normal",
  ...over,
});

describe("memory badge formatting", () => {
  it("shows the share of computer memory in use", () => {
    expect(systemPercent(sample())).toBe(38);
  });

  it("never divides by zero or passes 100", () => {
    expect(systemPercent(sample({ systemTotalBytes: 0 }))).toBe(0);
    expect(systemPercent(sample({ systemUsedBytes: 99 * 1024 ** 3 }))).toBe(100);
  });

  it("shows the app's own memory in readable units", () => {
    expect(appMemory(sample())).toBe("300.0 MB");
  });
});

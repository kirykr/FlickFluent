import { describe, expect, it } from "vitest";
import { baseName, dubError, etaMinutes, formatClock, percent } from "./format";
import { languageName } from "./languages";

describe("dub format helpers", () => {
  it("formats clock times", () => {
    expect(formatClock(0)).toBe("0:00");
    expect(formatClock(65_000)).toBe("1:05");
    expect(formatClock(3_725_000)).toBe("1:02:05");
    expect(formatClock(-5)).toBe("0:00");
  });

  it("rounds the ETA up and never says zero minutes", () => {
    expect(etaMinutes(null)).toBeNull();
    expect(etaMinutes(1)).toBe(1);
    expect(etaMinutes(61)).toBe(2);
    expect(etaMinutes(600)).toBe(10);
  });

  it("clamps percentages", () => {
    expect(percent(0.456)).toBe(46);
    expect(percent(2)).toBe(100);
    expect(percent(-1)).toBe(0);
  });

  it("builds error text params from core details", () => {
    const e = dubError({
      code: "NO_SPACE",
      messageKey: "dub.error.noSpace",
      details: { neededBytes: 2048, freeBytes: 1024 } as never,
    });
    expect(e.key).toBe("dub.error.noSpace");
    expect(e.params).toEqual({ needed: "2.0 KB", free: "1.0 KB" });
    expect(dubError({ code: "X", messageKey: "error.io" }).key).toBe("dub.error.generic");
    expect(dubError({ code: "X" }).key).toBe("dub.error.generic");
  });

  it("names languages from locale data and leaves auto to the UI", () => {
    expect(languageName("fr")).toBe("French");
    expect(languageName("auto")).toBe("");
  });

  it("takes the base name of a path on either platform", () => {
    expect(baseName("/a/b/My Movie.mp4")).toBe("My Movie");
    expect(baseName("C:\\Videos\\clip.final.mkv")).toBe("clip.final");
    expect(baseName("noext")).toBe("noext");
  });
});

import { fileName } from "./format";

describe("fileName", () => {
  it("keeps the extension and drops folders", () => {
    expect(fileName("/exports/holiday.mp4")).toBe("holiday.mp4");
    expect(fileName("C:\\out\\a.m4a")).toBe("a.m4a");
  });
});

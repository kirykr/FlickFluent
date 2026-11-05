import { describe, expect, it } from "vitest";
import {
  formatBytes,
  installedBytes,
  languageSummary,
  modelError,
  percent,
  unavailableReason,
} from "./format";
import type { ModelRecord } from "./types";

const model = (state: ModelRecord["state"], sizeBytes: number): ModelRecord => ({
  id: state,
  name: state,
  purpose: "",
  licence: "MIT",
  licenceClass: "permissive",
  licenceNoticeKey: null,
  sizeBytes,
  step: "translate",
  languages: [],
  machineClass: "minimum",
  unavailable: null,
  state,
  bytesDone: 0,
  errorCode: null,
});

describe("model formatting", () => {
  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1090)).toBe("1.1 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB");
  });

  it("computes progress without going over 100 or dividing by zero", () => {
    expect(percent(50, 200)).toBe(25);
    expect(percent(300, 200)).toBe(100);
    expect(percent(5, 0)).toBe(0);
  });

  it("adds up only installed models", () => {
    const list = [model("installed", 100), model("paused", 999), model("installed", 50)];
    expect(installedBytes(list)).toBe(150);
  });

  it("explains a full disk with both numbers", () => {
    const e = modelError("NO_SPACE", { neededBytes: 2 * 1024 ** 3, freeBytes: 512 * 1024 ** 2 });
    expect(e.key).toBe("models.error.noSpace");
    expect(e.params).toEqual({ needed: "2.0 GB", free: "512.0 MB" });
  });

  it("falls back to a generic message", () => {
    expect(modelError("WHAT").key).toBe("models.error.generic");
  });
});

describe("model details", () => {
  it("lists the first languages by name and counts the rest", () => {
    const names: Record<string, string> = {
      fr: "French",
      de: "German",
      km: "Khmer",
      en: "English",
    };
    const s = languageSummary(["km", "fr", "en", "de"], (c) => names[c], 3);
    expect(s.names).toEqual(["English", "French", "German"]);
    expect(s.more).toBe(1);
    expect(languageSummary([], (c) => c)).toEqual({ names: [], more: 0 });
  });

  it("says why a model cannot be used here", () => {
    const className = (c: string) => (c === "mid" ? "a mid-range computer" : c);
    expect(unavailableReason({ reason: "machine", needs: "mid" }, className)).toEqual({
      key: "models.unavailable.machine",
      params: { needs: "a mid-range computer" },
    });
    expect(unavailableReason({ reason: "platform", needs: null }, className).key).toBe(
      "models.unavailable.platform",
    );
  });
});

import { describe, expect, it } from "vitest";
import { asAppError, canRestoreBackup, errorKey } from "./errors";

describe("project errors", () => {
  it("maps each core code to its own message", () => {
    expect(errorKey({ code: "FORMAT_TOO_NEW" })).toBe("projects.error.formatTooNew");
    expect(errorKey({ code: "READ_ONLY" })).toBe("projects.error.readOnly");
    expect(errorKey({ code: "IO_ERROR" })).toBe("projects.error.io");
  });

  it("falls back to a generic message for unknown codes", () => {
    expect(errorKey({ code: "SOMETHING_NEW" })).toBe("projects.error.generic");
  });

  it("wraps values that are not core errors", () => {
    expect(asAppError("boom")).toEqual({ code: "UNKNOWN" });
    expect(asAppError(null)).toEqual({ code: "UNKNOWN" });
    expect(asAppError({ code: "DAMAGED" }).code).toBe("DAMAGED");
  });

  it("offers a backup only for damaged files that have one", () => {
    expect(canRestoreBackup({ code: "DAMAGED", details: { backupAvailable: true } })).toBe(true);
    expect(canRestoreBackup({ code: "DAMAGED", details: { backupAvailable: false } })).toBe(false);
    expect(canRestoreBackup({ code: "IO_ERROR", details: { backupAvailable: true } })).toBe(false);
  });
});

import { describe, expect, it } from "vitest";
import { itemLabelKey, rowsToShow } from "./format";
import type { CheckItem } from "./types";

const item = (key: string, status: CheckItem["status"]): CheckItem => ({
  key,
  required: "",
  detected: "",
  status,
  neededLabel: "",
  foundLabel: "",
});

describe("compat format", () => {
  it("shows only items below minimum", () => {
    const rows = rowsToShow([item("ramGb", "belowMinimum"), item("os", "unknown")]);
    expect(rows.map((r) => r.key)).toEqual(["ramGb"]);
  });

  it("maps an item to its catalogue key", () => {
    expect(itemLabelKey(item("freeDiskGb", "belowMinimum"))).toBe("compat.item.freeDiskGb");
  });
});

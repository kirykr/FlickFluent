import { describe, expect, it } from "vitest";
import { parseRows } from "./BulkEditDialog";
import type { TranscriptLine } from "../dub/types";

const line = (id: string, translated: string): TranscriptLine => ({
  id,
  sectionIndex: 0,
  startMs: 0,
  endMs: 1000,
  speakerId: "spk1",
  language: "fr",
  text: "x",
  translated,
  audio: null,
  audioMs: null,
  flags: [],
  status: "translated",
});

describe("parseRows", () => {
  const lines = [line("a", "One"), line("b", "Two")];

  it("returns only the rows whose text differs", () => {
    const r = parseRows("a | One\nb | Twenty", lines);
    expect(r.changes).toEqual([{ id: "b", translated: "Twenty" }]);
    expect(r.unreadable).toBe(0);
  });

  it("keeps a bar inside the text and counts rows it cannot read", () => {
    const r = parseRows("a | One | more\nghost | x\nno separator\n\n", lines);
    expect(r.changes).toEqual([{ id: "a", translated: "One | more" }]);
    expect(r.unreadable).toBe(2);
  });

  it("lets a row be emptied", () => {
    expect(parseRows("a |", lines).changes).toEqual([{ id: "a", translated: "" }]);
  });
});

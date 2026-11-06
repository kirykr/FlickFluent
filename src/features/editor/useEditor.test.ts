// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { TranscriptLine } from "../dub/types";
import type { EditorEvent, EditorState, EditResult } from "./types";

type Handler = (e: { payload: EditorEvent }) => void;
const listeners: Handler[] = [];
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, h: Handler) => {
    listeners.push(h);
    return () => listeners.splice(listeners.indexOf(h), 1);
  }),
}));

const ipc = vi.hoisted(() => ({
  editorOpen: vi.fn(),
  editorState: vi.fn(),
  editorUpdateLine: vi.fn(),
  editorInsertLine: vi.fn(),
  editorDeleteLines: vi.fn(),
  editorUndo: vi.fn(),
  editorRedo: vi.fn(),
  editorSearch: vi.fn(),
  editorReplace: vi.fn(),
  editorResynthesize: vi.fn(),
  editorStop: vi.fn(),
  editorFit: vi.fn(),
  captionTemplatesList: vi.fn(),
  captionTemplateSet: vi.fn(),
  subtitlesImportApply: vi.fn(),
}));
vi.mock("../../app/ipc", () => ipc);

import { useEditor } from "./useEditor";

const line = (id: string, startMs: number, translated = "Hello"): TranscriptLine => ({
  id,
  sectionIndex: 0,
  startMs,
  endMs: startMs + 1000,
  speakerId: "spk1",
  language: "fr",
  text: "Bonjour",
  translated,
  audio: `clips/${id}.wav`,
  audioMs: 900,
  flags: [],
  status: "synthesized",
});

const state = (over: Partial<EditorState> = {}): EditorState => ({
  lines: [line("a", 0), line("b", 2000), line("c", 4000)],
  speakers: [{ id: "spk1", gender: "male", voiceId: "", confidence: 1 }],
  changedIds: [],
  needsPostIds: [],
  canUndo: false,
  canRedo: false,
  captionTemplate: "classic",
  ...over,
});

const edit = (over: Partial<EditResult> = {}): EditResult => ({
  lines: [],
  removedIds: [],
  changedIds: [],
  needsPostIds: [],
  canUndo: false,
  canRedo: false,
  ...over,
});

const send = (e: EditorEvent) => act(() => listeners.forEach((h) => h({ payload: e })));

async function open() {
  const hook = renderHook(() => useEditor("p1"));
  await waitFor(() => expect(hook.result.current.view.ready).toBe(true));
  return hook;
}

beforeEach(() => {
  listeners.length = 0;
  for (const fn of Object.values(ipc)) fn.mockReset();
  ipc.editorOpen.mockResolvedValue(state());
  ipc.editorState.mockResolvedValue(state());
  ipc.captionTemplatesList.mockResolvedValue([]);
});
afterEach(cleanup);

describe("useEditor", () => {
  it("opens the session and holds the lines", async () => {
    const { result } = await open();
    expect(result.current.view.lines.map((l) => l.id)).toEqual(["a", "b", "c"]);
    expect(ipc.editorOpen).toHaveBeenCalledWith("p1");
  });

  it("says so when the editor cannot open", async () => {
    ipc.editorOpen.mockRejectedValue({ code: "IN_USE" });
    const { result } = renderHook(() => useEditor("p1"));
    await waitFor(() => expect(result.current.view.failed).toBe(true));
    expect(result.current.view.lines).toEqual([]);
  });

  it("marks an edited line changed, and undo and redo follow the core", async () => {
    const { result } = await open();
    ipc.editorUpdateLine.mockResolvedValue(
      edit({ lines: [line("b", 2000, "Hi there")], changedIds: ["b"], canUndo: true }),
    );
    await act(() => result.current.update({ id: "b", translated: "Hi there" }));
    expect(result.current.view.lines[1].translated).toBe("Hi there");
    expect(result.current.view.changedIds).toEqual(["b"]);
    expect(result.current.view.canUndo).toBe(true);

    ipc.editorUndo.mockResolvedValue(
      edit({ lines: [line("b", 2000)], changedIds: [], canUndo: false, canRedo: true }),
    );
    await act(() => result.current.undo());
    expect(result.current.view.lines[1].translated).toBe("Hello");
    expect(result.current.view.changedIds).toEqual([]);
    expect(result.current.view.canRedo).toBe(true);

    ipc.editorRedo.mockResolvedValue(
      edit({ lines: [line("b", 2000, "Hi there")], changedIds: ["b"], canUndo: true }),
    );
    await act(() => result.current.redo());
    expect(result.current.view.changedIds).toEqual(["b"]);
  });

  it("puts an inserted line in time order and drops a deleted one", async () => {
    const { result } = await open();
    ipc.editorInsertLine.mockResolvedValue(
      edit({ lines: [line("new", 1000, "Between")], changedIds: ["new"], canUndo: true }),
    );
    await act(() =>
      result.current.insert({
        startMs: 1000,
        endMs: 1800,
        speakerId: "spk1",
        translated: "Between",
      }),
    );
    expect(result.current.view.lines.map((l) => l.id)).toEqual(["a", "new", "b", "c"]);

    ipc.editorDeleteLines.mockResolvedValue(
      edit({ removedIds: ["a", "c"], changedIds: ["new"], canUndo: true }),
    );
    await act(() => result.current.remove(["a", "c"]));
    expect(result.current.view.lines.map((l) => l.id)).toEqual(["new", "b"]);
  });

  it("keeps the view as it was when the core refuses an edit, and shows why", async () => {
    const { result } = await open();
    ipc.editorUpdateLine.mockRejectedValue({
      code: "EDIT_END_BEFORE_START",
      messageKey: "editor.error.endBeforeStart",
    });
    await act(() => result.current.update({ id: "a", endMs: 10 }));
    expect(result.current.view.error?.code).toBe("EDIT_END_BEFORE_START");
    expect(result.current.view.lines[0].endMs).toBe(1000);
  });

  it("follows a run: progress, lines made one by one, and a stop that leaves the rest marked", async () => {
    ipc.editorOpen.mockResolvedValue(state({ changedIds: ["a", "b", "c"] }));
    const { result } = await open();
    ipc.editorResynthesize.mockResolvedValue("run1");
    await act(async () => {
      await result.current.resynthesize();
    });
    expect(result.current.view.run?.total).toBe(3);

    await send({
      type: "progress",
      runId: "run1",
      kind: "resynthesize",
      done: 1,
      total: 3,
      etaSeconds: 4,
    });
    expect(result.current.view.run?.done).toBe(1);
    await send({ type: "lineDone", runId: "run1", line: line("a", 0, "New") });
    expect(result.current.view.changedIds).toEqual(["b", "c"]);
    expect(result.current.view.lines[0].translated).toBe("New");

    ipc.editorState.mockResolvedValue(state({ changedIds: ["b", "c"] }));
    await send({
      type: "finished",
      runId: "run1",
      state: "stopped",
      error: null,
      remainingChangedIds: ["b", "c"],
    });
    await waitFor(() => expect(ipc.editorState).toHaveBeenCalled());
    expect(result.current.view.run).toBeNull();
    expect(result.current.view.left).toBe(2);
    expect(result.current.view.changedIds).toEqual(["b", "c"]);
  });

  it("keeps an edit made while the run goes on", async () => {
    ipc.editorOpen.mockResolvedValue(state({ changedIds: ["a", "b"] }));
    const { result } = await open();
    ipc.editorResynthesize.mockResolvedValue("run1");
    await act(async () => {
      await result.current.resynthesize();
    });
    ipc.editorUpdateLine.mockResolvedValue(
      edit({ lines: [line("c", 4000, "Late edit")], changedIds: ["a", "b", "c"], canUndo: true }),
    );
    await act(() => result.current.update({ id: "c", translated: "Late edit" }));
    await send({ type: "lineDone", runId: "run1", line: line("a", 0, "New") });
    expect(result.current.view.lines[2].translated).toBe("Late edit");
    expect(result.current.view.changedIds).toEqual(["b", "c"]);
  });

  it("ignores events of a run it does not follow", async () => {
    ipc.editorOpen.mockResolvedValue(state({ changedIds: ["a"] }));
    const { result } = await open();
    ipc.editorResynthesize.mockResolvedValue("run1");
    await act(async () => {
      await result.current.resynthesize();
    });
    await send({ type: "lineDone", runId: "other", line: line("a", 0, "Stranger") });
    expect(result.current.view.lines[0].translated).toBe("Hello");
  });
});

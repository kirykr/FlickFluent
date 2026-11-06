import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  editorDeleteLines,
  editorFit,
  editorInsertLine,
  editorOpen,
  editorRedo,
  editorReplace,
  editorResynthesize,
  editorSearch,
  editorState,
  editorStop,
  editorUndo,
  editorUpdateLine,
  captionTemplatesList,
  captionTemplateSet,
  subtitlesImportApply,
  editorMergeLines,
  editorSplitLine,
  editorDuplicateLine,
  editorBulkText,
  editorSetSpeakerGender,
} from "../../app/ipc";
import { asAppError } from "../projects/errors";
import type { AppErrorShape } from "../projects/types";
import type { TranscriptLine } from "../dub/types";
import type {
  EditorEvent,
  EditorState,
  FitMode,
  FitReport,
  EditResult,
  LineEdit,
  LineInsert,
  SearchMatch,
  SearchQuery,
  Speaker,
  CaptionTemplate,
  ImportChoice,
  SpeakerGender,
} from "./types";

export interface RunProgress {
  runId: string;
  done: number;
  total: number;
  etaSeconds: number | null;
}

export interface EditorView {
  /** The editor opened for this project. False while opening or when it could not. */
  ready: boolean;
  failed: boolean;
  lines: TranscriptLine[];
  speakers: Speaker[];
  /** Lines the voice engine has to make again. */
  changedIds: string[];
  /** Lines that only need speed, volume or pitch applied. */
  needsPostIds: string[];
  canUndo: boolean;
  canRedo: boolean;
  captionTemplate: string;
  /** Re-synthesis in progress. */
  run: RunProgress | null;
  /** After a stop or a failed run: how many lines are still to make. */
  left: number | null;
  /** The last Smart Fit or End Sync, until the next edit. */
  fitReport: FitReport | null;
  fitting: boolean;
  templates: CaptionTemplate[];
  error: AppErrorShape | null;
}

const EMPTY: EditorView = {
  ready: false,
  failed: false,
  lines: [],
  speakers: [],
  changedIds: [],
  needsPostIds: [],
  canUndo: false,
  canRedo: false,
  captionTemplate: "classic",
  run: null,
  left: null,
  fitReport: null,
  fitting: false,
  templates: [],
  error: null,
};

/** Keep the table in time order; equal starts keep the order they had. */
function byStart(lines: TranscriptLine[]): TranscriptLine[] {
  return lines
    .map((l, i) => [l, i] as const)
    .sort((a, b) => a[0].startMs - b[0].startMs || a[1] - b[1])
    .map(([l]) => l);
}

/** Apply the lines an edit returned, and the sets it reports, to the view. */
export function applyResult(v: EditorView, r: EditResult): EditorView {
  const removed = new Set(r.removedIds);
  const changed = new Map(r.lines.map((l) => [l.id, l]));
  const kept = v.lines.filter((l) => !removed.has(l.id)).map((l) => changed.get(l.id) ?? l);
  const have = new Set(kept.map((l) => l.id));
  const added = r.lines.filter((l) => !have.has(l.id));
  return {
    ...v,
    lines: added.length ? byStart([...kept, ...added]) : kept,
    changedIds: r.changedIds,
    needsPostIds: r.needsPostIds ?? [],
    canUndo: r.canUndo,
    canRedo: r.canRedo,
    left: null,
    fitReport: null,
    error: null,
  };
}

function fromState(v: EditorView, s: EditorState): EditorView {
  return {
    ...v,
    ready: true,
    failed: false,
    lines: s.lines,
    speakers: s.speakers,
    changedIds: s.changedIds,
    needsPostIds: s.needsPostIds ?? [],
    canUndo: s.canUndo,
    canRedo: s.canRedo,
    captionTemplate: s.captionTemplate,
  };
}

/** One project's editing session: the lines, the marks, and what the core says about them. */
export function useEditor(handle: string) {
  const [view, setView] = useState<EditorView>(EMPTY);
  const runRef = useRef<string | null>(null);

  const guard = useCallback(async <T>(fn: () => Promise<T>): Promise<T | undefined> => {
    try {
      return await fn();
    } catch (e) {
      setView((v) => ({ ...v, error: asAppError(e) }));
      return undefined;
    }
  }, []);

  const apply = useCallback(
    async (fn: () => Promise<EditResult>) => {
      const r = await guard(fn);
      if (r) setView((v) => applyResult(v, r));
      return r;
    },
    [guard],
  );

  const reload = useCallback(async () => {
    try {
      const s = await editorState(handle);
      setView((v) => fromState(v, s));
    } catch (e) {
      setView((v) => ({ ...v, error: asAppError(e) }));
    }
  }, [handle]);

  useEffect(() => {
    let alive = true;
    setView(EMPTY);
    captionTemplatesList()
      .then((templates) => alive && setView((v) => ({ ...v, templates })))
      .catch(() => {});
    editorOpen(handle)
      .then((s) => alive && setView((v) => fromState(v, s)))
      .catch(() => alive && setView({ ...EMPTY, failed: true }));
    return () => {
      alive = false;
    };
  }, [handle]);

  useEffect(() => {
    const unlisten: UnlistenFn[] = [];
    let alive = true;
    listen<EditorEvent>("editor:event", (e) => {
      const p = e.payload;
      if (runRef.current !== null && "runId" in p && p.runId !== runRef.current) return;
      if (p.type === "progress") {
        setView((v) => ({
          ...v,
          run: { runId: p.runId, done: p.done, total: p.total, etaSeconds: p.etaSeconds },
        }));
      } else if (p.type === "lineDone") {
        setView((v) => ({
          ...v,
          lines: v.lines.map((l) => (l.id === p.line.id ? p.line : l)),
          changedIds: v.changedIds.filter((id) => id !== p.line.id),
          needsPostIds: v.needsPostIds.filter((id) => id !== p.line.id),
        }));
      } else if (p.type === "finished") {
        runRef.current = null;
        setView((v) => ({
          ...v,
          run: null,
          left: p.state === "completed" ? null : p.remainingChangedIds.length,
          error: p.error ? (p.error as AppErrorShape) : null,
        }));
        void reload();
      }
    }).then((off) => {
      if (alive) unlisten.push(off);
      else off();
    });
    return () => {
      alive = false;
      unlisten.forEach((off) => off());
    };
  }, [reload]);

  return {
    view,
    reload,
    update: (edit: LineEdit) => apply(() => editorUpdateLine(handle, edit)),
    insert: (edit: LineInsert) => apply(() => editorInsertLine(handle, edit)),
    remove: (ids: string[]) => apply(() => editorDeleteLines(handle, ids)),
    undo: () => apply(() => editorUndo(handle)),
    redo: () => apply(() => editorRedo(handle)),
    search: (q: SearchQuery): Promise<SearchMatch[]> =>
      guard(() => editorSearch(handle, q)).then((r) => r?.matches ?? []),
    replace: (matches: SearchMatch[], replacement: string) =>
      apply(() => editorReplace(handle, matches, replacement)),
    resynthesize: async (ids?: string[]) => {
      const runId = await guard(() => editorResynthesize(handle, ids));
      if (runId) {
        runRef.current = runId;
        setView((v) => ({
          ...v,
          left: null,
          error: null,
          run: {
            runId,
            done: 0,
            total: ids?.length ?? v.changedIds.length + v.needsPostIds.length,
            etaSeconds: null,
          },
        }));
      }
      return runId;
    },
    fit: async (mode: FitMode, ids?: string[]) => {
      setView((v) => ({ ...v, fitting: true, fitReport: null, error: null }));
      const report = await guard(() => editorFit(handle, mode, ids));
      setView((v) => ({ ...v, fitting: false, fitReport: report ?? null }));
      if (report) await reload();
      return report;
    },
    merge: (ids: string[]) => apply(() => editorMergeLines(handle, ids)),
    split: (id: string, textOffset: number) => apply(() => editorSplitLine(handle, id, textOffset)),
    duplicate: (id: string) => apply(() => editorDuplicateLine(handle, id)),
    bulk: (texts: { id: string; translated: string }[]) =>
      apply(() => editorBulkText(handle, texts)),
    setGender: async (speakerId: string, gender: SpeakerGender) => {
      const r = await apply(() => editorSetSpeakerGender(handle, speakerId, gender));
      if (r) await reload();
      return r;
    },
    importApply: (previewId: string, choice: ImportChoice) =>
      apply(() => subtitlesImportApply(handle, previewId, choice)),
    setTemplate: async (id: string) => {
      const ok = await guard(async () => {
        await captionTemplateSet(handle, id);
        return true;
      });
      if (ok) setView((v) => ({ ...v, captionTemplate: id }));
    },
    stop: () => guard(() => editorStop(handle)),
    clearError: () => setView((v) => ({ ...v, error: null })),
  };
}

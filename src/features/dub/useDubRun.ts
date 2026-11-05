import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { dubStatus } from "../../app/ipc";
import type {
  DubStatus,
  ProgressEvent,
  RunError,
  RunState,
  Stage,
  StageStatus,
  TranscriptLine,
} from "./types";

export interface RunView {
  status: DubStatus | null;
  runId: string | null;
  stage: Stage | null;
  stageStatus: Partial<Record<Stage, StageStatus>>;
  progress: ProgressEvent | null;
  /** Lines as they arrive during a run; the saved lines once it has finished. */
  lines: TranscriptLine[];
  error: RunError | null;
  state: RunState | null;
  /** Spoken language found by auto-detect, once known. */
  detected: string | null;
}

const EMPTY: RunView = {
  status: null,
  runId: null,
  stage: null,
  stageStatus: {},
  progress: null,
  lines: [],
  error: null,
  state: null,
  detected: null,
};

/** Follows one project's dub: loads its saved state, then applies the events the core sends. */
export function useDubRun(handle: string) {
  const [view, setView] = useState<RunView>(EMPTY);
  const runRef = useRef<string | null>(null);

  const refresh = useCallback(async () => {
    const status = await dubStatus(handle);
    setView((v) => ({
      ...v,
      status,
      // Lines that arrived as events stay if the saved project has none yet.
      lines: status.running || status.dub.lines.length === 0 ? v.lines : status.dub.lines,
      state: status.run?.state ?? null,
      detected: status.dub.detectedLanguage ?? v.detected,
      error: status.run?.error ?? null,
    }));
  }, [handle]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const follow = useCallback((runId: string) => {
    runRef.current = runId;
    setView((v) => ({
      ...v,
      runId,
      lines: [],
      error: null,
      progress: null,
      stageStatus: {},
      state: "running",
    }));
  }, []);

  useEffect(() => {
    const unlisten: UnlistenFn[] = [];
    let alive = true;
    const mine = (p: { runId: string }) => runRef.current === null || p.runId === runRef.current;
    const add = async <T>(name: string, fn: (p: T) => void) => {
      const off = await listen<T>(name, (e) => fn(e.payload));
      if (alive) unlisten.push(off);
      else off();
    };
    void add<ProgressEvent>("dub:progress", (p) => {
      if (!mine(p)) return;
      setView((v) => ({ ...v, progress: p, stage: p.stage }));
    });
    void add<{ runId: string; stage: Stage; status: StageStatus }>("dub:stage", (p) => {
      if (!mine(p)) return;
      setView((v) => ({
        ...v,
        stage: p.status === "running" ? p.stage : v.stage,
        stageStatus: { ...v.stageStatus, [p.stage]: p.status },
      }));
    });
    void add<{ runId: string; line: TranscriptLine }>("dub:line", (p) => {
      if (!mine(p)) return;
      setView((v) => ({ ...v, lines: [...v.lines.filter((l) => l.id !== p.line.id), p.line] }));
    });
    void add<{
      runId: string;
      code: string;
      stage: Stage | null;
      sectionIndex?: number | null;
      message: string;
      nextStep: string;
      details?: Record<string, unknown> | null;
    }>("dub:error", (p) => {
      if (!mine(p)) return;
      setView((v) => ({
        ...v,
        error: {
          code: p.code,
          stage: p.stage ?? null,
          sectionIndex: p.sectionIndex ?? null,
          messageKey: p.message,
          nextStepKey: p.nextStep ?? "dub.next.retry",
          details: p.details ?? null,
        },
      }));
    });
    void add<{ runId: string; language: string }>("dub:detected", (p) => {
      if (!mine(p)) return;
      setView((v) => ({ ...v, detected: p.language }));
    });
    void add<{ runId: string; state: RunState }>("dub:done", (p) => {
      if (!mine(p)) return;
      runRef.current = null;
      setView((v) => ({ ...v, runId: null, state: p.state }));
      void refresh();
    });
    return () => {
      alive = false;
      unlisten.forEach((off) => off());
    };
  }, [refresh]);

  return { view, follow, refresh };
}

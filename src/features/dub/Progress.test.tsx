// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../../app/ipc", () => ({
  dubPreflight: vi.fn(async () => ({
    missingModels: [{ id: "whisper-small", name: "Whisper small", sizeBytes: 500 * 1024 * 1024 }],
    downloadBytes: 500 * 1024 * 1024,
  })),
  modelDownloadStart: vi.fn(async () => {}),
}));

import { RunProgress } from "./Progress";
import { nextAction } from "./format";
import type { RunView } from "./useDubRun";
import type { RunError, TranscriptLine } from "./types";

const line = (over: Partial<TranscriptLine> = {}): TranscriptLine => ({
  id: "s0000-l0000",
  sectionIndex: 0,
  startMs: 65_000,
  endMs: 67_000,
  speakerId: "spk1",
  language: "fr",
  text: "Bonjour tout le monde",
  translated: "Hello everyone",
  audio: null,
  audioMs: null,
  flags: [],
  status: "translated",
  ...over,
});

const view = (over: Partial<RunView> = {}): RunView => ({
  status: null,
  runId: "r1",
  stage: "transcribe",
  stageStatus: { extract: "done", separate: "done", transcribe: "running" },
  progress: {
    runId: "r1",
    stage: "transcribe",
    stageProgress: 0.5,
    overallProgress: 0.42,
    sectionIndex: 1,
    sectionCount: 4,
    etaSeconds: 125,
  },
  lines: [],
  error: null,
  state: "running",
  detected: null,
  ...over,
});

const failure = (over: Partial<RunError> = {}): RunError => ({
  code: "STAGE_FAILED",
  stage: "transcribe",
  sectionIndex: 1,
  messageKey: "dub.error.stageFailed",
  nextStepKey: "dub.next.retry",
  ...over,
});

const handlers = () => ({
  handle: "h1",
  onCancel: vi.fn(),
  onResume: vi.fn(),
  onRestart: vi.fn(),
  onConfirmSame: vi.fn(),
});

afterEach(cleanup);

describe("run progress", () => {
  it("shows the stage list, the overall percentage, the section and the ETA", () => {
    render(<RunProgress view={view()} {...handlers()} />);
    expect(screen.getByText("Writing down what is said")).toBeTruthy();
    expect(screen.getByLabelText("Overall progress").getAttribute("aria-valuenow")).toBe("42");
    expect(screen.getByText(/Part 2 of 4/)).toBeTruthy();
    expect(screen.getByText(/about 3 min left/)).toBeTruthy();
    const running = document.querySelector('[data-stage="transcribe"]');
    expect(running?.getAttribute("data-status")).toBe("running");
    expect(document.querySelector('[data-stage="extract"]')?.getAttribute("data-status")).toBe(
      "done",
    );
  });

  it("shows the running stage's own percentage", () => {
    render(<RunProgress view={view()} {...handlers()} />);
    expect(screen.getByTestId("stage-percent").textContent).toContain("50%");
  });

  it("lists transcript lines as they arrive, newest last", () => {
    const lines = [
      line({ id: "a", startMs: 1000, text: "Un", translated: "One" }),
      line({ id: "b", startMs: 2000, text: "Deux", translated: null }),
    ];
    render(<RunProgress view={view({ lines })} {...handlers()} />);
    const rows = screen.getAllByTestId("live-line");
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("One");
    expect(rows[1].textContent).toContain("Deux");
  });

  it("stops the run from the cancel button", async () => {
    const h = handlers();
    render(<RunProgress view={view()} {...h} />);
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(h.onCancel).toHaveBeenCalled();
  });

  it("shows the error message, the next step and a button for it", async () => {
    const h = handlers();
    render(<RunProgress view={view({ state: "failed", runId: null, error: failure() })} {...h} />);
    const alert = screen.getByTestId("dub-run-error");
    expect(alert.textContent).toContain("Writing down what is said");
    expect(alert.textContent).toContain("Try again.");
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(h.onResume).toHaveBeenCalled();
  });

  it("gives the exact disk numbers and the shortfall", () => {
    const error = failure({
      code: "NO_SPACE",
      stage: "transcribe",
      messageKey: "dub.error.noSpace",
      nextStepKey: "dub.next.freeDiskSpace",
      details: { neededBytes: 3 * 1024 ** 3, freeBytes: 1024 ** 3, shortfallBytes: 2 * 1024 ** 3 },
    });
    render(<RunProgress view={view({ state: "failed", runId: null, error })} {...handlers()} />);
    const text = screen.getByTestId("dub-run-error").textContent ?? "";
    expect(text).toContain("3.0 GB");
    expect(text).toContain("1.0 GB");
    expect(text).toContain("2.0 GB");
    expect(text).toContain("Free some disk space");
  });

  it("offers a new file when the source is gone", async () => {
    const h = handlers();
    const error = failure({
      messageKey: "dub.error.sourceMissing",
      nextStepKey: "dub.next.chooseAnotherFile",
    });
    render(<RunProgress view={view({ state: "failed", runId: null, error })} {...h} />);
    await userEvent.click(screen.getAllByRole("button", { name: "Choose another file" })[0]);
    expect(h.onRestart).toHaveBeenCalled();
  });

  it("asks before downloading models a run is missing, with their size", async () => {
    const error = failure({
      code: "MODELS_MISSING",
      messageKey: "dub.error.modelsMissing",
      nextStepKey: "dub.next.downloadModels",
      details: { missing: ["whisper-small"] },
    });
    render(<RunProgress view={view({ state: "failed", runId: null, error })} {...handlers()} />);
    expect(await screen.findByRole("button", { name: "Download 500.0 MB" })).toBeTruthy();
    expect(screen.getByText("Whisper small")).toBeTruthy();
  });

  it("keeps cancelled runs gentle and resumable", () => {
    const error = failure({
      code: "CANCELLED",
      stage: null,
      messageKey: "dub.error.cancelled",
      nextStepKey: "dub.next.resume",
    });
    render(<RunProgress view={view({ state: "cancelled", runId: null, error })} {...handlers()} />);
    expect(screen.getByText("Stopped")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Resume" })).toBeTruthy();
  });
});

describe("next-step actions", () => {
  it("maps each next-step key to one action", () => {
    expect(nextAction("dub.next.retry")).toBe("resume");
    expect(nextAction("dub.next.freeDiskSpace")).toBe("resume");
    expect(nextAction("dub.next.resume")).toBe("resume");
    expect(nextAction("dub.next.chooseAnotherFile")).toBe("chooseAnother");
    expect(nextAction("dub.next.restartRun")).toBe("restart");
    expect(nextAction("dub.next.downloadModels")).toBe("download");
    expect(nextAction("dub.next.reinstallRuntime")).toBe("download");
    expect(nextAction("dub.next.seeRequirements")).toBe("none");
  });
});

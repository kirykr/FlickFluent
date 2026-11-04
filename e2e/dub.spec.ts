import { expect, test, type Page } from "@playwright/test";

// The Tauri core is replaced by a fake that "runs" a dub in the page: it sends the same
// `dub:*` events as the real core, keeps its state in localStorage (so a reload stands in for
// killing and relaunching the app), and refuses to overwrite files. The real pipeline,
// resume-after-kill and export are tested in core/tests/pipeline_resume.rs and
// core/tests/stage_mix_export.rs.

declare global {
  interface Window {
    __fakeDub: { nextOpen: string | null; revealed: string[]; slow: boolean };
  }
}

function installFakeCore() {
  const fake = { nextOpen: null as string | null, revealed: [] as string[], slow: false };
  (window as unknown as { __fakeDub: typeof fake }).__fakeDub = fake;
  type Line = {
    id: string;
    startMs: number;
    endMs: number;
    speakerId: string;
    language: string;
    text: string;
    translated: string;
    flags: string[];
  };
  type Disk = {
    configured: boolean;
    whisper: boolean;
    run: "none" | "running" | "completed" | "cancelled";
    step: number;
    exported: string[];
  };
  const load = (): Disk =>
    JSON.parse(
      localStorage.getItem("fakeDub") ??
        '{"configured":false,"whisper":false,"run":"none","step":0,"exported":[]}',
    );
  const store = (d: Disk) => localStorage.setItem("fakeDub", JSON.stringify(d));

  const LINES: Line[] = [
    {
      id: "s0000-l0000",
      startMs: 500,
      endMs: 2500,
      speakerId: "spk1",
      language: "fr",
      text: "Bonjour tout le monde.",
      translated: "Hello everyone.",
      flags: [],
    },
    {
      id: "s0000-l0001",
      startMs: 3000,
      endMs: 5000,
      speakerId: "spk2",
      language: "fr",
      text: "Merci beaucoup.",
      translated: "Thank you very much, my friends.",
      flags: ["tooLong"],
    },
  ];

  const handlers: Record<string, number[]> = {};
  let cb = 0;
  const emit = (event: string, payload: unknown) =>
    (handlers[event] ?? []).forEach((h) =>
      (window as unknown as Record<string, (e: unknown) => void>)[`_${h}`]({
        event,
        id: h,
        payload,
      }),
    );

  let timer: number | undefined;
  let running = false;
  const STEPS = 6; // extract, separate, transcribe, translate, synthesize, mix
  const stages = ["extract", "separate", "transcribe", "translate", "synthesize", "mix"];
  const tick = () => {
    const d = load();
    if (d.run !== "running") return;
    const stage = stages[d.step];
    if (d.step > 0) emit("dub:stage", { runId: "r1", stage: stages[d.step - 1], status: "done" });
    emit("dub:stage", { runId: "r1", stage, status: "running" });
    emit("dub:progress", {
      runId: "r1",
      stage,
      stageProgress: 0.5,
      overallProgress: (d.step + 0.5) / STEPS,
      sectionIndex: 0,
      sectionCount: 1,
      etaSeconds: (STEPS - d.step) * 20,
    });
    if (stage === "transcribe")
      LINES.forEach((l) =>
        emit("dub:line", {
          runId: "r1",
          line: { ...l, sectionIndex: 0, audio: null, audioMs: null, status: "transcribed" },
        }),
      );
    d.step += 1;
    if (d.step >= STEPS) {
      emit("dub:stage", { runId: "r1", stage: "mix", status: "done" });
      store({ ...d, run: "completed" });
      running = false;
      emit("dub:done", { runId: "r1", state: "completed" });
      return;
    }
    store(d);
    timer = window.setTimeout(tick, fake.slow ? 400 : 120);
  };
  const begin = () => {
    running = true;
    timer = window.setTimeout(tick, 100);
  };

  const dub = () => ({
    source: !load().configured ? null : { path: "/videos/holiday.mp4", durationMs: 20000 },
    sourceLanguage: "fr",
    detectedLanguage: null,
    targetLanguage: "en",
    options: {
      muteOriginal: false,
      exportKind: "video",
      exportName: "holiday",
      quality: "standard",
    },
    lines:
      load().run === "completed"
        ? LINES.map((l) => ({
            ...l,
            sectionIndex: 0,
            audio: "x",
            audioMs: 1000,
            status: "synthesized",
          }))
        : [],
  });

  const handle = {
    id: "p1",
    name: "Dub test",
    path: "/demo/dub.flick",
    readOnly: false,
    dirty: false,
    notes: "",
  };
  const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
    run_compat_check: () => ({
      mode: "fullLocal",
      items: [],
      missing: [],
      blockedMessage: null,
      provisional: false,
      generatedAt: "",
    }),
    project_recent: () => [
      { path: "/demo/dub.flick", name: "Dub test", openedAt: "", available: true },
    ],
    project_open: () => ({ handle, recoveryAvailable: false }),
    project_suggest_file_name: (a) => `${a.name}.flick`,
    project_new: () => handle,
    "plugin:dialog|save": () => "/demo/dub.flick",
    "plugin:dialog|open": () => fake.nextOpen,
    "plugin:opener|reveal_item_in_dir": (a) => {
      fake.revealed.push(String((a.paths as string[])[0]));
    },
    "plugin:event|listen": (a) => {
      (handlers[a.event as string] ??= []).push(a.handler as number);
      return a.handler;
    },
    "plugin:event|unlisten": () => undefined,
    dub_probe_source: () => ({
      durationMs: 20000,
      hasAudio: true,
      hasVideo: true,
      container: "mp4",
      sizeBytes: 1_000_000,
    }),
    dub_configure: () => {
      store({ ...load(), configured: true });
      return handle;
    },
    dub_preflight: () => ({
      missingModels: load().whisper
        ? []
        : [{ id: "whisper", name: "Speech recognition", sizeBytes: 4 * 1024 * 1024 }],
      downloadBytes: 0,
      diskNeededBytes: 80_000_000,
      diskFreeBytes: 50_000_000_000,
      machineClass: "minimum",
      warnings: [],
      blockers: [],
    }),
    model_download_start: () => {
      window.setTimeout(() => {
        store({ ...load(), whisper: true });
        emit("model:progress", {
          id: "whisper",
          name: "Speech recognition",
          purpose: "",
          licence: "MIT",
          sizeBytes: 4194304,
          state: "installed",
          bytesDone: 4194304,
          errorCode: null,
        });
      }, 200);
    },
    dub_start: () => {
      store({ ...load(), run: "running", step: 0 });
      begin();
      return "r1";
    },
    dub_resume: () => {
      store({ ...load(), run: "running" });
      begin();
      return "r1";
    },
    dub_cancel: () => {
      window.clearTimeout(timer);
      running = false;
      store({ ...load(), run: "cancelled" });
      emit("dub:done", { runId: "r1", state: "cancelled" });
    },
    dub_discard_run: () => store({ ...load(), run: "none", step: 0 }),
    dub_status: () => {
      const d = load();
      // After a reload nothing is running: a run that was `running` was cut short.
      const interrupted = d.run === "running" && !running;
      return {
        run:
          d.run === "none"
            ? null
            : { id: "r1", state: d.run, error: null, sections: [{ index: 0 }] },
        interrupted,
        running,
        dub: dub(),
      };
    },
    dub_export: (a) => {
      const name = `${a.folder}/${a.fileName}.${a.kind === "video" ? "mp4" : "m4a"}`;
      const d = load();
      if (d.exported.includes(name) && !a.overwrite)
        throw { code: "FILE_EXISTS", messageKey: "dub.error.fileExists" };
      store({ ...d, exported: [...d.exported.filter((e) => e !== name), name] });
      return { path: name, sizeBytes: 2_500_000 };
    },
  };

  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    transformCallback: (fn: (e: unknown) => void) => {
      const id = ++cb;
      (window as unknown as Record<string, unknown>)[`_${id}`] = fn;
      return id;
    },
    invoke: async (cmd: string, args: Record<string, unknown>) => {
      const run = commands[cmd];
      if (!run) throw new Error(`fake core: unhandled command ${cmd}`);
      return run(args ?? {});
    },
  };
  (
    window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__: unknown }
  ).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => {},
  };
}

async function start(page: Page) {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByLabel("Project name").fill("Dub test");
  await page.getByRole("button", { name: "Create…" }).click();
  await page.getByTestId("dub-toggle").click();
}

async function chooseVideo(page: Page) {
  await page.evaluate(() => (window.__fakeDub.nextOpen = "/videos/holiday.mp4"));
  await page.getByRole("button", { name: "Choose a video…" }).click();
  await expect(page.getByTestId("dub-source")).toContainText("holiday");
  await expect(page.getByTestId("dub-source")).toContainText("0:20");
}

test("missing models are listed with sizes, downloaded, and then the dub can start", async ({
  page,
}) => {
  await start(page);
  const startButton = page.getByRole("button", { name: "Start dubbing" });
  await expect(startButton).toBeDisabled();
  await chooseVideo(page);

  await expect(page.getByTestId("dub-missing")).toContainText("Speech recognition");
  await expect(page.getByTestId("dub-missing")).toContainText("4.0 MB");
  await expect(startButton).toBeDisabled();
  await expect(page.getByTestId("dub-disk")).toContainText("needs about");

  await page.getByRole("button", { name: /^Download/ }).click();
  await expect(page.getByTestId("dub-missing")).toHaveCount(0);
  await expect(startButton).toBeEnabled();
});

test("a full dub shows progress, then the transcript, and exports both kinds", async ({ page }) => {
  await start(page);
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();

  await expect(page.getByTestId("dub-progress")).toBeVisible();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });

  const rows = page.getByTestId("transcript-table").locator("tbody tr");
  await expect(rows).toHaveCount(2);
  await expect(rows.first()).toContainText("Bonjour tout le monde.");
  await expect(rows.first()).toContainText("Hello everyone.");
  await expect(rows.nth(1)).toContainText("Too long");

  await page.evaluate(() => (window.__fakeDub.nextOpen = "/exports"));
  await page.getByRole("button", { name: "Choose a folder…" }).click();
  await page.getByTestId("dub-result").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("export-done")).toContainText("holiday.mp4");

  await page.getByRole("combobox", { name: "Save as" }).click();
  await page.getByRole("option", { name: "Audio only" }).click();
  await page.getByTestId("dub-result").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("export-done")).toContainText("holiday.m4a");

  await page.getByRole("button", { name: "Show in folder" }).click();
  expect(await page.evaluate(() => window.__fakeDub.revealed)).toEqual(["/exports/holiday.m4a"]);
});

test("saving over an existing file asks first", async ({ page }) => {
  await start(page);
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
  await page.evaluate(() => (window.__fakeDub.nextOpen = "/exports"));
  await page.getByRole("button", { name: "Choose a folder…" }).click();
  await page.getByTestId("dub-result").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("export-done")).toBeVisible();

  await page.getByTestId("dub-result").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("export-error")).toContainText("already exists");
  await page.getByRole("button", { name: "Replace it" }).click();
  await expect(page.getByTestId("export-done")).toBeVisible();
  await expect(page.getByTestId("export-error")).toHaveCount(0);
});

test("a dub cut short by closing the app can be resumed", async ({ page }) => {
  await start(page);
  await page.evaluate(() => (window.__fakeDub.slow = true));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("dub-progress")).toBeVisible();
  await expect(
    page.locator(
      '[data-stage="separate"][data-status="running"], [data-stage="transcribe"][data-status="running"]',
    ),
  ).toBeVisible();

  await page.reload(); // kill and relaunch
  await page.getByRole("button", { name: "Dub test" }).click();
  await page.getByTestId("dub-toggle").click();
  await expect(page.getByTestId("dub-progress")).toContainText("The dub was interrupted");
  await page.getByRole("button", { name: "Resume" }).click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
});

test("stopping a dub keeps it resumable", async ({ page }) => {
  await start(page);
  await page.evaluate(() => (window.__fakeDub.slow = true));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("dub-progress")).toBeVisible();
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.getByTestId("dub-progress")).toContainText("Stopped");
  await expect(page.getByRole("button", { name: "Resume" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start over" })).toBeVisible();
});

import { expect, test, type Page } from "@playwright/test";

// The Tauri core is replaced by a fake that "downloads" in the page, keeps its state in
// localStorage (so reloading stands in for quitting and relaunching), and sends the same
// `model:progress` events as the real core. Real downloads, resume, hashes and disk checks are
// tested in core/tests/downloader.rs against a local HTTP server.

declare global {
  interface Window {
    __fakeModels: { freeBytes: number };
  }
}

function installFakeCore() {
  const SIZE = 4 * 1024 * 1024;
  type Rec = { state: string; bytesDone: number };
  const load = (): Rec =>
    JSON.parse(localStorage.getItem("fakeModel") ?? '{"state":"notInstalled","bytesDone":0}');
  const store = (r: Rec) => localStorage.setItem("fakeModel", JSON.stringify(r));

  // After a relaunch nothing is running: an unfinished download is just a paused one on disk.
  const boot = load();
  if (boot.state === "downloading" || boot.state === "verifying") {
    store({ state: "paused", bytesDone: boot.bytesDone });
  }

  const fake = { freeBytes: 100 * 1024 ** 3 };
  (window as unknown as { __fakeModels: typeof fake }).__fakeModels = fake;

  const record = (r: Rec, errorCode: string | null = null) => ({
    id: "m1",
    name: "Test model",
    purpose: "Used to test downloads",
    licence: "MIT",
    sizeBytes: SIZE,
    state: r.state,
    bytesDone: r.bytesDone,
    errorCode,
  });

  // Event plumbing, shaped like Tauri's.
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
  const tick = () => {
    const r = load();
    if (r.state !== "downloading") return;
    r.bytesDone = Math.min(SIZE, r.bytesDone + SIZE / 8);
    if (r.bytesDone >= SIZE) {
      store({ state: "verifying", bytesDone: SIZE });
      emit("model:progress", record({ state: "verifying", bytesDone: SIZE }));
      timer = window.setTimeout(() => {
        store({ state: "installed", bytesDone: SIZE });
        emit("model:progress", record({ state: "installed", bytesDone: SIZE }));
      }, 150);
      return;
    }
    store(r);
    emit("model:progress", record(r));
    timer = window.setTimeout(tick, 150);
  };

  const start = () => {
    const r = load();
    if (fake.freeBytes < SIZE - r.bytesDone) {
      throw {
        code: "NO_SPACE",
        details: { neededBytes: SIZE - r.bytesDone, freeBytes: fake.freeBytes },
      };
    }
    store({ state: "downloading", bytesDone: r.bytesDone });
    timer = window.setTimeout(tick, 150);
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
    project_recent: () => [],
    settings_get: () => ({
      settings: {
        settingsVersion: 1,
        theme: "system",
        general: { uiLanguage: "en", defaultSaveFolder: null },
        hotkeys: {},
        analyticsEnabled: false,
        modelsFolder: null,
        lastCompatMode: null,
      },
      notice: null,
    }),
    "plugin:event|listen": (a) => {
      (handlers[a.event as string] ??= []).push(a.handler as number);
      return a.handler;
    },
    "plugin:event|unlisten": () => undefined,
    models_catalogue: () => [record(load(), localStorage.getItem("fakeModelError"))],
    model_download_start: start,
    model_download_resume: start,
    model_download_pause: () => {
      window.clearTimeout(timer);
      const r = load();
      store({ state: "paused", bytesDone: r.bytesDone });
      emit("model:progress", record({ state: "paused", bytesDone: r.bytesDone }));
    },
    model_download_cancel: () => {
      window.clearTimeout(timer);
      store({ state: "notInstalled", bytesDone: 0 });
      emit("model:progress", record({ state: "notInstalled", bytesDone: 0 }));
    },
    model_delete: () => {
      store({ state: "notInstalled", bytesDone: 0 });
    },
  };

  const internals = {
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
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = internals;
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
  await page.getByRole("button", { name: "Models", exact: true }).click();
}

const state = (page: Page) => page.getByTestId("state-m1");

test("shows what a model is before anything is downloaded", async ({ page }) => {
  await start(page);
  await expect(state(page)).toHaveText("Not downloaded");
  await page.getByRole("button", { name: "Download…" }).click();
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("Test model");
  await expect(dialog).toContainText("Used to test downloads");
  await expect(dialog).toContainText("4.0 MB");
  await expect(dialog).toContainText("MIT");
  // Nothing has started yet.
  await expect(state(page)).toHaveText("Not downloaded");
});

test("declining the confirmation starts nothing", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Not now" }).click();
  await expect(state(page)).toHaveText("Not downloaded");
});

test("a download shows progress and ends installed with its size counted", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Start download" }).click();
  await expect(state(page)).toHaveText("Downloading");
  await expect(state(page)).toHaveText("Installed", { timeout: 10_000 });
  await expect(page.getByTestId("storage-total")).toContainText("4.0 MB");
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();
});

test("pause, restart the app, resume: the progress is kept", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Start download" }).click();
  await expect(page.getByTestId("progress-m1")).not.toContainText("0 B of", { timeout: 5_000 });
  await page.getByRole("button", { name: "Pause" }).click();
  await expect(state(page)).toHaveText("Paused");
  const kept = await page.getByTestId("progress-m1").innerText();

  await page.reload(); // quit and relaunch
  await page.getByRole("button", { name: "Models", exact: true }).click();
  await expect(state(page)).toHaveText("Paused");
  await expect(page.getByTestId("progress-m1")).toHaveText(kept);

  await page.getByRole("button", { name: "Resume" }).click();
  await expect(state(page)).toHaveText("Installed", { timeout: 10_000 });
});

test("a full disk refuses the download and says how much is needed and free", async ({ page }) => {
  await start(page);
  await page.evaluate(() => (window.__fakeModels.freeBytes = 1024 * 1024));
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Start download" }).click();
  await expect(page.getByRole("alert")).toContainText("needs 4.0 MB and only 1.0 MB is free");
  await expect(state(page)).toHaveText("Not downloaded");
});

test("cancel throws away a partial download", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Start download" }).click();
  await expect(page.getByRole("button", { name: "Cancel" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(state(page)).toHaveText("Not downloaded");
});

test("an installed model can be deleted to free space after confirming", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Download…" }).click();
  await page.getByRole("button", { name: "Start download" }).click();
  await expect(state(page)).toHaveText("Installed", { timeout: 10_000 });

  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(state(page)).toHaveText("Not downloaded");
  await expect(page.getByTestId("storage-total")).toContainText("0 B");
});

test("a stopped download explains why and can be retried", async ({ page }) => {
  await start(page);
  await page.evaluate(() => {
    localStorage.setItem("fakeModel", JSON.stringify({ state: "failed", bytesDone: 1024 * 1024 }));
    localStorage.setItem("fakeModelError", "OFFLINE");
  });
  await page.reload();
  await page.getByRole("button", { name: "Models", exact: true }).click();
  await expect(state(page)).toHaveText("Download stopped");
  await expect(page.getByRole("alert")).toContainText("could not reach the internet");
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
});

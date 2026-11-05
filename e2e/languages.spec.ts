import { expect, test, type Page } from "@playwright/test";

// The Tauri core is replaced by a fake that answers `languages_list` and `languages_check_pair`
// from a small data table, so these tests check that the screen follows data: nothing in the UI
// names a language. The real list and pair rules are tested in core/tests/languages.rs.

declare global {
  interface Window {
    __fakeLang: {
      nextOpen: string | null;
      extra: boolean;
      started: number;
      lastOptions: unknown;
    };
  }
}

function installFakeCore() {
  const fake = {
    nextOpen: null as string | null,
    extra: false,
    started: 0,
    lastOptions: null as unknown,
  };
  (window as unknown as { __fakeLang: typeof fake }).__fakeLang = fake;

  type Lang = {
    code: string;
    transcribe: boolean;
    translateFrom: boolean;
    translateTo: boolean;
    speak: { voices: number; permissiveVoices: number };
    quality: { transcribe: null; translate: null; speak: null };
  };
  const lang = (code: string, over: Partial<Lang> = {}): Lang => ({
    code,
    transcribe: true,
    translateFrom: true,
    translateTo: true,
    speak: { voices: 1, permissiveVoices: 1 },
    quality: { transcribe: null, translate: null, speak: null },
    ...over,
  });
  const languages = (optIn: string[]) => [
    lang("en"),
    lang("fr"),
    lang("de"),
    lang("pt"),
    lang("km", {
      translateTo: optIn.includes("nllb"),
      translateFrom: optIn.includes("nllb"),
      speak: { voices: 0, permissiveVoices: 0 },
    }),
    ...(fake.extra ? [lang("sw")] : []),
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

  const handle = {
    id: "p1",
    name: "Lang test",
    path: "/demo/lang.flick",
    readOnly: false,
    dirty: false,
    notes: "",
  };
  let configured = false;
  let detected: string | null = null;
  let state = "none";

  const check = (source: string, target: string, optIn: string[]) => {
    const steps = [
      { step: "transcribe", supported: true, quality: null, reason: null },
    ] as unknown[];
    const fallbacks: unknown[] = [];
    const translateOk = target !== "km" || optIn.includes("nllb");
    steps.push({
      step: "translate",
      supported: translateOk,
      quality: null,
      reason: translateOk ? null : "languages.reason.noTranslator",
    });
    steps.push({
      step: "speak",
      supported: target !== "km",
      quality: null,
      reason: target !== "km" ? null : "languages.reason.noVoice",
    });
    if (target === "km") {
      fallbacks.push({
        kind: "subtitlesOnly",
        labelKey: "languages.fallback.subtitles",
        modelIds: [],
        noticeKeys: [],
      });
      if (!translateOk) {
        fallbacks.push({
          kind: "optInNonCommercial",
          labelKey: "languages.fallback.optIn",
          modelIds: ["nllb"],
          noticeKeys: ["licence.nonCommercial"],
        });
      }
    }
    return { ok: steps.every((s) => (s as { supported: boolean }).supported), steps, fallbacks };
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
    project_suggest_file_name: (a) => `${a.name}.flick`,
    project_new: () => handle,
    "plugin:dialog|save": () => "/demo/lang.flick",
    "plugin:dialog|open": () => fake.nextOpen,
    "plugin:event|listen": (a) => {
      (handlers[a.event as string] ??= []).push(a.handler as number);
      return a.handler;
    },
    "plugin:event|unlisten": () => undefined,
    languages_list: (a) => languages((a.optIn as string[]) ?? []),
    languages_check_pair: (a) =>
      check(a.source as string, a.target as string, (a.optIn as string[]) ?? []),
    dub_probe_source: () => ({
      durationMs: 20000,
      hasAudio: true,
      hasVideo: true,
      container: "mp4",
      sizeBytes: 1_000_000,
    }),
    dub_configure: (a) => {
      configured = true;
      fake.lastOptions = a.options;
      return handle;
    },
    dub_preflight: () => ({
      missingModels: [],
      downloadBytes: 0,
      diskNeededBytes: 1_000_000,
      diskFreeBytes: 50_000_000_000,
      machineClass: "minimum",
      warnings: [],
      blockers: [],
    }),
    dub_start: () => {
      fake.started += 1;
      state = "running";
      detected = null;
      window.setTimeout(() => {
        detected = "fr";
        emit("dub:detected", { runId: "r1", language: "fr" });
        emit("dub:stage", { runId: "r1", stage: "extract", status: "running" });
        emit("dub:progress", {
          runId: "r1",
          stage: "extract",
          stageProgress: 0.5,
          overallProgress: 0.1,
          sectionIndex: 0,
          sectionCount: 1,
          etaSeconds: 60,
        });
      }, 150);
      return "r1";
    },
    dub_status: () => ({
      run: state === "none" ? null : { id: "r1", state, error: null, sections: [{ index: 0 }] },
      interrupted: false,
      running: state === "running",
      dub: {
        source: configured ? { path: "/videos/clip.mp4", durationMs: 20000 } : null,
        sourceLanguage: "auto",
        detectedLanguage: detected,
        targetLanguage: "en",
        options: {
          muteOriginal: false,
          exportKind: "video",
          exportName: "clip",
          quality: "standard",
          fallbacks: {
            subtitlesOnly: false,
            genericVoice: false,
            pivotViaEnglish: false,
            optIn: [],
          },
        },
        lines: [],
      },
    }),
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
  await page.getByLabel("Project name").fill("Lang test");
  await page.getByRole("button", { name: "Create…" }).click();
  await page.getByTestId("dub-toggle").click();
}

async function chooseVideo(page: Page) {
  await page.evaluate(() => (window.__fakeLang.nextOpen = "/videos/clip.mp4"));
  await page.getByRole("button", { name: "Choose a video…" }).click();
  await expect(page.getByTestId("dub-source")).toContainText("clip");
}

const sourcePicker = (page: Page) => page.getByRole("combobox", { name: "Spoken language" });
const targetPicker = (page: Page) => page.getByRole("combobox", { name: "Dub into" });

test("typing part of a language name filters the list and picks by code", async ({ page }) => {
  await start(page);
  await expect(sourcePicker(page)).toContainText("Detect automatically");
  await sourcePicker(page).click();
  await page.getByPlaceholder("Search languages…").fill("ger");
  await expect(page.getByRole("option")).toHaveCount(1);
  await page.getByRole("option", { name: "German" }).click();
  await expect(sourcePicker(page)).toContainText("German");

  // The target list has no auto-detect entry.
  await targetPicker(page).click();
  await expect(page.getByRole("option", { name: "Detect automatically" })).toHaveCount(0);
  await page.getByPlaceholder("Search languages…").fill("zzz");
  await expect(page.getByText("No language found.")).toBeVisible();
});

test("a language added to the data appears with no change to the screen", async ({ page }) => {
  await start(page);
  await targetPicker(page).click();
  await expect(page.getByRole("option", { name: "Swahili" })).toHaveCount(0);
  await page.keyboard.press("Escape");

  // The catalogue gains a language; reopening the screen reads the list again.
  await page.evaluate(() => (window.__fakeLang.extra = true));
  await page.getByTestId("dub-toggle").click();
  await page.getByTestId("dub-toggle").click();
  await targetPicker(page).click();
  await page.getByPlaceholder("Search languages…").fill("swa");
  await expect(page.getByRole("option", { name: "Swahili" })).toHaveCount(1);
});

test("auto-detect shows the detected language once the run knows it", async ({ page }) => {
  await start(page);
  await chooseVideo(page);
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("detected-language")).toContainText("Detected language: French");
});

test("an unsupported pair is explained before start and start waits for a choice", async ({
  page,
}) => {
  await start(page);
  await chooseVideo(page);
  const startButton = page.getByRole("button", { name: "Start dubbing" });
  await expect(startButton).toBeEnabled();

  await targetPicker(page).click();
  await page.getByRole("option", { name: "Khmer" }).click();
  const panel = page.getByTestId("pair-check");
  await expect(panel).toBeVisible();
  await expect(panel.locator('[data-step="speak"]')).toContainText(
    "No voice speaks the target language.",
  );
  await expect(panel.locator('[data-step="translate"]')).toContainText("Translating");
  await expect(startButton).toBeDisabled();
  await expect(page.getByRole("button", { name: "Start dubbing" })).toBeDisabled();
  expect(await page.evaluate(() => window.__fakeLang.started)).toBe(0);

  // Subtitles alone do not fix translation: it needs the non-commercial model.
  await panel.getByRole("checkbox", { name: "Make subtitles only, without speech" }).click();
  await expect(startButton).toBeDisabled();
  await expect(panel.getByTestId("optin-notice")).toContainText("free only for non-commercial use");
  await panel
    .getByRole("checkbox", { name: "Use a model that is free only for non-commercial use" })
    .click();
  // Translation now works; speech is still limited but the user chose subtitles for it.
  await expect(panel.locator('[data-step="translate"]')).toHaveCount(0);
  await expect(panel.locator('[data-step="speak"]')).toHaveCount(1);
  await expect(startButton).toBeEnabled();
});

test("the same language needs a confirmation", async ({ page }) => {
  await start(page);
  await chooseVideo(page);
  await sourcePicker(page).click();
  await page.getByRole("option", { name: "English" }).click();
  await expect(page.getByRole("button", { name: "Start dubbing" })).toBeDisabled();
  await page.getByRole("button", { name: "Use the same language" }).click();
  await expect(page.getByRole("button", { name: "Start dubbing" })).toBeEnabled();
});

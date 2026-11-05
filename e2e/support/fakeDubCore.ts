import { expect, type Page } from "@playwright/test";

// The Tauri core is replaced by a fake that "runs" a dub in the page: it sends the same
// `dub:*` events as the real core, keeps its state in localStorage (so a reload stands in for
// killing and relaunching the app), and refuses to overwrite files. The real pipeline,
// resume-after-kill and export are tested in core/tests/pipeline_resume.rs and
// core/tests/stage_mix_export.rs.

declare global {
  interface Window {
    __fakeDub: {
      nextOpen: string | null;
      revealed: string[];
      slow: boolean;
      subtitlesOnly: boolean;
      /** Send the translation step to this cloud service (needs the user's agreement first). */
      cloud: string | null;
      /** `dub:cloud` events sent when the translate stage starts. */
      cloudNotes: { kind: string; keyLabel?: string; reason?: string; waitSeconds?: number }[];
      /** Fail the run when it reaches this step (0 extract .. 5 mix) with this error. */
      failAtStep: number | null;
      failure: {
        code: string;
        stage: string | null;
        messageKey: string;
        nextStepKey: string;
        details?: Record<string, unknown>;
      } | null;
    };
  }
}

export function installFakeCore() {
  const fake = {
    nextOpen: null as string | null,
    revealed: [] as string[],
    slow: false,
    subtitlesOnly: false,
    cloud: null as string | null,
    cloudNotes: [] as Window["__fakeDub"]["cloudNotes"],
    failAtStep: null as number | null,
    failure: null as Window["__fakeDub"]["failure"],
  };
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
    run: "none" | "running" | "completed" | "cancelled" | "failed";
    step: number;
    error?: NonNullable<Window["__fakeDub"]["failure"]>;
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
    if (fake.failure && fake.failAtStep === d.step) {
      const { failure } = fake;
      if (d.step > 0) emit("dub:stage", { runId: "r1", stage: stages[d.step - 1], status: "done" });
      emit("dub:stage", { runId: "r1", stage, status: "failed" });
      emit("dub:error", {
        runId: "r1",
        code: failure.code,
        stage: failure.stage,
        sectionIndex: 0,
        message: failure.messageKey,
        nextStep: failure.nextStepKey,
        details: failure.details ?? null,
      });
      store({ ...d, run: "failed", error: failure });
      running = false;
      emit("dub:done", { runId: "r1", state: "failed" });
      return;
    }
    if (d.step > 0) emit("dub:stage", { runId: "r1", stage: stages[d.step - 1], status: "done" });
    emit("dub:stage", { runId: "r1", stage, status: "running" });
    if (stage === "translate") {
      for (const n of fake.cloudNotes) {
        emit("dub:cloud", {
          runId: "r1",
          step: "translate",
          serviceId: fake.cloud ?? "google-gemini",
          keyLabel: n.keyLabel ?? null,
          reason: n.reason ?? null,
          waitSeconds: n.waitSeconds ?? null,
          kind: n.kind,
        });
      }
    }
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

  // The service a step goes to: set directly by a test, or chosen in Settings.
  const engines = (): { translate: string; speak: string } =>
    JSON.parse(localStorage.getItem("fakeEngines") ?? '{"translate":"local","speak":"local"}');
  const cloudService = (): string | null =>
    fake.cloud ?? (engines().translate !== "local" ? engines().translate : null);

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
      ...(cloudService()
        ? {
            fallbacks: {
              subtitlesOnly: false,
              genericVoice: false,
              pivotViaEnglish: false,
              optIn: [],
              engines: { translate: cloudService(), speak: "local" },
            },
          }
        : {}),
    },
    cloudUsage:
      cloudService() && load().run === "completed"
        ? [
            {
              step: "translate",
              serviceId: cloudService(),
              serviceName: "Google Gemini API",
              chars: 300,
              requests: 1,
              keysUsed: ["mine"],
              switches: 0,
              waitedSeconds: 0,
              runId: "r1",
            },
          ]
        : [],
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
    languages_list: () =>
      ["en", "fr", "de"].map((code) => ({
        code,
        transcribe: true,
        translateFrom: true,
        translateTo: true,
        speak: { voices: 1, permissiveVoices: 1 },
        quality: { transcribe: null, translate: null, speak: null },
      })),
    languages_check_pair: () => ({
      ok: true,
      steps: [
        { step: "transcribe", supported: true, quality: null, reason: null },
        { step: "translate", supported: true, quality: null, reason: null },
        { step: "speak", supported: true, quality: null, reason: null },
      ],
      fallbacks: [],
    }),
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
      cloud: cloudService()
        ? [
            {
              step: "translate",
              serviceId: cloudService(),
              serviceName: "Google Gemini API",
              estimatedChars: 300,
              needsConsent: localStorage.getItem("fakeConsent") !== "1",
              reason: "first",
              hasKey: true,
            },
          ]
        : [],
    }),
    cloud_services: () => [
      {
        id: "google-gemini",
        name: "Google Gemini API",
        steps: ["translate", "speak"],
        needsRegion: false,
        termsUrl: "https://ai.google.dev/gemini-api/terms",
        languages: { translate: [], speak: [] },
      },
    ],
    cloud_notice: () => ({
      serviceName: "Google Gemini API",
      step: "translate",
      sends: ["cloud.notice.translate"],
      doesNotSend: ["cloud.notice.doesNotSend", "cloud.notice.bill"],
      termsUrl: "https://ai.google.dev/gemini-api/terms",
      estimatedChars: 300,
      noticeVersion: 1,
    }),
    cloud_keys_list: () => [
      {
        id: "k1",
        serviceId: "google-gemini",
        label: "mine",
        region: null,
        last4: "1234",
        order: 0,
        state: "verified",
        verifiedAt: "2026-11-05T00:00:00Z",
        storage: "os",
      },
    ],
    cloud_engines_get: () => engines(),
    cloud_engines_set: (a) => {
      const next = { ...engines(), [String(a.step)]: String(a.engine) };
      localStorage.setItem("fakeEngines", JSON.stringify(next));
      return next;
    },
    cloud_consent_list: () =>
      localStorage.getItem("fakeConsent") === "1"
        ? [
            {
              serviceId: "google-gemini",
              step: "translate",
              agreedAt: "2026-11-05T00:00:00Z",
              agreedChars: 300,
              noticeVersion: 1,
            },
          ]
        : [],
    cloud_consent_withdraw: () => localStorage.removeItem("fakeConsent"),
    cloud_consent_grant: () => {
      localStorage.setItem("fakeConsent", "1");
      return {};
    },
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
      if (cloudService() && localStorage.getItem("fakeConsent") !== "1") {
        throw { code: "CLOUD_CONSENT_REQUIRED", messageKey: "dub.error.cloudConsentRequired" };
      }
      store({ ...load(), run: "running", step: 0 });
      begin();
      return "r1";
    },
    dub_resume: () => {
      fake.failure = null;
      store({ ...load(), run: "running", error: undefined });
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
            : {
                id: "r1",
                state: d.run,
                error: d.error
                  ? {
                      code: d.error.code,
                      stage: d.error.stage,
                      sectionIndex: 0,
                      messageKey: d.error.messageKey,
                      nextStepKey: d.error.nextStepKey,
                      details: d.error.details ?? null,
                    }
                  : null,
                sections: [{ index: 0 }],
              },
        interrupted,
        running,
        dub: dub(),
        hasDubAudio: !fake.subtitlesOnly,
      };
    },
    dub_export: (a) => {
      const ext = a.kind === "video" ? "mp4" : a.kind === "subtitles" ? "srt" : "m4a";
      const name = `${a.folder}/${a.fileName}.${ext}`;
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

export async function start(page: Page) {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByLabel("Project name").fill("Dub test");
  await page.getByRole("button", { name: "Create…" }).click();
  await page.getByTestId("dub-toggle").click();
}

export async function chooseVideo(page: Page) {
  await page.evaluate(() => (window.__fakeDub.nextOpen = "/videos/holiday.mp4"));
  await page.getByRole("button", { name: "Choose a video…" }).click();
  await expect(page.getByTestId("dub-source")).toContainText("holiday");
  await expect(page.getByTestId("dub-source")).toContainText("0:20");
}

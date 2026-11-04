import { expect, test } from "@playwright/test";

// The core is replaced by a fake that sends `memory:sample` events, shaped like Tauri's. The
// 85% rule and the real sampling are tested in core/tests/memory_level.rs.

declare global {
  interface Window {
    __fakeMemory: { send: (usedPercent: number, level: "normal" | "warning") => void };
  }
}

function installFakeCore() {
  const handlers: number[] = [];
  let cb = 0;
  const GB = 1024 ** 3;
  (window as unknown as { __fakeMemory: unknown }).__fakeMemory = {
    send: (usedPercent: number, level: string) =>
      handlers.forEach((h) =>
        (window as unknown as Record<string, (e: unknown) => void>)[`_${h}`]({
          event: "memory:sample",
          id: h,
          payload: {
            appBytes: 312 * 1024 * 1024,
            systemUsedBytes: (16 * GB * usedPercent) / 100,
            systemTotalBytes: 16 * GB,
            level,
          },
        }),
      ),
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
    models_catalogue: () => [],
    "plugin:event|listen": (a) => {
      if (a.event === "memory:sample") handlers.push(a.handler as number);
      return a.handler;
    },
    "plugin:event|unlisten": () => undefined,
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

test("the badge appears once the first sample arrives and updates", async ({ page }) => {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  await expect(page.getByTestId("mode")).toBeVisible();
  await expect(page.getByTestId("memory")).toHaveCount(0);

  await page.evaluate(() => window.__fakeMemory.send(40, "normal"));
  const badge = page.getByTestId("memory");
  await expect(badge).toContainText("App 312.0 MB");
  await expect(badge).toContainText("40% used");
  await expect(badge).toHaveAttribute("data-level", "normal");

  await page.evaluate(() => window.__fakeMemory.send(55, "normal"));
  await expect(badge).toContainText("55% used");
});

test("a crowded computer turns the badge to a warning with a hint", async ({ page }) => {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  // The badge listens only once the shell is on screen, so wait for it before sending.
  await expect(page.getByTestId("mode")).toBeVisible();
  await page.evaluate(() => window.__fakeMemory.send(90, "warning"));
  const badge = page.getByTestId("memory");
  await expect(badge).toHaveAttribute("data-level", "warning");
  await expect(badge).toContainText("running low on memory");

  await page.evaluate(() => window.__fakeMemory.send(50, "normal"));
  await expect(badge).toHaveAttribute("data-level", "normal");
  await expect(badge).not.toContainText("running low on memory");
});

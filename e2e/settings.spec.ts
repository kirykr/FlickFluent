import { expect, test, type Page } from "@playwright/test";
import { displayCombo } from "../src/features/settings/hotkeys";

// The browser reports the host OS, so expected shortcut text follows it.
const MAC = process.platform === "darwin";
const show = (combo: string) => displayCombo(combo, MAC);

// The Tauri core is replaced by a small fake that keeps settings in localStorage, so reloading the
// page stands in for restarting the app. The fake mirrors the core's rules (clash detection,
// unassigning with null); the real rules are tested in core/tests/settings.rs.

function installFakeCore() {
  const DEFAULTS = {
    settingsVersion: 1,
    theme: "system",
    general: { uiLanguage: "en", defaultSaveFolder: null },
    hotkeys: {
      "project.new": "Mod+N",
      "project.open": "Mod+O",
      "project.save": "Mod+S",
      "project.close": "Mod+W",
    } as Record<string, string>,
    analyticsEnabled: false,
    modelsFolder: null,
    lastCompatMode: null,
  };
  const load = () => JSON.parse(localStorage.getItem("fakeSettings") ?? JSON.stringify(DEFAULTS));
  const store = (s: unknown) => localStorage.setItem("fakeSettings", JSON.stringify(s));
  const view = (settings: unknown) => ({
    settings,
    notice: localStorage.getItem("fakeNotice"),
  });
  const report = (mode: string) => ({
    mode,
    items: [],
    missing: [],
    blockedMessage: null,
    provisional: false,
    generatedAt: "",
  });

  const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
    run_compat_check: () => report(localStorage.getItem("fakeMode") ?? "fullLocal"),
    project_recent: () => [],
    settings_get: () => {
      const v = view(load());
      localStorage.removeItem("fakeNotice"); // shown once
      return v;
    },
    settings_set: ({ patch }) => {
      const s = load();
      const p = patch as Record<string, unknown>;
      if (typeof p.theme === "string" && !["light", "dark", "system"].includes(p.theme)) {
        throw { code: "INVALID_SETTING" };
      }
      const hot = (p.hotkeys ?? {}) as Record<string, string | null>;
      const next = { ...s.hotkeys } as Record<string, string>;
      for (const [action, combo] of Object.entries(hot)) {
        if (combo === null) delete next[action];
        else next[action] = combo;
      }
      for (const [action, combo] of Object.entries(hot)) {
        const owner = Object.entries(next).find(([a, c]) => a !== action && c === combo && combo);
        if (owner) throw { code: "HOTKEY_CONFLICT", details: { action: owner[0], combo } };
      }
      const merged = {
        ...s,
        ...p,
        general: { ...s.general, ...((p.general as object) ?? {}) },
        hotkeys: next,
      };
      store(merged);
      return view(merged);
    },
    settings_reset: () => {
      store(DEFAULTS);
      return view(DEFAULTS);
    },
  };

  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: Record<string, unknown>) => {
      const run = commands[cmd];
      if (!run) throw new Error(`fake core: unhandled command ${cmd}`);
      return run(args ?? {});
    },
  };
}

async function start(page: Page) {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
}

const html = (page: Page) => page.locator("html");

test("a fresh install has analytics off and follows the system theme", async ({ page }) => {
  await start(page);
  await expect(page.getByLabel("Share anonymous usage data")).not.toBeChecked();
  await expect(page.getByLabel("Theme")).toHaveValue("system");
  await expect(html(page)).not.toHaveAttribute("data-theme", /.+/);
});

test("turning analytics on shows what is collected", async ({ page }) => {
  await start(page);
  await page.getByLabel("Share anonymous usage data").check();
  await expect(page.getByText("Never project content, media or file names")).toBeVisible();
});

test("theme and analytics choices survive a restart", async ({ page }) => {
  await start(page);
  await page.getByLabel("Theme").selectOption("dark");
  await page.getByLabel("Share anonymous usage data").check();
  await expect(html(page)).toHaveAttribute("data-theme", "dark");

  await page.reload(); // restart
  await expect(html(page)).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByLabel("Theme")).toHaveValue("dark");
  await expect(page.getByLabel("Share anonymous usage data")).toBeChecked();
});

test("a changed shortcut is kept after a restart", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Change: Save project" }).click();
  await page.keyboard.press("ControlOrMeta+Shift+KeyK");
  await expect(page.getByTestId("combo-project.save")).toHaveText(show("Mod+Shift+K"));

  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByTestId("combo-project.save")).toHaveText(show("Mod+Shift+K"));
});

test("a shortcut that is already used asks which one to keep", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Change: Save project" }).click();
  await page.keyboard.press("ControlOrMeta+KeyO"); // already "Open project"
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("already used for “Open project”");

  await dialog.getByRole("button", { name: "Keep as it was" }).click();
  await expect(page.getByTestId("combo-project.save")).toHaveText(show("Mod+S"));
  await expect(page.getByTestId("combo-project.open")).toHaveText(show("Mod+O"));

  await page.getByRole("button", { name: "Change: Save project" }).click();
  await page.keyboard.press("ControlOrMeta+KeyO");
  await page.getByRole("alertdialog").getByRole("button", { name: "Use it here" }).click();
  await expect(page.getByTestId("combo-project.save")).toHaveText(show("Mod+O"));
  await expect(page.getByTestId("combo-project.open")).toHaveText("Not set");
});

test("Escape cancels recording a shortcut", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Change: New project" }).click();
  await expect(page.getByTestId("combo-project.new")).toContainText("Press the new shortcut");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("combo-project.new")).toHaveText(show("Mod+N"));
});

test("reset to defaults restores everything after confirming", async ({ page }) => {
  await start(page);
  await page.getByLabel("Theme").selectOption("light");
  await page.getByLabel("Share anonymous usage data").check();

  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Reset to defaults" }).click();
  await expect(page.getByLabel("Theme")).toHaveValue("system");
  await expect(page.getByLabel("Share anonymous usage data")).not.toBeChecked();
});

test("re-running the compatibility check shows the new mode", async ({ page }) => {
  await start(page);
  await page.evaluate(() => localStorage.setItem("fakeMode", "hybrid"));
  await page.getByRole("button", { name: "Re-run compatibility check" }).click();
  await expect(page.getByText("Result: Hybrid mode")).toBeVisible();
});

test("a machine that is no longer compatible is blocked after a re-check", async ({ page }) => {
  await start(page);
  await page.evaluate(() => localStorage.setItem("fakeMode", "blocked"));
  await page.getByRole("button", { name: "Re-run compatibility check" }).click();
  await expect(page.getByRole("alert")).toContainText("This computer cannot run FlickFluent");
});

test("a replaced settings file is explained once", async ({ page }) => {
  await page.addInitScript(installFakeCore);
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.clear();
    localStorage.setItem("fakeNotice", "settings.corrupted");
  });
  await page.reload();
  await expect(page.getByRole("status")).toContainText("could not be read");
  await page.reload();
  await expect(page.getByText("could not be read")).toHaveCount(0);
});

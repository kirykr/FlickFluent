import { expect, test, type Page } from "@playwright/test";

// The Tauri core is replaced by a stub that returns a report, so the screens can be tested in a
// plain browser on both platforms. The reports mirror core/tests/fixtures (low, mid, high).

const item = (key: string, status: string, neededLabel: string, foundLabel: string) => ({
  key,
  required: neededLabel,
  detected: foundLabel,
  status,
  neededLabel,
  foundLabel,
});

const low = {
  mode: "blocked",
  provisional: true,
  generatedAt: "2026-11-04T00:00:00Z",
  items: [],
  missing: [
    item("cpuCores", "belowMinimum", "4 CPU cores", "2"),
    item("ramGb", "belowMinimum", "8 GB RAM", "4 GB"),
    item("freeDiskGb", "belowMinimum", "10 GB free disk space", "5 GB"),
  ],
  blockedMessage:
    "Your computer is not compatible with this app. You need 4 CPU cores (found 2), 8 GB RAM (found 4 GB) and 10 GB free disk space (found 5 GB) to run this app.",
};
const mid = { ...low, mode: "hybrid", missing: [], blockedMessage: null, provisional: false };
const high = { ...mid, mode: "fullLocal" };

async function stubCore(page: Page, report: unknown) {
  await page.addInitScript((r) => {
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        if (cmd === "run_compat_check") return r;
        if (cmd === "project_recent") return []; // the home screen lists recent projects
        if (cmd === "models_catalogue") return []; // the models screen lists models
        return null;
      },
    };
  }, report);
}

test("blocked machine sees the exact message and cannot continue", async ({ page }) => {
  await stubCore(page, low);
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText(
    "Your computer is not compatible with this app. You need 4 CPU cores (found 2), 8 GB RAM (found 4 GB) and 10 GB free disk space (found 5 GB) to run this app.",
  );
  await expect(page.getByRole("row")).toHaveCount(4); // header + 3 missing items
  await expect(page.getByTestId("mode")).toHaveCount(0); // the app shell is not shown
});

test("hybrid machine sees the notice and can dismiss it", async ({ page }) => {
  await stubCore(page, mid);
  await page.goto("/");
  await expect(page.getByTestId("mode")).toHaveText("Hybrid mode");
  await expect(page.getByRole("status")).toContainText("optional cloud key");
  await page.getByRole("button", { name: "Got it" }).click();
  await expect(page.getByRole("status")).toHaveCount(0);
});

test("full local machine sees no notice", async ({ page }) => {
  await stubCore(page, high);
  await page.goto("/");
  await expect(page.getByTestId("mode")).toHaveText("Full local mode");
  await expect(page.getByRole("status")).toHaveCount(0);
});

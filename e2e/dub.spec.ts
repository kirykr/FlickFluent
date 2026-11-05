import { expect, test } from "@playwright/test";
import { chooseVideo, start } from "./support/fakeDubCore";

// The Tauri core is replaced by a fake (e2e/support/fakeDubCore.ts). The real pipeline,
// resume-after-kill and export are tested in core/tests/pipeline_resume.rs and
// core/tests/stage_mix_export.rs.

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

test("a run that only made subtitles offers only subtitles and explains a video export", async ({
  page,
}) => {
  await start(page);
  await page.evaluate(() => (window.__fakeDub.subtitlesOnly = true));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });

  // The only kind on offer is subtitles, shown by name.
  await expect(page.getByRole("combobox", { name: "Save as" })).toContainText("Subtitles (.srt)");
  await page.evaluate(() => (window.__fakeDub.nextOpen = "/exports"));
  await page.getByRole("button", { name: "Choose a folder…" }).click();
  await page.getByTestId("dub-result").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("export-done")).toContainText("holiday.srt");
});

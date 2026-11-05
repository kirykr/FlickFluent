import { expect, test } from "@playwright/test";
import { chooseVideo, start } from "./support/fakeDubCore";

// Progress, errors and cancel as the user sees them. The real pacing, ETA and error cases are
// in core/tests/progress.rs and core/tests/errors.rs; the page here follows a fake core.

async function begin(page: Parameters<typeof start>[0]) {
  await start(page);
  await page.evaluate(() => (window.__fakeDub.slow = true));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("dub-progress")).toBeVisible();
}

test("updates arrive: stage, percentage, time left and the lines as they are written", async ({
  page,
}) => {
  await begin(page);
  const progress = page.getByTestId("dub-progress");
  await expect(progress).toContainText("Part 1 of 1");
  await expect(progress).toContainText("min left");
  await expect(page.getByTestId("stage-percent")).toContainText("50%");
  await expect(page.getByLabel("Overall progress")).toHaveAttribute("aria-valuenow", /\d+/);
  await expect(page.getByTestId("live-line").first()).toContainText("Hello everyone.", {
    timeout: 10_000,
  });
  await expect(page.locator('[data-stage="extract"]')).toHaveAttribute("data-status", "done");
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
});

test("a video that disappears mid-run gives a plain message and a next step", async ({ page }) => {
  await start(page);
  await page.evaluate(() => {
    window.__fakeDub.slow = true;
    window.__fakeDub.failAtStep = 3;
    window.__fakeDub.failure = {
      code: "STAGE_FAILED",
      stage: "translate",
      messageKey: "dub.error.sourceMissing",
      nextStepKey: "dub.next.chooseAnotherFile",
      details: { stage: "translate", reason: "sourceMissing" },
    };
  });
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();

  const error = page.getByTestId("dub-run-error");
  await expect(error).toContainText("gone or cannot be reached", { timeout: 10_000 });
  await expect(error).toContainText("Translating");
  await expect(error).toContainText("Choose another file.");
  await expect(page.locator('[data-stage="translate"]')).toHaveAttribute("data-status", "failed");
  // Work finished before the failure stays on screen.
  await expect(page.locator('[data-stage="extract"]')).toHaveAttribute("data-status", "done");

  // The file may come back, so trying again is offered next to choosing another one.
  await expect(error.getByRole("button", { name: "Try again" })).toBeVisible();
  await error.getByRole("button", { name: "Choose another file" }).click();
  await expect(page.getByRole("button", { name: "Change video…" })).toBeVisible();
});

test("a full disk says how much is needed and free, and the run can go on after", async ({
  page,
}) => {
  await start(page);
  await page.evaluate(() => {
    window.__fakeDub.slow = true;
    window.__fakeDub.failAtStep = 2;
    window.__fakeDub.failure = {
      code: "NO_SPACE",
      stage: "transcribe",
      messageKey: "dub.error.noSpace",
      nextStepKey: "dub.next.freeDiskSpace",
      details: {
        neededBytes: 3 * 1024 ** 3,
        freeBytes: 1024 ** 3,
        shortfallBytes: 2 * 1024 ** 3,
      },
    };
  });
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();

  const error = page.getByTestId("dub-run-error");
  await expect(error).toContainText("3.0 GB", { timeout: 10_000 });
  await expect(error).toContainText("1.0 GB");
  await expect(error).toContainText("2.0 GB");
  await error.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
});

test("cancel keeps finished work and the run can be resumed", async ({ page }) => {
  await begin(page);
  await expect(page.locator('[data-stage="extract"]')).toHaveAttribute("data-status", "done");
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.getByTestId("dub-progress")).toContainText("Stopped");
  await expect(page.locator('[data-stage="extract"]')).toHaveAttribute("data-status", "done");
  await page.getByRole("button", { name: "Resume" }).click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
});

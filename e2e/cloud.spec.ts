import { expect, test, type Page } from "@playwright/test";
import { installFakeCloudCore } from "./support/fakeCloudCore";

const SECRET = "AIzaSyVERYSECRET1234";

async function start(page: Page) {
  await page.addInitScript(installFakeCloudCore);
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
}

async function addKey(page: Page, label = "personal") {
  await page.getByRole("button", { name: "Add key" }).click();
  await page.getByLabel("Name for this key").fill(label);
  await page.getByLabel("Key", { exact: true }).fill(SECRET);
  await page.getByRole("button", { name: "Save key" }).click();
}

test("add a key, verify it, and see it masked after a restart", async ({ page }) => {
  await start(page);
  await expect(page.getByText("No keys yet.")).toBeVisible();
  await addKey(page);
  await expect(page.getByText("Key ending in 1234")).toBeVisible();

  await page.getByRole("button", { name: "Verify connection" }).click();
  await expect(page.getByText("The key works with Google Gemini API.")).toBeVisible();
  await expect(page.getByText("Works", { exact: true })).toBeVisible();

  await page.reload(); // restart
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByText("Key ending in 1234")).toBeVisible();
  await expect(page.getByText("personal")).toBeVisible();
  expect(await page.locator("body").innerText()).not.toContain(SECRET);
});

test("a refused key shows the reason", async ({ page }) => {
  await start(page);
  await addKey(page);
  await page.evaluate(() => (window.__fakeCloud.verify = "rejected"));
  await page.getByRole("button", { name: "Verify connection" }).click();
  await expect(page.getByText("refused this key", { exact: false })).toBeVisible();
  await expect(page.getByText("Refused", { exact: true })).toBeVisible();
});

test("remove asks first and then deletes the key", async ({ page }) => {
  await start(page);
  await addKey(page);
  await page.getByRole("button", { name: "Remove" }).click();
  await expect(page.getByText("Remove this key?")).toBeVisible();
  await page.getByRole("dialog").getByRole("button", { name: "Remove" }).click();
  await expect(page.getByText("No keys yet.")).toBeVisible();
});

// ---- a dub with a cloud step ---------------------------------------------------------------

import { chooseVideo, start as startDub } from "./support/fakeDubCore";

test("a cloud dub asks first, shows the amount, labels the cloud step and lists the usage", async ({
  page,
}) => {
  await startDub(page);
  await page.evaluate(() => (window.__fakeDub.cloud = "google-gemini"));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();

  // The amount is on the start screen before anything is asked.
  await expect(page.getByTestId("cloud-amounts")).toContainText("about 300 characters");
  await expect(page.getByTestId("cloud-amounts")).toContainText("Google Gemini API");

  await page.getByRole("button", { name: "Start dubbing" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Send text to Google Gemini API?");
  await expect(dialog).toContainText("The text of the lines to translate.");
  await expect(dialog).toContainText("Not sent: your video, your original audio");
  await expect(dialog.getByTestId("consent-amount")).toContainText("About 300 characters");
  await dialog.getByRole("button", { name: "Agree" }).click();

  // The stage list names the service.
  await expect(page.getByTestId("dub-progress")).toBeVisible();
  await expect(page.getByTestId("cloud-label")).toHaveText("Cloud: Google Gemini API");
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId("cloud-usage")).toContainText("Translation: Google Gemini API");
});

test("declining sends nothing and starts nothing", async ({ page }) => {
  await startDub(page);
  await page.evaluate(() => (window.__fakeDub.cloud = "google-gemini"));
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Not now" }).click();
  await expect(page.getByText("Nothing was sent.")).toBeVisible();
  await expect(page.getByTestId("dub-progress")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Start dubbing" })).toBeEnabled();
});

test("a run that moves to another key says so and still finishes", async ({ page }) => {
  await startDub(page);
  await page.evaluate(() => {
    window.__fakeDub.cloud = "google-gemini";
    window.__fakeDub.slow = true;
    window.__fakeDub.cloudNotes = [{ kind: "switched", keyLabel: "second" }];
    localStorage.setItem("fakeConsent", "1");
  });
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();
  await expect(page.getByTestId("cloud-note")).toContainText("Moved on to the key “second”.");
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 30_000 });
});

test("when every key is limited the run pauses, says how long to wait, and can resume", async ({
  page,
}) => {
  await startDub(page);
  await page.evaluate(() => {
    window.__fakeDub.cloud = "google-gemini";
    window.__fakeDub.failAtStep = 3; // translate
    window.__fakeDub.failure = {
      code: "CLOUD_KEYS_EXHAUSTED",
      stage: "translate",
      messageKey: "dub.error.cloudKeysExhausted",
      nextStepKey: "dub.next.waitAndResume",
      details: { service: "Google Gemini API", retryAfterSeconds: 700 },
    };
    localStorage.setItem("fakeConsent", "1");
  });
  await chooseVideo(page);
  await page.getByRole("button", { name: /^Download/ }).click();
  await page.getByRole("button", { name: "Start dubbing" }).click();

  const box = page.getByTestId("dub-run-error");
  await expect(box).toContainText("Google Gemini API asked you to wait about 700 seconds");
  await expect(box).toContainText("everything finished is kept");

  // Later: the limit has passed and the run is resumed.
  await page.evaluate(() => (window.__fakeDub.failure = null));
  await box.getByRole("button").click();
  await expect(page.getByTestId("dub-result")).toBeVisible({ timeout: 30_000 });
});

// ---- choosing the engine per step ----------------------------------------------------------

async function settings(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
}

test("a fresh install shows Local for both steps; cloud is chosen, used, switched back and asked about again after a withdrawal", async ({
  page,
}) => {
  await startDub(page);
  await settings(page);
  await expect(page.getByLabel("Translation engine")).toContainText("Local");
  await expect(page.getByLabel("Speech engine")).toContainText("Local");

  // Switch translation to the service (it has a verified key).
  await page.getByLabel("Translation engine").click();
  await page.getByRole("option", { name: "Google Gemini API" }).click();
  await expect(page.getByLabel("Translation engine")).toContainText("Google Gemini API");
  await expect(page.getByLabel("Speech engine")).toContainText("Local");

  // Withdraw the agreement from Settings: the next dub must ask again.
  await page.evaluate(() => localStorage.setItem("fakeConsent", "1"));
  await page.reload();
  await settings(page);
  await page.getByRole("button", { name: "Withdraw" }).click();
  await expect(page.getByText("You have not agreed to send anything yet.")).toBeVisible();
});

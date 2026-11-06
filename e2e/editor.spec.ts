import { expect, test } from "@playwright/test";
import { start } from "./support/fakeEditorCore";

// The Tauri core is replaced by a fake (e2e/support/fakeEditorCore.ts). The real edit rules,
// re-synthesis and splice are tested in core/tests/editor_*.rs.

const cell = (page: import("@playwright/test").Page, n: number) =>
  page.getByLabel(`Translation of line ${n}`, { exact: true });

test("editing a cell marks the line changed, and undo clears it", async ({ page }) => {
  await start(page);
  await expect(page.getByTestId("changed-badge")).toHaveCount(0);
  await cell(page, 2).fill("A new sentence");
  await cell(page, 2).blur();
  await expect(page.getByTestId("changed-badge")).toHaveCount(1);
  await expect(page.getByTestId("synthesize")).toContainText("(1)");

  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByTestId("changed-badge")).toHaveCount(0);
  await expect(cell(page, 2)).not.toHaveValue("A new sentence");
  await page.getByRole("button", { name: "Redo" }).click();
  await expect(page.getByTestId("changed-badge")).toHaveCount(1);
});

test("a time that is not a time goes back, and an end before the start is refused", async ({
  page,
}) => {
  await start(page);
  const end = page.getByLabel("End of line 2", { exact: true });
  const before = await end.inputValue();
  await end.fill("nonsense");
  await end.blur();
  await expect(end).toHaveValue(before);
  await end.fill("0:01.000");
  await end.blur();
  await expect(page.getByTestId("editor-error")).toContainText("end of a line must be after");
  await expect(end).toHaveValue(before);
});

test("search and replace changes only the chosen matches", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Search & Replace" }).click();
  await page.getByLabel("Find").fill("hello");
  await expect(page.getByTestId("search-count")).toContainText("3 matches");
  await page.getByLabel("Replace with").fill("Hi");
  await page.getByTestId("search-match").nth(1).getByRole("checkbox").click();
  await page.getByRole("button", { name: /Replace selected \(2\)/ }).click();
  await expect(cell(page, 1)).toHaveValue("Hi 0");
  await expect(cell(page, 6)).toHaveValue("Hello 5");
  await expect(page.getByTestId("synthesize")).toContainText("(2)");
});

test("insert and delete change the table, and undo brings the line back", async ({ page }) => {
  await start(page, 4);
  await expect(page.getByTestId("editor-row")).toHaveCount(4);
  await page.getByRole("button", { name: "Insert", exact: true }).click();
  await expect(page.getByTestId("editor-row")).toHaveCount(5);

  await page.getByLabel("Select line 1", { exact: true }).click();
  await page.getByRole("button", { name: /Delete selected \(1\)/ }).click();
  await expect(page.getByTestId("editor-row")).toHaveCount(4);
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByTestId("editor-row")).toHaveCount(5);
});

test("making speech clears the marks one line at a time, and Stop leaves the rest", async ({
  page,
}) => {
  await start(page);
  await page.evaluate(() => (window.__fakeEditor.lineMs = 1200));
  for (const n of [1, 2, 3, 4]) {
    await cell(page, n).fill(`Edited ${n}`);
    await cell(page, n).blur();
  }
  await expect(page.getByTestId("synthesize")).toContainText("(4)");
  await page.getByTestId("synthesize").click();
  await expect(page.getByTestId("editor-run")).toBeVisible();
  await expect(page.getByTestId("changed-badge")).toHaveCount(3, { timeout: 8000 });
  await page.getByTestId("editor-stop").click();
  await expect(page.getByTestId("editor-left")).toContainText("lines are still to make");
  await expect(page.getByTestId("synthesize")).toBeEnabled();

  await page.evaluate(() => (window.__fakeEditor.lineMs = 20));
  await page.getByTestId("synthesize").click();
  await expect(page.getByTestId("changed-badge")).toHaveCount(0, { timeout: 5000 });
  await expect(page.getByTestId("synthesize")).toContainText("Nothing to make");
});

test("a table of 6,000 lines draws a few rows and still scrolls to the end", async ({ page }) => {
  await start(page, 6000);
  const rows = await page.getByTestId("editor-row").count();
  expect(rows).toBeLessThan(60);
  await page.getByTestId("editor-table").evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(cell(page, 6000)).toBeVisible({ timeout: 5000 });
});

test("a new dub asks before it replaces edited lines", async ({ page }) => {
  await start(page, 4);
  await page.evaluate(() => (window.__fakeEditor.editsWillBeLost = 2));
  await page.getByTestId("dub-toggle").click();
  await page.getByRole("button", { name: "Choose a video…" }).click();
  const go = page.getByRole("button", { name: "Start dubbing" });
  await expect(go).toBeEnabled();
  await go.click();
  await expect(page.getByTestId("discard-edits")).toContainText("replace 2 lines you edited");
  await page.getByRole("button", { name: "Keep my edits" }).click();
  expect(await page.evaluate(() => window.__fakeEditor.lastStart)).toBeNull();

  await go.click();
  await page.getByRole("button", { name: "Start a new dub" }).click();
  await expect
    .poll(() => page.evaluate(() => window.__fakeEditor.lastStart?.discardEdits))
    .toBe(true);
});

test("Smart Fit fits what it can and says why it cannot fit the rest", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Smart Fit" }).click();
  await expect(page.getByTestId("fit-summary")).toContainText("1 fitted, 1 still flagged");
  await expect(page.getByTestId("fit-summary")).toContainText("Line 4:");
  await expect(page.getByTestId("fit-summary")).toContainText("safe speed-up");
  const flags = page.getByTestId("editor-row").nth(1);
  await expect(flags).not.toContainText("Too long");
  await expect(page.getByTestId("editor-row").nth(3)).toContainText("Too long");
});

test("End Sync moves a clip earlier in the timeline", async ({ page }) => {
  await start(page);
  const clip = page.locator('[data-line="l2"]');
  const before = Number(await clip.getAttribute("data-left"));
  await page.getByLabel("Select line 3", { exact: true }).click();
  await page.getByRole("button", { name: "End Sync" }).click();
  await expect(clip).toHaveAttribute("data-left", String(before - 300));
});

test("a slider change marks only that line as needing a re-make", async ({ page }) => {
  await start(page);
  await page.locator('[data-line="l4"]').click();
  await page.getByTestId("fit-controls").getByRole("slider").first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByTestId("changed-badge")).toHaveCount(1);
  await expect(page.getByTestId("editor-row").nth(4)).toContainText("Re-make");
  await expect(page.getByTestId("synthesize")).toContainText("(1)");
});

test("export offers the sources and formats, and says SRT has no style", async ({ page }) => {
  await start(page);
  await page.getByRole("button", { name: "Export Subtitles" }).click();
  const dialog = page.getByTestId("subtitles-dialog");
  await expect(dialog.getByTestId("format-note")).toContainText("SRT has no style");
  await dialog.getByRole("combobox", { name: "Format" }).click();
  await page.getByRole("option", { name: "WebVTT" }).click();
  await expect(dialog.getByTestId("format-note")).toContainText("caption template is saved");
  await dialog.getByRole("button", { name: "Save…" }).click();
  await expect(dialog.getByTestId("export-count")).toContainText("Saved 12 subtitles");
  const sent = await page.evaluate(() => window.__fakeEditor.exported);
  expect(sent).toEqual([{ path: "/demo/e.flick", format: "vtt", source: "translated" }]);
});

test("an import shows how the file compares, and nothing changes until it is applied", async ({
  page,
}) => {
  await start(page);
  await page.evaluate(() => {
    window.__fakeEditor.cues = 30;
    window.__fakeEditor.nextOpen = "/subs/film.srt";
  });
  await page.getByRole("button", { name: "Import Subtitles" }).click();
  const dialog = page.getByTestId("subtitles-dialog");
  await dialog.getByRole("button", { name: "Choose a subtitle file…" }).click();
  const preview = dialog.getByTestId("import-preview");
  await expect(preview).toContainText("30 subtitles in the file, 12 lines");
  await expect(preview).toContainText("windows-1252");
  await expect(preview).toContainText("Subtitle 13 has no line");
  // The count differs, so only two of the three choices are offered.
  await expect(preview.getByText("Replace the text of the lines")).toHaveCount(0);
  await expect(cell(page, 1)).toHaveValue("Hello 0");

  await preview.getByText("Fill each line from the subtitle that overlaps it").click();
  await preview.getByRole("button", { name: "Apply" }).click();
  await expect(dialog.getByTestId("import-applied")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(cell(page, 1)).toHaveValue("Imported (fillByOverlap) l0");
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(cell(page, 1)).toHaveValue("Hello 0");
});

test("a bad subtitle file shows the first problem and changes nothing", async ({ page }) => {
  await start(page);
  await page.evaluate(() => (window.__fakeEditor.nextOpen = "/subs/bad.srt"));
  await page.getByRole("button", { name: "Import Subtitles" }).click();
  const dialog = page.getByTestId("subtitles-dialog");
  await dialog.getByRole("button", { name: "Choose a subtitle file…" }).click();
  await expect(dialog.getByTestId("subtitles-error")).toContainText("cue 2 (line 6)");
  await expect(dialog.getByTestId("import-preview")).toHaveCount(0);
});

test("changing the caption template changes the preview, Khmer included", async ({ page }) => {
  await start(page);
  await page.getByLabel("Select line 5", { exact: true }).click();
  const caption = page.getByTestId("caption-preview");
  await expect(caption).toContainText("សួស្តី ពិភពលោក");
  for (const [name, id] of [
    ["Classic", "classic"],
    ["Soft box", "soft-box"],
    ["Top note", "top-note"],
  ]) {
    await page.getByRole("combobox", { name: "Caption template" }).click();
    await page.getByRole("option", { name }).click();
    await expect(caption).toHaveAttribute("data-template", id);
    await expect(caption).toContainText("សួស្តី ពិភពលោក");
    const font = await caption.locator("span").evaluate((el) => getComputedStyle(el).fontFamily);
    expect(font).toContain("Noto Sans Khmer");
    expect(font).toContain("Khmer Sangam MN");
  }
  const style = await caption.locator("span").evaluate((el) => getComputedStyle(el).color);
  expect(style).toBe("rgb(255, 216, 74)");
});

const rows = (page: import("@playwright/test").Page) => page.getByTestId("editor-row");

test("split cuts a line at the cursor, merge joins neighbours of one speaker, duplicate copies", async ({
  page,
}) => {
  await start(page, 6);
  // Split line 1 ("Hello 0") after "Hello".
  await cell(page, 1).focus();
  await cell(page, 1).evaluate((el: HTMLTextAreaElement) => el.setSelectionRange(5, 5));
  await page.getByRole("button", { name: "Actions for line 1" }).click();
  await page.getByRole("menuitem", { name: "Split at the cursor" }).last().click();
  await expect(rows(page)).toHaveCount(7);
  await expect(cell(page, 1)).toHaveValue("Hello");
  await expect(cell(page, 2)).toHaveValue("0");

  // Lines 2 and 3 are by different speakers: merging is refused.
  await page.getByRole("button", { name: "Actions for line 2" }).click();
  await expect(
    page.getByRole("menuitem", { name: "Merge with the next line" }).last(),
  ).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");
  // Line 1 and its second half have the same speaker.
  await page.getByRole("button", { name: "Actions for line 1" }).click();
  await page.getByRole("menuitem", { name: "Merge with the next line" }).last().click();
  await expect(rows(page)).toHaveCount(6);
  await expect(cell(page, 1)).toHaveValue("Hello 0");

  await page.getByRole("button", { name: "Actions for line 3" }).click();
  await page.getByRole("menuitem", { name: "Duplicate" }).last().click();
  await expect(rows(page)).toHaveCount(7);
  await expect(cell(page, 4)).toHaveValue(await cell(page, 3).inputValue());
});

test("the bulk dialog counts what would change before it changes anything", async ({ page }) => {
  await start(page, 4);
  await page.getByRole("button", { name: "Edit all text" }).click();
  const dialog = page.getByTestId("bulk-dialog");
  await expect(dialog.getByTestId("bulk-count")).toContainText("0 lines will change");
  const box = dialog.getByRole("textbox");
  await expect(box).toHaveValue(/l0 \| Hello 0/);
  const text = await box.inputValue();
  await box.fill(text.replace("Hello 0", "Hi 0").replace("Good morning 2", "Morning 2"));
  await expect(dialog.getByTestId("bulk-count")).toContainText("2 lines will change");
  await expect(cell(page, 1)).toHaveValue("Hello 0");
  await dialog.getByRole("button", { name: "Apply 2 changes" }).click();
  await expect(cell(page, 1)).toHaveValue("Hi 0");
  await expect(cell(page, 3)).toHaveValue("Morning 2");
  await expect(page.getByTestId("synthesize")).toContainText("(2)");
});

test("changing a speaker's gender marks that speaker's lines", async ({ page }) => {
  await start(page, 6);
  await expect(page.getByTestId("synthesize")).toContainText("Nothing to make");
  await page.getByRole("button", { name: "spk1 Female" }).click();
  await expect(page.getByTestId("speaker-note")).toContainText(
    "3 lines of spk1 now need speech again",
  );
  await expect(page.getByTestId("synthesize")).toContainText("(3)");
  await expect(page.locator('[data-speaker="spk1"]')).toContainText("set by you");
});

test("the memory indicator in the editor follows the core's samples", async ({ page }) => {
  await start(page);
  await page.evaluate(() => window.__fakeEditor.memory(40));
  const badge = page.getByTestId("editor-memory");
  await expect(badge).toContainText("40%");
  await page.evaluate(() => window.__fakeEditor.memory(90));
  await expect(badge).toHaveAttribute("data-level", "warning");
});

test("SC-005: on 6,000 lines the matches show in under 2 seconds", async ({ page }) => {
  await start(page, 6000);
  await page.getByRole("button", { name: "Search & Replace" }).click();
  const started = Date.now();
  await page.getByLabel("Find").fill("hello");
  await expect(page.getByTestId("search-count")).toContainText("matches", { timeout: 2000 });
  const took = Date.now() - started;
  console.log(`matches shown after ${took} ms`);
  expect(took).toBeLessThan(2000);
});

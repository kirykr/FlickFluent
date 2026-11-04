import { expect, test, type Page } from "@playwright/test";

// The Tauri core is replaced by a small fake that keeps a "disk" in localStorage. Reloading the
// page stands in for killing and relaunching the app: open sessions are lost, the disk stays, and
// the fake autosaves unsaved notes to a recovery entry just as the real core does every 30 s.
// The real file handling is covered by the Rust tests in core/tests/project_*.rs.

type FakeFile = {
  name: string;
  notes: string;
  damaged?: boolean;
  tooNew?: boolean;
  backupNotes?: string;
  lockedElsewhere?: boolean;
};
type FakeDisk = {
  files: Record<string, FakeFile>;
  recovery: Record<string, string>;
  recent: string[];
};

declare global {
  interface Window {
    __fake: {
      nextSave: string | null;
      nextOpen: string | null;
      seed: (p: string, f: FakeFile) => void;
    };
  }
}

function installFakeCore() {
  const load = (): FakeDisk =>
    JSON.parse(localStorage.getItem("fakeDisk") ?? '{"files":{},"recovery":{},"recent":[]}');
  const store = (d: FakeDisk) => localStorage.setItem("fakeDisk", JSON.stringify(d));
  const sessions: Record<string, { path: string; notes: string; dirty: boolean }> = {};
  let n = 0;

  const handle = (id: string) => {
    const s = sessions[id];
    const f = load().files[s.path];
    return {
      id,
      name: f.name,
      path: s.path,
      readOnly: !!f.lockedElsewhere,
      dirty: s.dirty,
      notes: s.notes,
    };
  };
  const open = (path: string, notes: string, dirty: boolean) => {
    const id = `p${++n}`;
    sessions[id] = { path, notes, dirty };
    const d = load();
    d.recent = [path, ...d.recent.filter((r) => r !== path)];
    store(d);
    return id;
  };

  const fake = {
    nextSave: null as string | null,
    nextOpen: null as string | null,
    seed: (p: string, f: FakeFile) => {
      const d = load();
      d.files[p] = f;
      store(d);
    },
  };
  (window as unknown as { __fake: typeof fake }).__fake = fake;

  const commands: Record<string, (a: Record<string, string>) => unknown> = {
    run_compat_check: () => ({
      mode: "fullLocal",
      items: [],
      missing: [],
      blockedMessage: null,
      provisional: false,
      generatedAt: "",
    }),
    "plugin:dialog|save": () => fake.nextSave,
    "plugin:dialog|open": () => fake.nextOpen,
    project_suggest_file_name: (a) => `${a.name}.flick`,
    project_new: (a) => {
      const d = load();
      d.files[a.path] = { name: a.name, notes: "" };
      store(d);
      return handle(open(a.path, "", false));
    },
    project_open: (a) => {
      const f = load().files[a.path];
      if (!f) throw { code: "IO_ERROR" };
      if (f.tooNew) throw { code: "FORMAT_TOO_NEW" };
      if (f.damaged)
        throw { code: "DAMAGED", details: { backupAvailable: f.backupNotes !== undefined } };
      const recovery = a.path in load().recovery;
      return { handle: handle(open(a.path, f.notes, false)), recoveryAvailable: recovery };
    },
    project_recover: (a) => {
      const d = load();
      const f = d.files[a.path];
      const notes = a.path in d.recovery ? d.recovery[a.path] : (f.backupNotes ?? f.notes);
      return handle(open(a.path, notes, true));
    },
    project_discard_recovery: (a) => {
      const d = load();
      delete d.recovery[a.path];
      store(d);
    },
    project_set_notes: (a) => {
      const s = sessions[a.handle];
      s.notes = a.notes;
      s.dirty = true;
      const d = load();
      d.recovery[s.path] = a.notes; // the 30 second autosave, made immediate
      store(d);
      return handle(a.handle);
    },
    project_save: (a) => {
      const s = sessions[a.handle];
      const d = load();
      d.files[s.path].notes = s.notes;
      delete d.recovery[s.path];
      store(d);
      s.dirty = false;
      return handle(a.handle);
    },
    project_close: (a) => {
      const s = sessions[a.handle];
      const d = load();
      delete d.recovery[s.path];
      store(d);
      delete sessions[a.handle];
    },
    project_recent: () => {
      const d = load();
      return d.recent
        .filter((p) => d.files[p])
        .map((p) => ({ path: p, name: d.files[p].name, openedAt: "", available: true }));
    },
    project_delete: (a) => {
      const d = load();
      delete d.files[a.path];
      d.recent = d.recent.filter((r) => r !== a.path);
      store(d);
    },
  };

  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: Record<string, string>) => {
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
}

async function createProject(page: Page, name: string, path: string) {
  await page.evaluate((p) => (window.__fake.nextSave = p), path);
  await page.getByLabel("Project name").fill(name);
  await page.getByRole("button", { name: "Create…" }).click();
  await expect(page.getByRole("heading", { name })).toBeVisible();
}

test("create, save, close and reopen keeps the content", async ({ page }) => {
  await start(page);
  await createProject(page, "សាកល្បង Project", "/demo/test.flick");

  await page.getByLabel("Notes").fill("សួស្តី hello");
  await expect(page.getByTestId("save-state")).toHaveText("Unsaved changes");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByTestId("save-state")).toHaveText("All changes saved");

  await page.getByRole("button", { name: "Close project" }).click();
  await page.getByRole("button", { name: "សាកល្បង Project" }).click();
  await expect(page.getByLabel("Notes")).toHaveValue("សួស្តី hello");
});

test("after the app is killed the unsaved work can be restored", async ({ page }) => {
  await start(page);
  await createProject(page, "Crash test", "/demo/crash.flick");
  await page.getByLabel("Notes").fill("typed but never saved");

  await page.reload(); // kill and relaunch
  await page.getByRole("button", { name: "Crash test" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("Unsaved work found");
  await page.getByRole("button", { name: "Restore" }).click();
  await expect(page.getByLabel("Notes")).toHaveValue("typed but never saved");
  await expect(page.getByTestId("save-state")).toHaveText("Unsaved changes");
});

test("discarding the recovery keeps the last saved version", async ({ page }) => {
  await start(page);
  await createProject(page, "Discard test", "/demo/discard.flick");
  await page.getByLabel("Notes").fill("kept");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await page.getByLabel("Notes").fill("lost");

  await page.reload();
  await page.getByRole("button", { name: "Discard test" }).click();
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(page.getByLabel("Notes")).toHaveValue("kept");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

test("a project from a newer version is refused with a clear message", async ({ page }) => {
  await start(page);
  await page.evaluate(() =>
    window.__fake.seed("/demo/new.flick", { name: "New", notes: "", tooNew: true }),
  );
  await page.evaluate(() => (window.__fake.nextOpen = "/demo/new.flick"));
  await page.getByRole("button", { name: "Open…" }).click();
  await expect(page.getByRole("alert")).toContainText("newer version of FlickFluent");
});

test("a damaged project with a backup can be restored from the backup", async ({ page }) => {
  await start(page);
  await page.evaluate(() =>
    window.__fake.seed("/demo/bad.flick", {
      name: "Bad",
      notes: "",
      damaged: true,
      backupNotes: "from backup",
    }),
  );
  await page.evaluate(() => (window.__fake.nextOpen = "/demo/bad.flick"));
  await page.getByRole("button", { name: "Open…" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("damaged");
  await page.getByRole("button", { name: "Restore from backup" }).click();
  await expect(page.getByLabel("Notes")).toHaveValue("from backup");
});

test("a project open elsewhere is read-only", async ({ page }) => {
  await start(page);
  await page.evaluate(() =>
    window.__fake.seed("/demo/locked.flick", {
      name: "Locked",
      notes: "look only",
      lockedElsewhere: true,
    }),
  );
  await page.evaluate(() => (window.__fake.nextOpen = "/demo/locked.flick"));
  await page.getByRole("button", { name: "Open…" }).click();
  await expect(page.getByText("open in another window")).toBeVisible();
  await expect(page.getByLabel("Notes")).toBeDisabled();
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
});

test("a project can be deleted from the recent list after confirming", async ({ page }) => {
  await start(page);
  await createProject(page, "Delete me", "/demo/delete.flick");
  await page.getByRole("button", { name: "Close project" }).click();

  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByText("No recent projects yet.")).toBeVisible();
});

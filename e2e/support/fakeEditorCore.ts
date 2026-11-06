import { expect, type Page } from "@playwright/test";

// The Tauri core is replaced by a fake editor: the same commands and `editor:event` stream as
// the real one, kept in the page. The real rules (changed marks, undo, search, re-synthesis,
// splice) are tested in core/tests/editor_*.rs.

declare global {
  interface Window {
    __fakeEditor: {
      /** Lines the finished dub has. Set before the page loads, through `start`. */
      count: number;
      /** Edited lines a new dub would replace. */
      editsWillBeLost: number;
      /** Arguments of the last `dub_start`. */
      lastStart: Record<string, unknown> | null;
      /** Wait this long per line when making speech. */
      lineMs: number;
      /** What the open dialog returns next (a subtitle file path). */
      nextOpen: string | null;
      /** Files "written" by subtitles_export: path and format. */
      exported: { path: string; format: string; source: string }[];
      /** What the next imported file looks like to the preview. */
      cues: number;
      /** Send a `memory:sample` with this share of the computer's memory in use. */
      memory: (usedPercent: number) => void;
    };
  }
}

export function installFakeCore(count: number) {
  const fake = {
    count,
    editsWillBeLost: 0,
    lastStart: null as Record<string, unknown> | null,
    lineMs: 150,
    nextOpen: null as string | null,
    exported: [] as { path: string; format: string; source: string }[],
    cues: 4,
    memory: (usedPercent: number) =>
      emit("memory:sample", {
        appBytes: 400 * 1024 * 1024,
        systemUsedBytes: (16 * 1024 ** 3 * usedPercent) / 100,
        systemTotalBytes: 16 * 1024 ** 3,
        level: usedPercent >= 85 ? "warning" : "normal",
      }),
  };
  (window as unknown as { __fakeEditor: typeof fake }).__fakeEditor = fake;
  type Line = {
    id: string;
    sectionIndex: number;
    startMs: number;
    endMs: number;
    speakerId: string;
    language: string;
    text: string;
    translated: string | null;
    audio: string | null;
    audioMs: number | null;
    flags: string[];
    status: string;
    speed: number;
    dubStartMs: number | null;
  };
  const words = ["Hello", "Thank you", "Good morning", "See you soon", "Welcome"];
  let lines: Line[] = Array.from({ length: fake.count }, (_, i) => ({
    id: `l${i}`,
    sectionIndex: 0,
    startMs: i * 3000,
    endMs: i * 3000 + 2000,
    speakerId: i % 2 ? "spk2" : "spk1",
    language: "fr",
    text: `Ligne ${i}`,
    translated: i === 4 ? "សួស្តី ពិភពលោក" : `${words[i % words.length]} ${i}`,
    audio: `clips/l${i}.wav`,
    audioMs: 1500,
    flags: i === 1 || i === 3 ? ["tooLong"] : [],
    status: "synthesized",
    speed: 1,
    dubStartMs: null,
  }));
  let changed = new Set<string>();
  let needsPost = new Set<string>();
  const undo: { lines: Line[]; changed: string[]; post: string[] }[] = [];
  let redo: typeof undo = [];
  let canceled = false;
  let template = "classic";
  const genders: Record<string, string> = { spk1: "male", spk2: "female" };
  let runId = 0;

  const snap = () => ({
    lines: JSON.parse(JSON.stringify(lines)) as Line[],
    changed: [...changed],
    post: [...needsPost],
  });
  const restore = (s: ReturnType<typeof snap>) => {
    lines = s.lines;
    changed = new Set(s.changed);
    needsPost = new Set(s.post);
  };
  const result = (before: Line[], extra: Partial<{ removedIds: string[] }> = {}) => {
    const old = new Map(before.map((l) => [l.id, JSON.stringify(l)]));
    const now = new Set(lines.map((l) => l.id));
    return {
      lines: lines.filter((l) => old.get(l.id) !== JSON.stringify(l)),
      removedIds: before.filter((l) => !now.has(l.id)).map((l) => l.id),
      changedIds: [...changed],
      needsPostIds: [...needsPost],
      canUndo: undo.length > 0,
      canRedo: redo.length > 0,
      ...extra,
    };
  };
  const edit = (fn: () => void) => {
    const before = snap();
    fn();
    undo.push(before);
    redo = [];
    return result(before.lines);
  };
  const state = () => ({
    lines,
    speakers: [
      {
        id: "spk1",
        gender: genders.spk1,
        voiceId: "",
        confidence: 1,
        genderSource: genders.spk1 === "male" ? "detected" : "user",
      },
      { id: "spk2", gender: genders.spk2, voiceId: "", confidence: 1, genderSource: "detected" },
    ],
    changedIds: [...changed],
    needsPostIds: [...needsPost],
    canUndo: undo.length > 0,
    canRedo: redo.length > 0,
    captionTemplate: template,
  });

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
    name: "Edit test",
    path: "/demo/e.flick",
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
    project_recent: () => [],
    project_suggest_file_name: (a) => `${a.name}.flick`,
    project_new: () => handle,
    project_open: () => ({ handle, recoveryAvailable: false }),
    "plugin:dialog|save": () => "/demo/e.flick",
    "plugin:dialog|open": () => fake.nextOpen ?? "/videos/h.mp4",
    "plugin:event|listen": (a) => {
      (handlers[a.event as string] ??= []).push(a.handler as number);
      return a.handler;
    },
    "plugin:event|unlisten": () => undefined,
    editor_open: () => state(),
    editor_state: () => state(),
    editor_update_line: (a) => {
      const e = a.edit as Record<string, unknown>;
      const line = lines.find((l) => l.id === e.id)!;
      const start = (e.startMs as number) ?? line.startMs;
      const end = (e.endMs as number) ?? line.endMs;
      if (end <= start)
        throw { code: "EDIT_END_BEFORE_START", messageKey: "editor.error.endBeforeStart" };
      return edit(() => {
        if (e.translated !== undefined) {
          line.translated = e.translated as string;
          changed.add(line.id);
        }
        if (e.speakerId !== undefined) {
          line.speakerId = e.speakerId as string;
          changed.add(line.id);
        }
        if (e.speed !== undefined) {
          line.speed = e.speed as number;
          needsPost.add(line.id);
        }
        line.startMs = start;
        line.endMs = end;
      });
    },
    editor_insert_line: (a) => {
      const e = a.edit as {
        afterId: string | null;
        startMs: number;
        endMs: number;
        speakerId: string;
        translated: string;
      };
      return edit(() => {
        const id = `e${lines.length}x`;
        const at = lines.findIndex((l) => l.id === e.afterId) + 1;
        lines.splice(at, 0, {
          id,
          sectionIndex: 0,
          startMs: e.startMs,
          endMs: e.endMs,
          speakerId: e.speakerId,
          language: "en",
          text: "",
          translated: e.translated,
          audio: null,
          audioMs: null,
          flags: [],
          status: "translated",
          speed: 1,
          dubStartMs: null,
        });
        if (e.translated) changed.add(id);
      });
    },
    editor_delete_lines: (a) =>
      edit(() => {
        const ids = a.ids as string[];
        lines = lines.filter((l) => !ids.includes(l.id));
        ids.forEach((i) => changed.delete(i));
      }),
    editor_undo: () => {
      const before = snap();
      restore(undo.pop()!);
      redo.push(before);
      return result(before.lines);
    },
    editor_redo: () => {
      const before = snap();
      undo.push(before);
      restore(redo.pop()!);
      return result(before.lines);
    },
    editor_search: (a) => {
      const q = String(a.query);
      const fold = (s: string) => (a.matchCase ? s : s.toLowerCase());
      const matches: unknown[] = [];
      for (const l of lines) {
        for (const f of ["original", "translated"] as const) {
          if (a.field !== "both" && a.field !== f) continue;
          const text = (f === "original" ? l.text : l.translated) ?? "";
          let at = fold(text).indexOf(fold(q));
          while (q && at >= 0) {
            matches.push({ lineId: l.id, field: f, start: at, end: at + q.length });
            at = fold(text).indexOf(fold(q), at + q.length);
          }
        }
      }
      return { matches };
    },
    editor_replace: (a) =>
      edit(() => {
        const ms = (
          a.matches as { lineId: string; field: string; start: number; end: number }[]
        ).filter((m) => m.field === "translated");
        for (const l of lines) {
          const mine = ms.filter((m) => m.lineId === l.id).sort((x, y) => y.start - x.start);
          for (const m of mine) {
            l.translated =
              l.translated!.slice(0, m.start) + String(a.replacement) + l.translated!.slice(m.end);
            changed.add(l.id);
          }
        }
      }),
    editor_resynthesize: () => {
      const id = `run${++runId}`;
      canceled = false;
      const ids = [...new Set([...changed, ...needsPost])];
      void (async () => {
        let done = 0;
        for (const lineId of ids) {
          await new Promise((r) => setTimeout(r, fake.lineMs));
          if (canceled) break;
          const line = lines.find((l) => l.id === lineId);
          if (line) {
            line.audio = `clips/${lineId}.new.wav`;
            changed.delete(lineId);
            needsPost.delete(lineId);
            emit("editor:event", { type: "lineDone", runId: id, line });
          }
          done += 1;
          emit("editor:event", {
            type: "progress",
            runId: id,
            kind: "resynthesize",
            done,
            total: ids.length,
            etaSeconds: null,
          });
        }
        emit("editor:event", {
          type: "finished",
          runId: id,
          state: canceled ? "stopped" : "completed",
          error: null,
          remainingChangedIds: [...changed, ...needsPost],
        });
      })();
      return id;
    },
    editor_fit: (a) => {
      const ids = a.ids as string[] | null;
      const report = { mode: a.mode, lines: [] as unknown[], fitted: 0, stillFlagged: 0 };
      const before = snap();
      for (const l of lines) {
        if (ids ? !ids.includes(l.id) : a.mode === "smart" && !l.flags.includes("tooLong"))
          continue;
        if (a.mode === "smart") {
          const limit = l.id === "l3";
          const after = limit ? 1.3 : 1.2;
          report.lines.push({
            id: l.id,
            speedBefore: l.speed,
            speedAfter: after,
            dubStartBefore: null,
            dubStartAfter: null,
            fits: !limit,
            reason: limit ? "needsMoreThanLimit" : null,
          });
          l.speed = after;
          if (!limit) l.flags = l.flags.filter((f) => f !== "tooLong");
          limit ? report.stillFlagged++ : report.fitted++;
        } else {
          l.dubStartMs = Math.max(0, l.startMs - 300);
          report.lines.push({
            id: l.id,
            speedBefore: l.speed,
            speedAfter: l.speed,
            dubStartBefore: null,
            dubStartAfter: l.dubStartMs,
            fits: true,
            reason: null,
          });
          report.fitted++;
        }
      }
      undo.push(before);
      return report;
    },
    caption_templates_list: () => {
      const font = 'Inter, "Noto Sans Khmer", "Khmer UI", "Khmer Sangam MN", sans-serif';
      const base = {
        fontFamily: font,
        sizePercent: 5,
        color: "#ffffff",
        outlineColor: "#000000",
        outlineWidth: 2,
        background: null,
        position: "bottom",
        marginPercent: 6,
      };
      return [
        { ...base, id: "classic", nameKey: "editor.captions.classic" },
        {
          ...base,
          id: "soft-box",
          nameKey: "editor.captions.softBox",
          background: "#000000b3",
          outlineWidth: 0,
        },
        {
          ...base,
          id: "top-note",
          nameKey: "editor.captions.topNote",
          color: "#ffd84a",
          position: "top",
        },
      ];
    },
    caption_template_set: (a) => {
      template = String(a.id);
    },
    subtitles_export: (a) => {
      fake.exported.push({
        path: String(a.path),
        format: String(a.format),
        source: String(a.source),
      });
      return { cueCount: lines.length };
    },
    subtitles_import_preview: (a) => {
      if (String(a.path).endsWith("bad.srt"))
        throw {
          code: "SUBTITLE_PARSE_FAILED",
          messageKey: "editor.error.subtitleParseFailed",
          details: { detail: "cue 2 (line 6): the end is before the start" },
        };
      const same = fake.cues === lines.length;
      return {
        previewId: "pv1",
        format: "srt",
        encoding: "windows-1252",
        cueCount: fake.cues,
        lineCount: lines.length,
        allMatch: same && false,
        differences: [
          { index: 1, kind: "time", lineStartMs: 0, cueStartMs: 40 },
          ...(same
            ? []
            : [
                { index: lines.length + 1, kind: "extraCue", lineStartMs: null, cueStartMs: 99000 },
              ]),
        ],
        choices: same
          ? ["replaceText", "replaceLines", "fillByOverlap"]
          : ["replaceLines", "fillByOverlap"],
      };
    },
    subtitles_import_apply: (a) =>
      edit(() => {
        for (const l of lines) {
          l.translated = `Imported (${a.choice}) ${l.id}`;
          changed.add(l.id);
        }
      }),
    editor_merge_lines: (a) =>
      edit(() => {
        const ids = a.ids as string[];
        const first = lines.find((l) => l.id === ids[0])!;
        const second = lines.find((l) => l.id === ids[1])!;
        if (first.speakerId !== second.speakerId)
          throw { code: "EDIT_CANNOT_MERGE", messageKey: "editor.error.cannotMerge" };
        first.translated = `${first.translated} ${second.translated}`;
        first.endMs = second.endMs;
        lines = lines.filter((l) => l.id !== second.id);
        changed.add(first.id);
        changed.delete(second.id);
      }),
    editor_split_line: (a) =>
      edit(() => {
        const i = lines.findIndex((l) => l.id === a.id);
        const l = lines[i];
        const text = l.translated ?? "";
        const at = a.textOffset as number;
        if (at <= 0 || at >= text.length)
          throw { code: "EDIT_OUT_OF_RANGE", messageKey: "editor.error.outOfRange" };
        const cut = l.startMs + Math.floor(((l.endMs - l.startMs) * at) / text.length);
        const tail = {
          ...l,
          id: `${l.id}s`,
          startMs: cut,
          translated: text.slice(at).trim(),
          audio: null,
        };
        l.translated = text.slice(0, at).trim();
        l.endMs = cut;
        lines.splice(i + 1, 0, tail);
        changed.add(l.id);
        changed.add(tail.id);
      }),
    editor_duplicate_line: (a) =>
      edit(() => {
        const i = lines.findIndex((l) => l.id === a.id);
        const l = lines[i];
        const copy = {
          ...l,
          id: `${l.id}d`,
          startMs: l.endMs,
          endMs: l.endMs + (l.endMs - l.startMs),
          audio: null,
        };
        lines.splice(i + 1, 0, copy);
        changed.add(copy.id);
      }),
    editor_bulk_text: (a) =>
      edit(() => {
        for (const row of a.texts as { id: string; translated: string }[]) {
          const l = lines.find((x) => x.id === row.id)!;
          if (l.translated !== row.translated) {
            l.translated = row.translated;
            changed.add(l.id);
          }
        }
      }),
    editor_set_speaker_gender: (a) => {
      const before = snap();
      genders[String(a.speakerId)] = String(a.gender);
      for (const l of lines) if (l.speakerId === a.speakerId) changed.add(l.id);
      undo.push(before);
      return result(before.lines);
    },
    editor_stop: () => {
      canceled = true;
    },
    // The dub flow, enough to reach the start button.
    languages_list: () =>
      ["en", "fr"].map((code) => ({
        code,
        transcribe: true,
        translateFrom: true,
        translateTo: true,
        speak: { voices: 1, permissiveVoices: 1 },
        quality: { transcribe: null, translate: null, speak: null },
      })),
    languages_check_pair: () => ({ ok: true, steps: [], fallbacks: [] }),
    cloud_services: () => [],
    dub_probe_source: () => ({
      durationMs: 20000,
      hasAudio: true,
      hasVideo: true,
      container: "mp4",
      sizeBytes: 1_000_000,
    }),
    dub_configure: () => handle,
    dub_preflight: () => ({
      missingModels: [],
      downloadBytes: 0,
      diskNeededBytes: 1,
      diskFreeBytes: 1e12,
      machineClass: "fullLocal",
      warnings: [],
      blockers: [],
      cloud: [],
      editsWillBeLost: fake.editsWillBeLost,
    }),
    dub_start: (a) => {
      fake.lastStart = a;
      return "r1";
    },
    dub_status: () => ({
      run: null,
      interrupted: false,
      running: false,
      dub: {
        source: null,
        sourceLanguage: "fr",
        targetLanguage: "en",
        options: { muteOriginal: false, exportKind: "video", exportName: "x", quality: "standard" },
        speakers: [],
        lines: [],
        cloudUsage: [],
      },
      hasDubAudio: false,
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

/** Open the app on a project whose finished dub has `count` lines. */
export async function start(page: Page, count = 12) {
  await page.addInitScript(installFakeCore, count);
  await page.goto("/");
  await page.getByLabel("Project name").fill("Edit test");
  await page.getByRole("button", { name: "Create…" }).click();
  await expect(page.getByTestId("editor-table")).toBeVisible();
}

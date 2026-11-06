import { useState } from "react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import type { TranscriptLine } from "../dub/types";
import { useEditorContext } from "./EditorContext";
import type { FitMode, LineEdit } from "./types";

/** The safe ranges the core enforces (core/src/editor/limits.rs). */
export const RANGES = {
  speed: { min: 0.85, max: 1.3, step: 0.01 },
  volumeDb: { min: -12, max: 6, step: 0.5 },
  pitchSemitones: { min: -4, max: 4, step: 0.5 },
} as const;

type Knob = keyof typeof RANGES;

function Setting({
  knob,
  label,
  value,
  format,
  disabled,
  onCommit,
}: {
  knob: Knob;
  label: string;
  value: number;
  format: (v: number) => string;
  disabled: boolean;
  onCommit: (v: number) => void;
}) {
  const [live, setLive] = useState<number | null>(null);
  const r = RANGES[knob];
  return (
    <div className="flex items-center gap-2">
      <Label className="w-12 text-xs text-muted-foreground uppercase">{label}</Label>
      <Slider
        className="shrink-0 data-horizontal:w-28"
        min={r.min}
        max={r.max}
        step={r.step}
        disabled={disabled}
        value={[live ?? value]}
        aria-label={label}
        onValueChange={(v) => setLive(Array.isArray(v) ? v[0] : v)}
        onValueCommitted={(v) => {
          setLive(null);
          const n = Array.isArray(v) ? v[0] : v;
          if (n !== value) onCommit(n);
        }}
      />
      <span className="w-14 font-mono text-xs" data-testid={`value-${knob}`}>
        {format(live ?? value)}
      </span>
    </div>
  );
}

/** Smart Fit, End Sync, and speed, volume and pitch for the one selected line. */
export function FitControls() {
  const ctx = useEditorContext();
  if (!ctx) return null;
  const { ed, readOnly, selected } = ctx;
  const { view } = ed;
  const one: TranscriptLine | undefined =
    selected.size === 1 ? view.lines.find((l) => l.id === [...selected][0]) : undefined;
  const busy = readOnly || view.run !== null || view.fitting || view.lines.length === 0;
  const ids = selected.size ? [...selected] : undefined;
  const scope = selected.size ? t("editor.fit.scopeSelected") : t("editor.fit.scopeFlagged");
  const run = (mode: FitMode) => void ed.fit(mode, ids);
  const set = (edit: Omit<LineEdit, "id">) => one && void ed.update({ id: one.id, ...edit });

  return (
    <div className="flex flex-col gap-2" data-testid="fit-controls">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <Button
          size="sm"
          variant="outline"
          disabled={busy}
          onClick={() => run("smart")}
          title={scope}
        >
          {t("editor.smartFit")}
        </Button>
        <Button size="sm" variant="outline" disabled={busy} onClick={() => run("endSync")}>
          {t("editor.endSync")}
        </Button>
        <Setting
          knob="speed"
          label={t("editor.fit.speed")}
          value={one?.speed ?? 1}
          format={(v) => t("editor.fit.speedValue", { value: v.toFixed(2) })}
          disabled={busy || !one}
          onCommit={(speed) => set({ speed })}
        />
        <Setting
          knob="volumeDb"
          label={t("editor.fit.volume")}
          value={one?.volumeDb ?? 0}
          format={(v) => t("editor.fit.dbValue", { value: v.toFixed(1) })}
          disabled={busy || !one}
          onCommit={(volumeDb) => set({ volumeDb })}
        />
        <Setting
          knob="pitchSemitones"
          label={t("editor.fit.pitch")}
          value={one?.pitchSemitones ?? 0}
          format={(v) => t("editor.fit.stValue", { value: v.toFixed(1) })}
          disabled={busy || !one}
          onCommit={(pitchSemitones) => set({ pitchSemitones })}
        />
      </div>
      {!one && <p className="text-xs text-muted-foreground">{t("editor.fit.pickOne")}</p>}
      {view.fitting && <p className="text-xs text-muted-foreground">{t("editor.fit.running")}</p>}
      {view.fitReport && (
        <div className="text-xs" data-testid="fit-summary">
          <p>
            {view.fitReport.lines.length === 0
              ? t("editor.fit.nothing")
              : t("editor.fit.summary", {
                  fitted: view.fitReport.fitted,
                  still: view.fitReport.stillFlagged,
                })}
          </p>
          <ul className="text-muted-foreground">
            {view.fitReport.lines
              .filter((l) => l.reason)
              .map((l) => (
                <li key={l.id}>
                  {t("editor.fit.line", {
                    n: view.lines.findIndex((x) => x.id === l.id) + 1,
                    reason: t(`editor.fit.reason.${l.reason}`),
                  })}
                </li>
              ))}
          </ul>
        </div>
      )}
    </div>
  );
}

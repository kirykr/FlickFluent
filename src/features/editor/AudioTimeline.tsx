import { Languages, Scissors } from "lucide-react";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Slider } from "@/components/ui/slider";
import type { TranscriptLine } from "../dub/types";
import { ComingSoon } from "./ComingSoon";
import { useEditorContext } from "./EditorContext";
import { FitControls } from "./FitControls";

function Knob({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center gap-2">
      <Label className="text-xs text-muted-foreground uppercase">{label}</Label>
      <Slider
        className="shrink-0 data-horizontal:w-20"
        disabled
        defaultValue={[value]}
        aria-label={label}
      />
    </div>
  );
}

/** The colour of a clip: the track's own, or a warning when its line is flagged. */
function clipClass(line: TranscriptLine, base: string): string {
  if (line.flags.includes("tooLong")) return "border-destructive/60 bg-destructive/30";
  if (line.flags.includes("overlap")) return "border-warning/60 bg-warning/30";
  return base;
}

function Tracks() {
  const ctx = useEditorContext();
  if (!ctx || ctx.ed.view.lines.length === 0) return null;
  const { ed, selected, setSelected } = ctx;
  const { lines, speakers } = ed.view;
  const total = Math.max(
    1,
    ...lines.map(
      (l) => (l.dubStartMs ?? l.startMs) + Math.max(l.audioMs ?? 0, l.endMs - l.startMs),
    ),
  );
  const ids = speakers.length
    ? speakers.map((s) => s.id)
    : [...new Set(lines.map((l) => l.speakerId))];
  return (
    <div className="flex min-w-[40rem] flex-col gap-1.5" data-testid="timeline-tracks">
      {ids.map((id) => (
        <div key={id} className="flex items-stretch gap-2">
          <span className="w-12 shrink-0 self-center truncate font-mono text-xs text-muted-foreground">
            {id}
          </span>
          <div className="relative h-9 flex-1 rounded-md bg-muted/50">
            {lines
              .map((line, i) => ({ line, i }))
              .filter(({ line }) => line.speakerId === id)
              .map(({ line, i }) => {
                const from = line.dubStartMs ?? line.startMs;
                const width = line.audioMs ?? line.endMs - line.startMs;
                return (
                  <button
                    key={line.id}
                    type="button"
                    data-testid="timeline-clip"
                    data-line={line.id}
                    data-left={from}
                    aria-label={t("editor.clip", { n: i + 1, start: Math.round(from / 100) / 10 })}
                    aria-pressed={selected.has(line.id)}
                    onClick={() => setSelected(new Set([line.id]))}
                    className={`absolute inset-y-1 rounded-md border ${clipClass(line, "border-success/50 bg-success/30")} ${selected.has(line.id) ? "ring-2 ring-primary" : ""}`}
                    style={{
                      left: `${(from / total) * 100}%`,
                      width: `${Math.max(0.3, (width / total) * 100)}%`,
                    }}
                  />
                );
              })}
          </div>
        </div>
      ))}
    </div>
  );
}

/** Bottom strip of the editor: the fitting controls and one track of clips for each speaker. */
export function AudioTimeline() {
  const ctx = useEditorContext();
  return (
    <Card size="sm">
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <h2 className="text-sm font-semibold">{t("editor.timeline")}</h2>
          <Knob label={t("editor.zoom")} value={20} />
          <ComingSoon>
            <Button size="sm" variant="outline" disabled>
              <Scissors data-icon="inline-start" />
              {t("editor.split")}
            </Button>
          </ComingSoon>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <ComingSoon>
              <Button size="sm" variant="outline" disabled>
                <Languages data-icon="inline-start" />
                {t("editor.language")}
              </Button>
            </ComingSoon>
            <ComingSoon>
              <Button size="sm" disabled className="bg-success text-white">
                {t("editor.startTranscription")}
              </Button>
            </ComingSoon>
            {ctx && (
              <Badge variant="outline">
                {t("editor.items", { count: ctx.ed.view.lines.length })}
              </Badge>
            )}
          </div>
        </div>
        {ctx ? <FitControls /> : null}
        <ScrollArea>
          <Tracks />
        </ScrollArea>
      </CardContent>
    </Card>
  );
}

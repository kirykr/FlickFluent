import { Languages, Scissors, Sparkles } from "lucide-react";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Slider } from "@/components/ui/slider";
import { ComingSoon } from "./ComingSoon";
import { SAMPLE_CLIPS } from "./placeholder";

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

function Track({ label, className }: { label: string; className: string }) {
  return (
    <div className="flex items-stretch gap-2">
      <span className="w-8 shrink-0 self-center font-mono text-xs text-muted-foreground">
        {label}
      </span>
      <div className="relative h-9 flex-1 rounded-md bg-muted/50">
        {SAMPLE_CLIPS.map((c) => (
          <div
            key={c.left}
            className={`absolute inset-y-1 rounded-md border ${className}`}
            style={{ left: `${c.left}%`, width: `${c.width}%` }}
          />
        ))}
      </div>
    </div>
  );
}

/** Bottom strip of the editor: transport knobs, action buttons and two sample tracks. */
export function AudioTimeline() {
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
            <ComingSoon>
              <Button size="sm" variant="outline" disabled>
                {t("editor.smartFit")}
              </Button>
            </ComingSoon>
            <ComingSoon>
              <Button size="sm" variant="outline" disabled>
                {t("editor.endSync")}
              </Button>
            </ComingSoon>
            <ComingSoon>
              <Button size="sm" disabled className="bg-warning text-white">
                <Sparkles data-icon="inline-start" />
                {t("editor.synthesize")}
              </Button>
            </ComingSoon>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <Knob label={t("editor.speed")} value={40} />
          <Knob label={t("editor.vol")} value={60} />
          <Knob label={t("editor.pitch")} value={50} />
          <ComingSoon>
            <Button size="sm" variant="secondary" disabled>
              {t("editor.reverb")}
            </Button>
          </ComingSoon>
          <Badge variant="outline">{t("editor.items", { count: SAMPLE_CLIPS.length })}</Badge>
        </div>
        <ScrollArea>
          <div className="flex min-w-[40rem] flex-col gap-1.5">
            <Track label={t("editor.trackText")} className="border-primary/50 bg-primary/30" />
            <Track label={t("editor.trackAudio")} className="border-success/50 bg-success/30" />
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}

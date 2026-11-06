import {
  Captions,
  Clapperboard,
  Download,
  Eye,
  Headphones,
  Maximize,
  Mic,
  MicOff,
  Upload,
  WandSparkles,
} from "lucide-react";
import { useState, type ComponentType } from "react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ComingSoon } from "./ComingSoon";
import { useEditorContext } from "./EditorContext";
import { SubtitlesDialog } from "./SubtitlesDialog";

const TOOLS: { key: string; icon: ComponentType<{ className?: string }>; wide?: boolean }[] = [
  { key: "importMedia", icon: Clapperboard },
  { key: "addSoundtrack", icon: Headphones },
  { key: "importVoiceover", icon: Mic },
  { key: "vocalSeparator", icon: WandSparkles },
  { key: "audioMuted", icon: MicOff },
  { key: "exportSubtitles", icon: Upload },
  { key: "importSubtitles", icon: Download },
  { key: "aspectRatio", icon: Maximize },
  { key: "privacyMask", icon: Eye, wide: true },
];

/** Left column of the editor: tool buttons, all placeholders for now. */
export function ToolPanel() {
  const ctx = useEditorContext();
  const [dialog, setDialog] = useState<"export" | "import" | null>(null);
  const [last, setLast] = useState<"export" | "import">("export");
  const ready = !!ctx && ctx.ed.view.ready && ctx.ed.view.lines.length > 0;
  const live: Record<string, "export" | "import"> = {
    exportSubtitles: "export",
    importSubtitles: "import",
  };
  const templates = ctx?.ed.view.templates ?? [];
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>{t("editor.tools")}</CardTitle>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-3">
        <ScrollArea className="min-h-0 flex-1">
          <div className="grid grid-cols-2 gap-2 pr-2">
            {TOOLS.map(({ key, icon: Icon, wide }) =>
              live[key] ? (
                <Button
                  key={key}
                  variant="outline"
                  disabled={!ready}
                  className="h-auto w-full flex-col gap-1.5 py-2 whitespace-normal"
                  onClick={() => {
                    setLast(live[key]);
                    setDialog(live[key]);
                  }}
                >
                  <Icon className="size-5 text-primary" />
                  <span className="text-center text-xs">{t(`editor.tool.${key}`)}</span>
                </Button>
              ) : (
                <ComingSoon key={key} className={wide ? "col-span-2" : undefined}>
                  <Button
                    variant="outline"
                    disabled
                    className="h-auto w-full flex-col gap-1.5 py-2 whitespace-normal"
                  >
                    <Icon className="size-5 text-primary" />
                    <span className="text-center text-xs">{t(`editor.tool.${key}`)}</span>
                  </Button>
                </ComingSoon>
              ),
            )}
          </div>
        </ScrollArea>
        {ctx && templates.length > 0 ? (
          <div className="flex flex-col gap-1">
            <Label htmlFor="caption-template" className="flex items-center gap-1.5 text-xs">
              <Captions className="size-4" />
              {t("editor.captionTemplateLabel")}
            </Label>
            <Select
              items={templates.map((x) => ({ value: x.id, label: t(x.nameKey) }))}
              value={ctx.ed.view.captionTemplate}
              disabled={ctx.readOnly}
              onValueChange={(v) => v && ctx.ed.setTemplate(v)}
            >
              <SelectTrigger id="caption-template">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {templates.map((x) => (
                  <SelectItem key={x.id} value={x.id}>
                    {t(x.nameKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        ) : (
          <ComingSoon>
            <Button disabled size="lg" className="w-full">
              <Captions data-icon="inline-start" />
              {t("editor.captionTemplate")}
            </Button>
          </ComingSoon>
        )}
        <SubtitlesDialog
          open={dialog !== null}
          tab={dialog ?? last}
          onOpenChange={(o) => !o && setDialog(null)}
        />
      </CardContent>
    </Card>
  );
}

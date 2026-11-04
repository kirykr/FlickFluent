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
import type { ComponentType } from "react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ComingSoon } from "./ComingSoon";

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
  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>{t("editor.tools")}</CardTitle>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-3">
        <ScrollArea className="min-h-0 flex-1">
          <div className="grid grid-cols-2 gap-2 pr-2">
            {TOOLS.map(({ key, icon: Icon, wide }) => (
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
            ))}
          </div>
        </ScrollArea>
        <ComingSoon>
          <Button disabled size="lg" className="w-full">
            <Captions data-icon="inline-start" />
            {t("editor.captionTemplate")}
          </Button>
        </ComingSoon>
      </CardContent>
    </Card>
  );
}

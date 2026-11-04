import { Film, Play, Square, Volume2 } from "lucide-react";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Slider } from "@/components/ui/slider";
import { ComingSoon } from "./ComingSoon";

/** Centre column of the editor: video frame, clock and transport controls. */
export function PreviewMonitor() {
  return (
    <Card className="h-full">
      <CardHeader className="flex flex-row items-center justify-between gap-2">
        <CardTitle className="whitespace-nowrap">{t("editor.preview")}</CardTitle>
        <Badge variant="secondary" className="shrink-0">
          {t("editor.previewStage")}
        </Badge>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col items-center gap-3">
        <div className="flex min-h-0 w-full max-w-60 flex-1 flex-col items-center justify-center gap-2 rounded-lg bg-muted text-muted-foreground">
          <Film className="size-8" />
          <p className="px-4 text-center text-xs">{t("editor.previewEmpty")}</p>
        </div>
        <div className="flex w-full max-w-60 flex-col gap-2 rounded-lg border p-3">
          <div className="flex justify-between font-mono text-xs">
            <span>00:00.00</span>
            <span className="text-muted-foreground">00:00.00</span>
          </div>
          <Progress value={0} />
          <div className="flex items-center gap-2">
            <ComingSoon>
              <Button size="icon" disabled aria-label={t("editor.play")}>
                <Play />
              </Button>
            </ComingSoon>
            <ComingSoon>
              <Button size="icon" variant="outline" disabled aria-label={t("editor.stop")}>
                <Square />
              </Button>
            </ComingSoon>
            <Volume2 className="ml-auto size-4 text-muted-foreground" />
            <Slider
              className="shrink-0 data-horizontal:w-16"
              disabled
              defaultValue={[80]}
              aria-label={t("editor.volume")}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { modelDownloadStart } from "../../app/ipc";
import { formatBytes } from "../models/format";
import type { ModelRecord } from "../models/types";
import { percent } from "./format";
import type { MissingModel } from "./types";

interface Props {
  models: MissingModel[];
  disabled?: boolean;
  /** Reports whether any of the models is downloading right now. */
  onDownloading?: (downloading: boolean) => void;
  /** A model finished installing: the caller can check again what is still missing. */
  onInstalled?: () => void;
  /** A download could not start. */
  onError?: (e: unknown) => void;
}

/** The size prompt and progress for models a dub needs, in the new-dub form and in a run. */
export function ModelPrompt({ models, disabled, onDownloading, onInstalled, onError }: Props) {
  const [downloads, setDownloads] = useState<Record<string, ModelRecord>>({});

  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    void listen<ModelRecord>("model:progress", (e) => {
      setDownloads((d) => ({ ...d, [e.payload.id]: e.payload }));
      if (e.payload.state === "installed") onInstalled?.();
    }).then((u) => (alive ? (off = u) : u()));
    return () => {
      alive = false;
      off?.();
    };
    // The listener must not be torn down on every render of the caller.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const downloading = models.some((m) => {
    const d = downloads[m.id];
    return d && (d.state === "downloading" || d.state === "verifying");
  });
  useEffect(() => {
    onDownloading?.(downloading);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [downloading]);

  const total = models.reduce((sum, m) => sum + m.sizeBytes, 0);
  const done = models.reduce((sum, m) => sum + (downloads[m.id]?.bytesDone ?? 0), 0);

  const download = async () => {
    for (const m of models) {
      try {
        await modelDownloadStart(m.id);
      } catch (e) {
        onError?.(e);
        return;
      }
    }
  };

  if (models.length === 0) return null;
  return (
    <Card size="sm" data-testid="dub-missing">
      <CardHeader>
        <CardTitle>{t("dub.new.missingTitle")}</CardTitle>
        <CardDescription>{t("dub.new.missingBody", { size: formatBytes(total) })}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <ul className="text-sm">
          {models.map((m) => (
            <li key={m.id} className="flex justify-between gap-3">
              <span className="truncate">{m.name}</span>
              <span className="text-muted-foreground">{formatBytes(m.sizeBytes)}</span>
            </li>
          ))}
        </ul>
        {downloading ? (
          <Progress
            value={percent(total ? done / total : 0)}
            aria-label={t("dub.new.downloading")}
          />
        ) : (
          <Button onClick={download} disabled={disabled}>
            {t("dub.new.download", { size: formatBytes(total) })}
          </Button>
        )}
      </CardContent>
    </Card>
  );
}

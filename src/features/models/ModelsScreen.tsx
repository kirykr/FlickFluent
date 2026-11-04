import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Download } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
  modelDelete,
  modelDownloadCancel,
  modelDownloadPause,
  modelDownloadResume,
  modelDownloadStart,
  modelsCatalogue,
} from "../../app/ipc";
import { asAppError } from "../projects/errors";
import { formatBytes, installedBytes, modelError, percent } from "./format";
import type { ModelRecord } from "./types";

export function ModelsScreen() {
  const [models, setModels] = useState<ModelRecord[]>([]);
  const [confirming, setConfirming] = useState<ModelRecord | null>(null);
  const [message, setMessage] = useState<{ key: string; params: Record<string, string> } | null>(
    null,
  );

  const reload = useCallback(() => {
    modelsCatalogue()
      .then(setModels)
      .catch(() => setModels([]));
  }, []);
  useEffect(reload, [reload]);

  // The core sends `model:progress` with the full record, about five times a second.
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    listen<ModelRecord>("model:progress", (e) => {
      setModels((list) => list.map((m) => (m.id === e.payload.id ? e.payload : m)));
    })
      .then((un) => (cancelled ? un() : (off = un)))
      .catch(() => {});
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  // Runs a command; any error becomes a message and the list is refreshed either way.
  const run = async (action: () => Promise<void>) => {
    setMessage(null);
    try {
      await action();
    } catch (e) {
      const err = asAppError(e);
      setMessage(modelError(err.code, err.details));
    }
    reload();
  };

  const start = (m: ModelRecord) => {
    setConfirming(null);
    return run(() => modelDownloadStart(m.id));
  };

  const remove = (m: ModelRecord) => {
    if (!window.confirm(t("models.confirmDelete"))) return;
    return run(() => modelDelete(m.id));
  };

  const active = (m: ModelRecord) =>
    m.state === "downloading" || m.state === "paused" || m.state === "failed";

  return (
    <section className="mx-auto flex max-w-3xl flex-col gap-4 py-4">
      <div>
        <h1 className="text-xl font-semibold">{t("models.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("models.intro")}</p>
      </div>

      {message && (
        <Alert variant="destructive" role="alert">
          <AlertDescription>{t(message.key, message.params)}</AlertDescription>
        </Alert>
      )}

      {confirming && (
        <Alert role="alertdialog" aria-label={t("models.confirmTitle")}>
          <Download />
          <AlertTitle>{t("models.confirmTitle")}</AlertTitle>
          <AlertDescription>
            <p>
              <b>{confirming.name}</b>
              <br />
              {confirming.purpose}
            </p>
            <p className="mt-1">
              {t("models.size")}: {formatBytes(confirming.sizeBytes)} · {t("models.licence")}:{" "}
              {confirming.licence}
            </p>
            <div className="mt-2 flex gap-2">
              <Button onClick={() => start(confirming)}>{t("models.confirmStart")}</Button>
              <Button variant="outline" onClick={() => setConfirming(null)}>
                {t("models.confirmCancel")}
              </Button>
            </div>
          </AlertDescription>
        </Alert>
      )}

      {models.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("models.empty")}</p>
      ) : (
        <ul className="flex flex-col gap-3">
          {models.map((m) => (
            <li key={m.id} data-testid={`model-${m.id}`}>
              <Card>
                <CardContent className="flex items-start justify-between gap-4">
                  <div className="flex min-w-0 flex-1 flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <strong>{m.name}</strong>
                      <span className="text-xs text-muted-foreground">
                        {formatBytes(m.sizeBytes)} · {m.licence}
                      </span>
                    </div>
                    <p className="text-sm text-muted-foreground">{m.purpose}</p>
                    <p>
                      <Badge
                        variant={m.state === "installed" ? "default" : "secondary"}
                        data-testid={`state-${m.id}`}
                      >
                        {t(`models.state.${m.state}`)}
                      </Badge>
                    </p>
                    {active(m) && (
                      <div className="flex items-center gap-3">
                        <Progress className="max-w-xs" value={percent(m.bytesDone, m.sizeBytes)} />
                        <span
                          className="text-xs text-muted-foreground"
                          data-testid={`progress-${m.id}`}
                        >
                          {t("models.progress", {
                            done: formatBytes(m.bytesDone),
                            total: formatBytes(m.sizeBytes),
                          })}
                        </span>
                      </div>
                    )}
                    {m.state === "failed" && m.errorCode && (
                      <p className="text-sm text-destructive" role="alert">
                        {t(modelError(m.errorCode).key)}
                      </p>
                    )}
                  </div>
                  <div className="flex gap-2">
                    {m.state === "notInstalled" && (
                      <Button onClick={() => setConfirming(m)}>{t("models.download")}</Button>
                    )}
                    {m.state === "downloading" && (
                      <Button variant="outline" onClick={() => run(() => modelDownloadPause(m.id))}>
                        {t("models.pause")}
                      </Button>
                    )}
                    {m.state === "paused" && (
                      <Button onClick={() => run(() => modelDownloadResume(m.id))}>
                        {t("models.resume")}
                      </Button>
                    )}
                    {m.state === "failed" && (
                      <Button onClick={() => run(() => modelDownloadResume(m.id))}>
                        {t("models.retry")}
                      </Button>
                    )}
                    {active(m) && (
                      <Button variant="ghost" onClick={() => run(() => modelDownloadCancel(m.id))}>
                        {t("models.cancel")}
                      </Button>
                    )}
                    {m.state === "installed" && (
                      <Button variant="destructive" onClick={() => remove(m)}>
                        {t("models.delete")}
                      </Button>
                    )}
                  </div>
                </CardContent>
              </Card>
            </li>
          ))}
        </ul>
      )}

      <p className="text-sm text-muted-foreground" data-testid="storage-total">
        {t("models.storage")}: {formatBytes(installedBytes(models))}
      </p>
    </section>
  );
}

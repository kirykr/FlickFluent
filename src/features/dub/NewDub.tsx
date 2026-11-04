import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { FileVideo } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  dubConfigure,
  dubPreflight,
  dubProbeSource,
  dubStart,
  modelDownloadStart,
} from "../../app/ipc";
import { asAppError } from "../projects/errors";
import { formatBytes } from "../models/format";
import type { ModelRecord } from "../models/types";
import type { AppErrorShape } from "../projects/types";
import { BASIC_LANGUAGES, baseName, dubError, formatClock, languageName, percent } from "./format";
import type { DubData, DubOptions, Preflight, Quality, SourceInfo } from "./types";

interface Props {
  handle: string;
  /** What the project already holds, to prefill the form. */
  saved: DubData | null;
  readOnly: boolean;
  onStarted: (runId: string) => void;
}

const VIDEO_EXTENSIONS = ["mp4", "mkv", "mov", "avi", "webm", "m4v", "mp3", "wav", "m4a", "flac"];

export function NewDub({ handle, saved, readOnly, onStarted }: Props) {
  const [path, setPath] = useState<string>(saved?.source?.path ?? "");
  const [info, setInfo] = useState<SourceInfo | null>(null);
  const [source, setSource] = useState(saved?.sourceLanguage || "auto");
  const [target, setTarget] = useState(saved?.targetLanguage || "en");
  const [mute, setMute] = useState(saved?.options.muteOriginal ?? false);
  const [quality, setQuality] = useState<Quality>(saved?.options.quality ?? "standard");
  const [pre, setPre] = useState<Preflight | null>(null);
  const [error, setError] = useState<AppErrorShape | null>(null);
  const [busy, setBusy] = useState(false);
  const [sameOk, setSameOk] = useState(false);
  const [downloads, setDownloads] = useState<Record<string, ModelRecord>>({});

  const options = useCallback(
    (): DubOptions => ({
      muteOriginal: mute,
      exportKind: saved?.options.exportKind ?? "video",
      exportName: saved?.options.exportName || baseName(path),
      quality,
    }),
    [mute, quality, saved, path],
  );

  const attempt = useCallback(async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
    setError(null);
    try {
      return await fn();
    } catch (e) {
      setError(asAppError(e));
      return undefined;
    }
  }, []);

  const check = useCallback(async () => {
    if (!path) return;
    setBusy(true);
    const result = await attempt(async () => {
      await dubConfigure(handle, path, source, target, options(), sameOk);
      return dubPreflight(handle);
    });
    if (result) setPre(result);
    setBusy(false);
  }, [attempt, handle, options, path, sameOk, source, target]);

  // The model listener below outlives renders, so it calls the latest `check` through this.
  const checkRef = useRef(check);
  checkRef.current = check;

  // Re-check when any choice changes and a file is chosen.
  useEffect(() => {
    if (path && !readOnly) void check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, source, target, mute, quality, sameOk]);

  useEffect(() => {
    let off: (() => void) | undefined;
    void listen<ModelRecord>("model:progress", (e) => {
      setDownloads((d) => ({ ...d, [e.payload.id]: e.payload }));
      if (e.payload.state === "installed") void checkRef.current();
    }).then((u) => (off = u));
    return () => off?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handle]);

  const choose = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: t("dub.new.mediaFiles"), extensions: VIDEO_EXTENSIONS }],
    });
    if (typeof picked !== "string") return;
    setInfo(null);
    const probed = await attempt(() => dubProbeSource(picked));
    if (probed) {
      setInfo(probed);
      setPath(picked);
      setSameOk(false);
    }
  };

  const downloadMissing = async () => {
    for (const m of pre?.missingModels ?? []) {
      await attempt(() => modelDownloadStart(m.id));
    }
  };

  const start = async () => {
    setBusy(true);
    const runId = await attempt(() => dubStart(handle));
    setBusy(false);
    if (runId) onStarted(runId);
  };

  const languages = (withAuto: boolean) =>
    BASIC_LANGUAGES.filter((c) => withAuto || c !== "auto").map((c) => ({
      value: c,
      label: c === "auto" ? t("dub.new.autoDetect") : languageName(c),
    }));

  const sameLanguage = source !== "auto" && source === target;
  const downloading = (pre?.missingModels ?? []).some((m) => {
    const d = downloads[m.id];
    return d && (d.state === "downloading" || d.state === "verifying");
  });
  const blocked = (pre?.blockers.length ?? 0) > 0;
  const ready =
    !!pre && !blocked && pre.missingModels.length === 0 && !sameLanguage && !downloading;
  const err = error
    ? dubError(error as AppErrorShape & { details?: Record<string, unknown> })
    : null;
  const total = (pre?.missingModels ?? []).reduce((sum, m) => sum + m.sizeBytes, 0);
  const done = (pre?.missingModels ?? []).reduce(
    (sum, m) => sum + (downloads[m.id]?.bytesDone ?? 0),
    0,
  );

  return (
    <section
      className="mx-auto flex w-full max-w-2xl flex-col gap-4 py-2"
      aria-label={t("dub.title")}
    >
      <Card>
        <CardHeader>
          <CardTitle>{t("dub.new.title")}</CardTitle>
          <CardDescription>{t("dub.new.intro")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex items-center gap-3">
            <Button variant="outline" onClick={choose} disabled={readOnly || busy}>
              <FileVideo />
              {path ? t("dub.new.change") : t("dub.new.choose")}
            </Button>
            <div className="min-w-0 flex-1 text-sm" data-testid="dub-source">
              {path ? (
                <>
                  <p className="truncate font-medium">{baseName(path)}</p>
                  {info && (
                    <p className="text-xs text-muted-foreground">
                      {formatClock(info.durationMs)} · {formatBytes(info.sizeBytes)}
                    </p>
                  )}
                </>
              ) : (
                <p className="text-muted-foreground">{t("dub.new.noFile")}</p>
              )}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="dub-source-language">{t("dub.new.sourceLanguage")}</Label>
              <Select
                items={languages(true)}
                value={source}
                onValueChange={(v) => v && setSource(v)}
              >
                <SelectTrigger id="dub-source-language" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {languages(true).map((l) => (
                    <SelectItem key={l.value} value={l.value}>
                      {l.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="dub-target-language">{t("dub.new.targetLanguage")}</Label>
              <Select
                items={languages(false)}
                value={target}
                onValueChange={(v) => v && setTarget(v)}
              >
                <SelectTrigger id="dub-target-language" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {languages(false).map((l) => (
                    <SelectItem key={l.value} value={l.value}>
                      {l.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="flex items-center justify-between gap-3">
            <div>
              <Label htmlFor="dub-mute">{t("dub.new.muteOriginal")}</Label>
              <p className="text-xs text-muted-foreground">{t("dub.new.muteOriginalHint")}</p>
            </div>
            <Switch id="dub-mute" checked={mute} onCheckedChange={setMute} />
          </div>

          <div className="flex items-center justify-between gap-3">
            <div>
              <Label htmlFor="dub-fast">{t("dub.new.fast")}</Label>
              <p className="text-xs text-muted-foreground">{t("dub.new.fastHint")}</p>
            </div>
            <Switch
              id="dub-fast"
              checked={quality === "fast"}
              onCheckedChange={(on) => setQuality(on ? "fast" : "standard")}
            />
          </div>
        </CardContent>
      </Card>

      {sameLanguage && (
        <Alert role="alert">
          <AlertTitle>{t("dub.error.sameLanguage")}</AlertTitle>
          <AlertDescription>
            <Button size="sm" variant="outline" onClick={() => setSameOk(true)}>
              {t("dub.new.sameConfirm")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {err && (
        <Alert variant="destructive" role="alert" data-testid="dub-error">
          <AlertDescription>{t(err.key, err.params)}</AlertDescription>
        </Alert>
      )}

      {pre?.warnings.map((w) => (
        <Alert key={w}>
          <AlertDescription>{t(w)}</AlertDescription>
        </Alert>
      ))}

      {pre?.blockers.map((b) => (
        <Alert key={`${b.step}-${b.reason}`} variant="destructive" role="alert">
          <AlertDescription>
            {t(b.reason === "machine" ? "dub.blocker.machine" : "dub.blocker.language", {
              step: b.step,
              detail: b.detail,
            })}
          </AlertDescription>
        </Alert>
      ))}

      {pre && pre.missingModels.length > 0 && (
        <Card size="sm" data-testid="dub-missing">
          <CardHeader>
            <CardTitle>{t("dub.new.missingTitle")}</CardTitle>
            <CardDescription>
              {t("dub.new.missingBody", { size: formatBytes(total) })}
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <ul className="text-sm">
              {pre.missingModels.map((m) => (
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
              <Button onClick={downloadMissing} disabled={readOnly}>
                {t("dub.new.download", { size: formatBytes(total) })}
              </Button>
            )}
          </CardContent>
        </Card>
      )}

      {pre && (
        <p className="text-xs text-muted-foreground" data-testid="dub-disk">
          {t("dub.new.disk", {
            needed: formatBytes(pre.diskNeededBytes),
            free: pre.diskFreeBytes === null ? "?" : formatBytes(pre.diskFreeBytes),
          })}
        </p>
      )}

      <div className="flex justify-end">
        <Button size="lg" disabled={!ready || busy || readOnly} onClick={start}>
          {t("dub.new.start")}
        </Button>
      </div>
    </section>
  );
}

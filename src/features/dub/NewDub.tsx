import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FileVideo } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  dubConfigure,
  dubPreflight,
  dubProbeSource,
  dubStart,
  languagesCheckPair,
  cloudServices,
  languagesList,
} from "../../app/ipc";
import { asAppError } from "../projects/errors";
import { ConsentDialog } from "../cloud/ConsentDialog";
import { roundChars, type CloudPreflight } from "../cloud/types";
import { formatBytes } from "../models/format";
import type { AppErrorShape } from "../projects/types";
import { baseName, dubError, formatClock } from "./format";
import { LanguagePicker } from "./LanguagePicker";
import { ModelPrompt } from "./ModelPrompt";
import { pairResolved } from "./languages";
import { PairCheck } from "./PairCheck";
import {
  NO_FALLBACKS,
  type DubData,
  type DubOptions,
  type FallbackChoices,
  type Language,
  type PairCheck as PairCheckResult,
  type Preflight,
  type Quality,
  type SourceInfo,
} from "./types";

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
  const [downloading, setDownloading] = useState(false);
  const [languages, setLanguages] = useState<Language[]>([]);
  const [pair, setPair] = useState<PairCheckResult | null>(null);
  // Cloud steps that still need the user's yes before the run starts (one question at a time).
  const [asking, setAsking] = useState<CloudPreflight[]>([]);
  const [declined, setDeclined] = useState(false);
  const [serviceNames, setServiceNames] = useState<Record<string, string>>({});
  const [fallbacks, setFallbacks] = useState<FallbackChoices>(
    saved?.options.fallbacks ?? NO_FALLBACKS,
  );

  const options = useCallback(
    (): DubOptions => ({
      muteOriginal: mute,
      exportKind: saved?.options.exportKind ?? "video",
      exportName: saved?.options.exportName || baseName(path),
      quality,
      fallbacks,
    }),
    [mute, quality, saved, path, fallbacks],
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

  // The language list comes from the catalogue; non-commercial models count once opted in.
  const optInKey = fallbacks.optIn.join(",");
  useEffect(() => {
    languagesList(fallbacks.optIn)
      .then(setLanguages)
      .catch(() => setLanguages([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [optInKey]);

  useEffect(() => {
    cloudServices()
      .then((list) => setServiceNames(Object.fromEntries((list ?? []).map((s) => [s.id, s.name]))))
      .catch(() => setServiceNames({}));
  }, []);

  // What the chosen pair needs, shown before anything is processed.
  useEffect(() => {
    languagesCheckPair(source, target, fallbacks.optIn)
      .then(setPair)
      .catch(() => setPair(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, target, optInKey]);

  // A different pair needs different fixes: start from none (but keep what the project saved
  // when the screen first opens).
  const firstPair = useRef(true);
  useEffect(() => {
    if (firstPair.current) {
      firstPair.current = false;
      return;
    }
    setFallbacks({ ...NO_FALLBACKS });
  }, [source, target]);

  // Re-check when any choice changes and a file is chosen.
  useEffect(() => {
    if (path && !readOnly) void check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, source, target, mute, quality, sameOk, fallbacks]);

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

  const go = async () => {
    setBusy(true);
    const runId = await attempt(() => dubStart(handle));
    setBusy(false);
    if (runId) onStarted(runId);
  };

  const start = async () => {
    setDeclined(false);
    const need = (pre?.cloud ?? []).filter((c) => c.needsConsent);
    // Nothing is sent before the notice has been read and agreed to.
    if (need.length > 0) return setAsking(need);
    await go();
  };

  const agreed = () => {
    const rest = asking.slice(1);
    setAsking(rest);
    if (rest.length === 0) void go();
  };

  const declinedNow = () => {
    setAsking([]);
    setDeclined(true);
  };

  const sameLanguage = source !== "auto" && source === target && !sameOk;
  // Language limits are shown by the pair panel; only machine limits are listed separately.
  const machineBlockers = (pre?.blockers ?? []).filter((b) => b.reason === "machine");
  const cloud = pre?.cloud ?? [];
  const missingKey = cloud.find((c) => !c.hasKey);
  const ready =
    !!pre &&
    !missingKey &&
    machineBlockers.length === 0 &&
    pairResolved(pair, fallbacks) &&
    pre.missingModels.length === 0 &&
    !sameLanguage &&
    !downloading;
  const err = error
    ? dubError(error as AppErrorShape & { details?: Record<string, unknown> })
    : null;
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
              <LanguagePicker
                id="dub-source-language"
                languages={languages}
                value={source}
                onChange={setSource}
                role="source"
                allowAuto
                disabled={readOnly}
              />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="dub-target-language">{t("dub.new.targetLanguage")}</Label>
              <LanguagePicker
                id="dub-target-language"
                languages={languages}
                value={target}
                onChange={setTarget}
                role="target"
                disabled={readOnly}
              />
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

      <PairCheck
        check={pair}
        choices={fallbacks}
        onChoices={setFallbacks}
        readOnly={readOnly}
        serviceNames={serviceNames}
      />

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

      {cloud.length > 0 && (
        <ul className="flex flex-col gap-1 text-sm" data-testid="cloud-amounts">
          {cloud.map((c) => (
            <li key={c.step}>
              {t("cloud.amount", {
                step: t(`cloud.stepLabel.${c.step}`),
                chars: roundChars(c.estimatedChars),
                service: c.serviceName,
              })}
            </li>
          ))}
        </ul>
      )}

      {missingKey && (
        <Alert variant="destructive" role="alert">
          <AlertDescription>
            {t("dub.new.cloudNoKey", {
              service: missingKey.serviceName,
              step: t(`cloud.stepLabel.${missingKey.step}`),
            })}
          </AlertDescription>
        </Alert>
      )}

      {declined && (
        <Alert role="status">
          <AlertDescription>{t("cloud.declined")}</AlertDescription>
        </Alert>
      )}

      {pre?.warnings.map((w) => (
        <Alert key={w}>
          <AlertDescription>{t(w)}</AlertDescription>
        </Alert>
      ))}

      {machineBlockers.map((b) => (
        <Alert key={`${b.step}-${b.reason}`} variant="destructive" role="alert">
          <AlertDescription>
            {t(b.reason === "machine" ? "dub.blocker.machine" : "dub.blocker.language", {
              step: b.step,
              detail: b.detail,
            })}
          </AlertDescription>
        </Alert>
      ))}

      {pre && (
        <ModelPrompt
          models={pre.missingModels}
          disabled={readOnly}
          onDownloading={setDownloading}
          onInstalled={() => void checkRef.current()}
          onError={(e) => setError(asAppError(e))}
        />
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
      <ConsentDialog item={asking[0] ?? null} onAgree={agreed} onDecline={declinedNow} />
    </section>
  );
}

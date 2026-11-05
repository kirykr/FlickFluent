import { Check, Circle, X } from "lucide-react";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress as Bar } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { etaMinutes, formatClock, nextAction, percent, STAGE_KEYS } from "./format";
import { languageName } from "./languages";
import { RunError } from "./RunError";
import type { RunView } from "./useDubRun";
import type { Stage } from "./types";

const ORDER: Stage[] = ["extract", "separate", "transcribe", "translate", "synthesize", "mix"];

interface Props {
  handle: string;
  view: RunView;
  onCancel: () => void;
  onResume: () => void;
  onRestart: () => void;
  /** The detected language equals the target and the user agrees to go on. */
  onConfirmSame: () => void;
  /** "Cloud: <service>" for each stage that runs on a cloud service. */
  cloudLabels?: Partial<Record<Stage, string>>;
}

/** Lines shown while the run goes on; the full list is in the result. */
const LIVE_LINES = 6;

export function RunProgress({
  handle,
  view,
  onCancel,
  onResume,
  onRestart,
  onConfirmSame,
  cloudLabels = {},
}: Props) {
  // A run recorded as running with nothing running was cut short (the app closed or crashed).
  const interrupted = !!view.status?.interrupted && view.runId === null;
  const running =
    !interrupted &&
    (view.state === "running" || view.state === "queued" || view.state === "cancelling");
  const overall = view.progress?.overallProgress ?? 0;
  const eta = etaMinutes(view.progress?.etaSeconds ?? null);
  const failedHere = view.error ? nextAction(view.error.nextStepKey) : null;
  const live = [...view.lines].sort((x, y) => x.startMs - y.startMs).slice(-LIVE_LINES);
  const canResume = view.state === "cancelled" || view.state === "failed" || interrupted;

  return (
    <Card className="mx-auto w-full max-w-2xl" data-testid="dub-progress">
      <CardHeader>
        <CardTitle>
          {view.state === "cancelled"
            ? t("dub.run.stopped")
            : view.state === "failed"
              ? t("dub.run.failed")
              : interrupted
                ? t("dub.run.interrupted")
                : t("dub.run.working")}
        </CardTitle>
        {running && (
          <CardDescription>
            {view.progress && view.progress.sectionCount > 0
              ? t("dub.run.section", {
                  n: view.progress.sectionIndex + 1,
                  total: view.progress.sectionCount,
                })
              : t("dub.run.preparing")}
            {eta !== null && ` · ${t("dub.run.eta", { minutes: eta })}`}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {view.detected && (
          <p className="text-sm" data-testid="detected-language">
            {t("languages.detected", { language: languageName(view.detected) || view.detected })}
          </p>
        )}
        <Bar value={percent(overall)} aria-label={t("dub.run.overall")} />
        <ul className="flex flex-col gap-1 text-sm">
          {ORDER.map((stage) => {
            const status = view.stageStatus[stage];
            return (
              <li
                key={stage}
                className="flex items-center gap-2"
                data-stage={stage}
                data-status={status ?? "pending"}
              >
                {status === "done" ? (
                  <Check className="size-4 text-primary" />
                ) : status === "running" ? (
                  <Spinner className="size-4" />
                ) : status === "failed" ? (
                  <X className="size-4 text-destructive" />
                ) : (
                  <Circle className="size-4 text-muted-foreground" />
                )}
                <span className={status === "pending" || !status ? "text-muted-foreground" : ""}>
                  {t(STAGE_KEYS[stage])}
                </span>
                {cloudLabels[stage] && (
                  <Badge variant="outline" data-testid="cloud-label">
                    {cloudLabels[stage]}
                  </Badge>
                )}
                {running && status === "running" && view.progress?.stage === stage && (
                  <span
                    className="ml-auto text-xs text-muted-foreground"
                    data-testid="stage-percent"
                  >
                    {percent(view.progress.stageProgress)}%
                  </span>
                )}
              </li>
            );
          })}
        </ul>

        {running && view.cloud && (
          <p className="text-sm text-muted-foreground" role="status" data-testid="cloud-note">
            {t(`cloud.note.${view.cloud.kind}`, {
              key: view.cloud.keyLabel ?? "",
              seconds: view.cloud.waitSeconds ?? 0,
              reason: view.cloud.reason ? t(`cloud.note.reason.${view.cloud.reason}`) : "",
            })}
          </p>
        )}

        {live.length > 0 && (
          <section aria-label={t("dub.run.linesTitle")} className="flex flex-col gap-1">
            <h3 className="text-xs font-medium text-muted-foreground">{t("dub.run.linesTitle")}</h3>
            <ul className="flex max-h-40 flex-col gap-1 overflow-y-auto text-sm">
              {live.map((l) => (
                <li key={l.id} className="flex gap-2" data-testid="live-line">
                  <span className="shrink-0 tabular-nums text-muted-foreground">
                    {formatClock(l.startMs)}
                  </span>
                  <span className="min-w-0">{l.translated ?? l.text}</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {view.error && (
          <RunError
            handle={handle}
            error={view.error}
            state={view.state}
            detected={view.detected}
            onResume={onResume}
            onRestart={onRestart}
            onConfirmSame={onConfirmSame}
          />
        )}

        <div className="flex justify-end gap-2">
          {running && view.runId && (
            <Button variant="outline" onClick={onCancel} disabled={view.state === "cancelling"}>
              {t("dub.run.cancel")}
            </Button>
          )}
          {!running && canResume && !view.error && (
            <Button onClick={onResume}>{t("dub.run.resume")}</Button>
          )}
          {!running && canResume && failedHere !== "restart" && failedHere !== "chooseAnother" && (
            <Button variant="outline" onClick={onRestart}>
              {t("dub.run.startOver")}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

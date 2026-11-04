import { Check, Circle, X } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { dubError, etaMinutes, percent, STAGE_KEYS } from "./format";
import type { RunView } from "./useDubRun";
import type { Stage } from "./types";

const ORDER: Stage[] = ["extract", "separate", "transcribe", "translate", "synthesize", "mix"];

interface Props {
  view: RunView;
  onCancel: () => void;
  onResume: () => void;
  onRestart: () => void;
}

export function RunProgress({ view, onCancel, onResume, onRestart }: Props) {
  // A run recorded as running with nothing running was cut short (the app closed or crashed).
  const interrupted = !!view.status?.interrupted && view.runId === null;
  const running =
    !interrupted &&
    (view.state === "running" || view.state === "queued" || view.state === "cancelling");
  view.state === "running" || view.state === "queued" || view.state === "cancelling";
  const overall = view.progress?.overallProgress ?? 0;
  const eta = etaMinutes(view.progress?.etaSeconds ?? null);
  const err = view.error
    ? dubError({
        code: view.error.code,
        messageKey: view.error.messageKey,
        details: view.error.details,
      })
    : null;
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
        <Progress value={percent(overall)} aria-label={t("dub.run.overall")} />
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
              </li>
            );
          })}
        </ul>

        {err && (
          <Alert
            variant={view.state === "cancelled" ? "default" : "destructive"}
            role="alert"
            data-testid="dub-run-error"
          >
            <AlertDescription>{t(err.key, err.params)}</AlertDescription>
          </Alert>
        )}

        <div className="flex justify-end gap-2">
          {running && view.runId && (
            <Button variant="outline" onClick={onCancel} disabled={view.state === "cancelling"}>
              {t("dub.run.cancel")}
            </Button>
          )}
          {!running && canResume && <Button onClick={onResume}>{t("dub.run.resume")}</Button>}
          {!running && canResume && (
            <Button variant="outline" onClick={onRestart}>
              {t("dub.run.startOver")}
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

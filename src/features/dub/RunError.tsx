import { useCallback, useEffect, useState } from "react";
import { t } from "../../i18n";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { dubPreflight } from "../../app/ipc";
import { dubError, nextAction, STAGE_KEYS } from "./format";
import { languageName } from "./languages";
import { ModelPrompt } from "./ModelPrompt";
import type { MissingModel, RunError as RunErrorData, RunState } from "./types";

interface Props {
  handle: string;
  error: RunErrorData;
  state: RunState | null;
  detected: string | null;
  onResume: () => void;
  onRestart: () => void;
  onConfirmSame: () => void;
}

/** What went wrong in plain words, what to do next, and a button that does it. */
export function RunError({
  handle,
  error,
  state,
  detected,
  onResume,
  onRestart,
  onConfirmSame,
}: Props) {
  const stage = error.stage ?? ((error.details as { stage?: string } | null)?.stage as never);
  const stageName = stage && STAGE_KEYS[stage as keyof typeof STAGE_KEYS];
  const shown = dubError(
    { code: error.code, messageKey: error.messageKey, details: error.details },
    stageName ? t(stageName) : null,
  );
  const detail = (error.details as { message?: string } | null | undefined)?.message;
  const tail = (error.details as { stderrTail?: unknown } | null | undefined)?.stderrTail;
  const engineLog = Array.isArray(tail) ? tail.filter((l) => typeof l === "string").join("\n") : "";
  const action = nextAction(error.nextStepKey);
  const [missing, setMissing] = useState<MissingModel[]>([]);

  const load = useCallback(async () => {
    try {
      setMissing((await dubPreflight(handle)).missingModels);
    } catch {
      setMissing([]);
    }
  }, [handle]);
  useEffect(() => {
    if (action === "download") void load();
  }, [action, load]);

  const soft = state === "cancelled";
  const waitingForModels = action === "download" && missing.length > 0;

  return (
    <div className="flex flex-col gap-3" data-testid="dub-run-error">
      <Alert variant={soft ? "default" : "destructive"} role="alert">
        <AlertDescription>
          {t(shown.key, shown.params)}
          {detail && <span className="mt-1 block text-xs opacity-70">{detail}</span>}
          <span className="mt-1 block">{t(error.nextStepKey)}</span>
          {engineLog && (
            <Collapsible className="mt-2" data-testid="engine-log">
              <CollapsibleTrigger className="text-xs underline">
                {t("dub.run.engineLog")}
              </CollapsibleTrigger>
              <CollapsibleContent>
                <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap text-xs select-text">
                  {engineLog}
                </pre>
              </CollapsibleContent>
            </Collapsible>
          )}
        </AlertDescription>
      </Alert>

      {error.code === "SAME_LANGUAGE" && (
        <Alert data-testid="same-detected">
          <AlertDescription className="flex items-center gap-3">
            {t("languages.sameDetected", {
              language: languageName(detected ?? "") || detected || "",
            })}
            <Button size="sm" variant="outline" onClick={onConfirmSame}>
              {t("dub.new.sameConfirm")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {waitingForModels && <ModelPrompt models={missing} onInstalled={() => void load()} />}

      <div className="flex justify-end gap-2">
        {(action === "resume" || (action === "download" && !waitingForModels)) && (
          <Button onClick={onResume}>{soft ? t("dub.run.resume") : t("dub.run.tryAgain")}</Button>
        )}
        {action === "chooseAnother" && error.code === "STAGE_FAILED" && (
          <Button variant="outline" onClick={onResume}>
            {t("dub.run.tryAgain")}
          </Button>
        )}
        {action === "chooseAnother" && (
          <Button onClick={onRestart}>{t("dub.run.chooseAnother")}</Button>
        )}
        {action === "restart" && <Button onClick={onRestart}>{t("dub.run.startOver")}</Button>}
      </div>
    </div>
  );
}

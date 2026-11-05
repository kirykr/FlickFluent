import { useState } from "react";
import { t } from "../../i18n";
import { dubCancel, dubConfigure, dubDiscardRun, dubResume } from "../../app/ipc";
import { asAppError } from "../projects/errors";
import type { AppErrorShape } from "../projects/types";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { NewDub } from "./NewDub";
import { Result } from "./Result";
import { RunProgress } from "./RunProgress";
import { dubError } from "./format";
import { useDubRun } from "./useDubRun";

interface Props {
  handle: string;
  readOnly: boolean;
}

/** One project's dub: set it up, watch it run, then read the transcript and export. */
export function DubScreen({ handle, readOnly }: Props) {
  const { view, follow, refresh } = useDubRun(handle);
  const [error, setError] = useState<AppErrorShape | null>(null);
  const [fresh, setFresh] = useState(false);

  const guard = async (fn: () => Promise<unknown>) => {
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(asAppError(e));
    }
  };

  const status = view.status;
  if (!status) return null;
  const state = view.state;
  const finished = state === "completed" && !status.running;
  const active =
    status.running || state === "running" || state === "cancelling" || state === "queued";
  const unfinished =
    !fresh && (active || state === "cancelled" || state === "failed" || status.interrupted);
  const err = error ? dubError(error) : null;

  const errorBanner = err && (
    <Alert variant="destructive" role="alert">
      <AlertDescription>{t(err.key, err.params)}</AlertDescription>
    </Alert>
  );

  if (finished && !fresh) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-3">
        {errorBanner}
        <Result
          handle={handle}
          dub={status.dub}
          lines={
            view.lines.length
              ? [...view.lines].sort((a, b) => a.startMs - b.startMs)
              : status.dub.lines
          }
          noSpeech={status.run?.error?.code === "NO_SPEECH_FOUND"}
          detected={view.detected ?? status.dub.detectedLanguage}
          hasDubAudio={status.hasDubAudio}
        />
      </div>
    );
  }

  if (unfinished) {
    return (
      <div className="flex flex-col gap-3">
        {errorBanner}
        <RunProgress
          view={view}
          onCancel={() => view.runId && guard(() => dubCancel(view.runId as string))}
          onResume={() =>
            guard(async () => {
              follow(await dubResume(handle));
            })
          }
          onConfirmSame={() =>
            guard(async () => {
              const d = status.dub;
              if (!d.source) return;
              await dubConfigure(
                handle,
                d.source.path,
                d.sourceLanguage,
                d.targetLanguage,
                d.options,
                true,
              );
              follow(await dubResume(handle));
            })
          }
          onRestart={() =>
            guard(async () => {
              await dubDiscardRun(handle);
              setFresh(true);
              await refresh();
            })
          }
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {errorBanner}
      <NewDub
        handle={handle}
        saved={status.dub.source ? status.dub : null}
        readOnly={readOnly}
        onStarted={(id) => {
          setFresh(false);
          follow(id);
        }}
      />
    </div>
  );
}

import type { ReactNode } from "react";
import { AudioTimeline } from "./AudioTimeline";
import { PreviewMonitor } from "./PreviewMonitor";
import { ToolPanel } from "./ToolPanel";
import { TranscriptionEditor } from "./TranscriptionEditor";

/** The editor arrangement from the design: tools, preview, transcription, timeline. */
export function EditorLayout({ header, main }: { header: ReactNode; main?: ReactNode }) {
  if (main) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-3">
        {header}
        <div className="min-h-0 flex-1 overflow-auto">{main}</div>
      </div>
    );
  }
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {header}
      <div className="grid min-h-0 flex-1 grid-cols-[15rem_19rem_1fr] gap-3">
        <ToolPanel />
        <PreviewMonitor />
        <TranscriptionEditor />
      </div>
      <AudioTimeline />
    </div>
  );
}

import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import { subtitlesExport, subtitlesImportPreview } from "../../app/ipc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { asAppError } from "../projects/errors";
import type { AppErrorShape } from "../projects/types";
import { formatMs } from "./format";
import { useEditorContext } from "./EditorContext";
import type { ImportChoice, ImportPreview, SubtitleFormat, SubtitleSource } from "./types";

interface Props {
  open: boolean;
  tab: "export" | "import";
  onOpenChange: (open: boolean) => void;
}

function errorText(e: AppErrorShape): string {
  const key = (e as { messageKey?: string }).messageKey;
  const params = (e.details ?? {}) as Record<string, string>;
  return t(key && key.startsWith("editor.") ? key : "editor.loadFailed", params);
}

/** Save the lines as SRT or WebVTT, or read a subtitle file and choose what to do with it. */
export function SubtitlesDialog({ open: isOpen, tab: first, onOpenChange }: Props) {
  const ctx = useEditorContext();
  const [tab, setTab] = useState(first);
  const [source, setSource] = useState<SubtitleSource>("translated");
  const [format, setFormat] = useState<SubtitleFormat>("srt");
  const [saved, setSaved] = useState<number | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [choice, setChoice] = useState<ImportChoice | null>(null);
  const [applied, setApplied] = useState(false);
  const [error, setError] = useState<AppErrorShape | null>(null);

  useEffect(() => {
    if (isOpen) {
      setTab(first);
      setSaved(null);
      setPreview(null);
      setChoice(null);
      setApplied(false);
      setError(null);
    }
  }, [isOpen, first]);

  if (!ctx) return null;
  const { handle, ed, readOnly } = ctx;

  const guard = async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
    setError(null);
    try {
      return await fn();
    } catch (e) {
      setError(asAppError(e));
      return undefined;
    }
  };

  const sources: { value: SubtitleSource; label: string }[] = [
    { value: "translated", label: t("editor.subtitles.sourceTranslated") },
    { value: "original", label: t("editor.subtitles.sourceOriginal") },
    { value: "both", label: t("editor.subtitles.sourceBoth") },
  ];
  const formats: { value: SubtitleFormat; label: string }[] = [
    { value: "srt", label: t("editor.subtitles.formatSrt") },
    { value: "vtt", label: t("editor.subtitles.formatVtt") },
  ];

  const doExport = async () => {
    const path = await save({
      defaultPath: `subtitles.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (typeof path !== "string") return;
    const r = await guard(() => subtitlesExport(handle, source, format, path));
    if (r) setSaved(r.cueCount);
  };

  const choose = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "SRT, WebVTT", extensions: ["srt", "vtt"] }],
    });
    if (typeof picked !== "string") return;
    setApplied(false);
    const p = await guard(() => subtitlesImportPreview(handle, picked));
    if (p) {
      setPreview(p);
      setChoice(p.choices[0] ?? null);
    }
  };

  const apply = async () => {
    if (!preview || !choice) return;
    const r = await guard(() => ed.importApply(preview.previewId, choice));
    if (r) {
      setApplied(true);
      setPreview(null);
    }
  };

  const describe = (d: ImportPreview["differences"][number]) =>
    d.kind === "time"
      ? t("editor.subtitles.diffTime", {
          n: d.index,
          cue: formatMs(d.cueStartMs ?? 0),
          line: formatMs(d.lineStartMs ?? 0),
        })
      : d.kind === "missingCue"
        ? t("editor.subtitles.diffMissing", { n: d.index })
        : t("editor.subtitles.diffExtra", { n: d.index });

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl" data-testid="subtitles-dialog">
        <DialogHeader>
          <DialogTitle>{t("editor.subtitles.title")}</DialogTitle>
          <DialogDescription>
            {t("editor.tool.exportSubtitles")} / {t("editor.tool.importSubtitles")}
          </DialogDescription>
        </DialogHeader>
        {error && (
          <Alert variant="destructive" role="alert" data-testid="subtitles-error">
            <AlertDescription>{errorText(error)}</AlertDescription>
          </Alert>
        )}
        <Tabs value={tab} onValueChange={(v) => setTab(v as "export" | "import")}>
          <TabsList>
            <TabsTrigger value="export">{t("editor.subtitles.export")}</TabsTrigger>
            <TabsTrigger value="import">{t("editor.subtitles.import")}</TabsTrigger>
          </TabsList>

          <TabsContent value="export" className="flex flex-col gap-3">
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-source">{t("editor.subtitles.source")}</Label>
                <Select
                  items={sources}
                  value={source}
                  onValueChange={(v) => v && setSource(v as SubtitleSource)}
                >
                  <SelectTrigger id="sub-source">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {sources.map((s) => (
                      <SelectItem key={s.value} value={s.value}>
                        {s.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-format">{t("editor.subtitles.format")}</Label>
                <Select
                  items={formats}
                  value={format}
                  onValueChange={(v) => v && setFormat(v as SubtitleFormat)}
                >
                  <SelectTrigger id="sub-format">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {formats.map((f) => (
                      <SelectItem key={f.value} value={f.value}>
                        {f.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <p className="text-xs text-muted-foreground" data-testid="format-note">
              {format === "srt" ? t("editor.subtitles.srtNote") : t("editor.subtitles.vttNote")}
            </p>
            <div className="flex items-center gap-3">
              <Button onClick={doExport}>{t("editor.subtitles.save")}</Button>
              {saved !== null && (
                <span className="text-sm" data-testid="export-count">
                  {t("editor.subtitles.saved", { count: saved })}
                </span>
              )}
            </div>
          </TabsContent>

          <TabsContent value="import" className="flex flex-col gap-3">
            <div className="flex items-center gap-3">
              <Button variant="outline" disabled={readOnly} onClick={choose}>
                {t("editor.subtitles.chooseFile")}
              </Button>
              {!preview && !applied && (
                <span className="text-xs text-muted-foreground">{t("editor.subtitles.none")}</span>
              )}
            </div>
            {applied && (
              <Alert data-testid="import-applied">
                <AlertDescription>{t("editor.subtitles.applied")}</AlertDescription>
              </Alert>
            )}
            {preview && (
              <div className="flex flex-col gap-3" data-testid="import-preview">
                <p className="text-sm">
                  {t("editor.subtitles.summary", {
                    cues: preview.cueCount,
                    lines: preview.lineCount,
                    encoding: preview.encoding,
                  })}
                </p>
                {preview.allMatch ? (
                  <p className="text-sm text-muted-foreground">{t("editor.subtitles.allMatch")}</p>
                ) : (
                  <div className="text-sm">
                    <p className="font-medium">
                      {t("editor.subtitles.differences", { count: preview.differences.length })}
                    </p>
                    <ul className="max-h-28 overflow-auto text-xs text-muted-foreground">
                      {preview.differences.map((d) => (
                        <li key={`${d.kind}${d.index}`}>{describe(d)}</li>
                      ))}
                    </ul>
                  </div>
                )}
                <div className="flex flex-col gap-1">
                  <Label>{t("editor.subtitles.choice")}</Label>
                  <ToggleGroup
                    orientation="vertical"
                    spacing={1}
                    value={choice ? [choice] : []}
                    onValueChange={(v) => v[0] && setChoice(v[0] as ImportChoice)}
                    className="flex-col items-stretch"
                  >
                    {preview.choices.map((c) => (
                      <ToggleGroupItem
                        key={c}
                        value={c}
                        className="h-auto justify-start py-2 text-left whitespace-normal"
                      >
                        <span className="flex flex-col">
                          <span>{t(`editor.subtitles.${c}`)}</span>
                          <span className="text-xs text-muted-foreground">
                            {t(`editor.subtitles.${c}Hint`)}
                          </span>
                        </span>
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                </div>
                <Button disabled={!choice || readOnly} onClick={apply}>
                  {t("editor.subtitles.apply")}
                </Button>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

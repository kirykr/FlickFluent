import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { dubExport } from "../../app/ipc";
import { UsageList } from "../cloud/UsageList";
import { asAppError } from "../projects/errors";
import { formatBytes } from "../models/format";
import { baseName, dubError, FLAG_KEYS, fileName, formatClock } from "./format";
import { languageName } from "./languages";
import type { DubData, ExportKind, ExportResult, TranscriptLine } from "./types";

interface Props {
  handle: string;
  dub: DubData;
  lines: TranscriptLine[];
  noSpeech: boolean;
  detected: string | null;
  /** The run made dubbed sound; without it only subtitles can be saved. */
  hasDubAudio: boolean;
}

export function Result({ handle, dub, lines, noSpeech, detected, hasDubAudio }: Props) {
  // What the work folder holds decides, not a setting that may have been saved later or lost.
  const subtitlesOnly = !hasDubAudio;
  const [kind, setKind] = useState<ExportKind>(
    subtitlesOnly ? "subtitles" : dub.options.exportKind || "video",
  );
  const [name, setName] = useState(dub.options.exportName || baseName(dub.source?.path ?? "dub"));
  const [folder, setFolder] = useState<string | null>(null);
  const [overwrite, setOverwrite] = useState(false);
  const [done, setDone] = useState<ExportResult | null>(null);
  const [error, setError] = useState<{
    code: string;
    details?: Record<string, unknown> | null;
    messageKey?: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);

  // A run that did not speak has nothing to play: its result is the subtitles.
  const kinds: { value: ExportKind; label: string }[] = subtitlesOnly
    ? [{ value: "subtitles", label: t("dub.result.kindSubtitles") }]
    : [
        { value: "video", label: t("dub.result.kindVideo") },
        { value: "audio", label: t("dub.result.kindAudio") },
        { value: "subtitles", label: t("dub.result.kindSubtitles") },
      ];

  // The chosen kind may not be on offer (the list follows what the run made): use the first.
  const activeKind = kinds.some((k) => k.value === kind) ? kind : kinds[0].value;

  const pickFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setFolder(picked);
  };

  const run = async (force = false) => {
    if (!folder) return;
    setBusy(true);
    setError(null);
    setDone(null);
    try {
      setDone(await dubExport(handle, activeKind, name.trim(), folder, force || overwrite));
      setOverwrite(false);
    } catch (e) {
      const err = asAppError(e);
      setError(err);
      if (err.code === "FILE_EXISTS") setOverwrite(false);
    } finally {
      setBusy(false);
    }
  };

  const err = error ? dubError(error) : null;
  const speakerLabel = (id: string) => id.replace(/^spk/, t("dub.result.speaker") + " ");

  return (
    <div className="flex min-h-0 flex-col gap-3" data-testid="dub-result">
      <Card size="sm">
        <CardHeader>
          <CardTitle>{t("dub.result.exportTitle")}</CardTitle>
          <CardDescription>
            {t("dub.result.exportBody")}
            {detected && (
              <span className="ml-2" data-testid="result-language">
                {t("languages.detected", { language: languageName(detected) || detected })}
              </span>
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="export-kind">{t("dub.result.kind")}</Label>
            <Select
              items={kinds}
              value={activeKind}
              onValueChange={(v) => v && setKind(v as ExportKind)}
            >
              <SelectTrigger id="export-kind" className="w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {kinds.map((k) => (
                  <SelectItem key={k.value} value={k.value}>
                    {k.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex min-w-48 flex-1 flex-col gap-1">
            <Label htmlFor="export-name">{t("dub.result.fileName")}</Label>
            <Input id="export-name" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <Button variant="outline" onClick={pickFolder}>
            {folder ? baseName(folder) || folder : t("dub.result.chooseFolder")}
          </Button>
          <Button disabled={!folder || !name.trim() || busy} onClick={() => run()}>
            {t("dub.result.export")}
          </Button>
        </CardContent>
      </Card>
      <UsageList usage={dub.cloudUsage ?? []} />

      {err && (
        <Alert variant="destructive" role="alert" data-testid="export-error">
          <AlertDescription className="flex items-center gap-3">
            {t(err.key, err.params)}
            {error?.code === "FILE_EXISTS" && (
              <Button size="sm" variant="outline" onClick={() => run(true)}>
                {t("dub.result.overwrite")}
              </Button>
            )}
          </AlertDescription>
        </Alert>
      )}

      {done && (
        <Alert data-testid="export-done">
          <AlertDescription className="flex items-center gap-3">
            <span className="truncate">
              {t("dub.result.saved", {
                name: fileName(done.path),
                size: formatBytes(done.sizeBytes),
              })}
            </span>
            <Button size="sm" variant="outline" onClick={() => revealItemInDir(done.path)}>
              {t("dub.result.openFolder")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {noSpeech && (
        <Alert>
          <AlertDescription>{t("dub.error.noSpeechFound")}</AlertDescription>
        </Alert>
      )}

      <div
        className="min-h-0 flex-1 overflow-auto rounded-md border"
        data-testid="transcript-table"
      >
        <Table className="table-fixed">
          <TableHeader>
            <TableRow>
              <TableHead className="w-16">{t("dub.result.start")}</TableHead>
              <TableHead className="w-16">{t("dub.result.end")}</TableHead>
              <TableHead className="w-24">{t("dub.result.speaker")}</TableHead>
              <TableHead className="w-[34%]">{t("dub.result.original")}</TableHead>
              <TableHead className="w-[34%]">{t("dub.result.translated")}</TableHead>
              <TableHead>{t("dub.result.flags")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.length === 0 && (
              <TableRow>
                <TableCell colSpan={6} className="text-muted-foreground">
                  {t("dub.result.empty")}
                </TableCell>
              </TableRow>
            )}
            {lines.map((l) => (
              <TableRow key={l.id} data-line={l.id}>
                <TableCell className="tabular-nums">{formatClock(l.startMs)}</TableCell>
                <TableCell className="tabular-nums">{formatClock(l.endMs)}</TableCell>
                <TableCell>{speakerLabel(l.speakerId)}</TableCell>
                <TableCell className="whitespace-normal break-words align-top">{l.text}</TableCell>
                <TableCell className="whitespace-normal break-words align-top">
                  {l.translated ?? ""}
                </TableCell>
                <TableCell className="whitespace-normal">
                  <span className="flex flex-wrap gap-1">
                    {l.flags.map((f) => (
                      <Badge key={f} variant="secondary">
                        {t(FLAG_KEYS[f])}
                      </Badge>
                    ))}
                  </span>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

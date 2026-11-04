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
import { asAppError } from "../projects/errors";
import { formatBytes } from "../models/format";
import { baseName, dubError, FLAG_KEYS, fileName, formatClock } from "./format";
import type { DubData, ExportKind, ExportResult, TranscriptLine } from "./types";

interface Props {
  handle: string;
  dub: DubData;
  lines: TranscriptLine[];
  noSpeech: boolean;
}

export function Result({ handle, dub, lines, noSpeech }: Props) {
  const [kind, setKind] = useState<ExportKind>(dub.options.exportKind || "video");
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
      setDone(await dubExport(handle, kind, name.trim(), folder, force || overwrite));
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
          <CardDescription>{t("dub.result.exportBody")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="export-kind">{t("dub.result.kind")}</Label>
            <Select
              items={[
                { value: "video", label: t("dub.result.kindVideo") },
                { value: "audio", label: t("dub.result.kindAudio") },
              ]}
              value={kind}
              onValueChange={(v) => v && setKind(v as ExportKind)}
            >
              <SelectTrigger id="export-kind" className="w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="video">{t("dub.result.kindVideo")}</SelectItem>
                <SelectItem value="audio">{t("dub.result.kindAudio")}</SelectItem>
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
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-20">{t("dub.result.start")}</TableHead>
              <TableHead className="w-20">{t("dub.result.end")}</TableHead>
              <TableHead className="w-24">{t("dub.result.speaker")}</TableHead>
              <TableHead>{t("dub.result.original")}</TableHead>
              <TableHead>{t("dub.result.translated")}</TableHead>
              <TableHead className="w-40">{t("dub.result.flags")}</TableHead>
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
                <TableCell className="whitespace-normal">{l.text}</TableCell>
                <TableCell className="whitespace-normal">{l.translated ?? ""}</TableCell>
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

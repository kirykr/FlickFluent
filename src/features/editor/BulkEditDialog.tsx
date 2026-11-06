import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import type { TranscriptLine } from "../dub/types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  lines: TranscriptLine[];
  onApply: (texts: { id: string; translated: string }[]) => Promise<unknown>;
}

const SEP = " | ";

/** Read `id | text` rows. Rows with an unknown id or no separator are counted as unreadable. */
export function parseRows(
  text: string,
  lines: TranscriptLine[],
): { changes: { id: string; translated: string }[]; unreadable: number } {
  const known = new Map(lines.map((l) => [l.id, l.translated ?? ""]));
  const changes: { id: string; translated: string }[] = [];
  let unreadable = 0;
  for (const row of text.split("\n")) {
    if (!row.trim()) continue;
    const at = row.indexOf("|");
    const id = at < 0 ? "" : row.slice(0, at).trim();
    if (!known.has(id)) {
      unreadable += 1;
      continue;
    }
    const translated = row
      .slice(at + 1)
      .replace(/^ /, "")
      .trimEnd();
    if (translated !== known.get(id)) changes.push({ id, translated });
  }
  return { changes, unreadable };
}

/** All translations as one text, one row each, with a count of what would change before it does. */
export function BulkEditDialog({ open, onOpenChange, lines, onApply }: Props) {
  const [text, setText] = useState("");
  useEffect(() => {
    if (open) setText(lines.map((l) => `${l.id}${SEP}${l.translated ?? ""}`).join("\n"));
    // The text is taken once, when the dialog opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
  const { changes, unreadable } = open ? parseRows(text, lines) : { changes: [], unreadable: 0 };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl" data-testid="bulk-dialog">
        <DialogHeader>
          <DialogTitle>{t("editor.bulk.title")}</DialogTitle>
          <DialogDescription>{t("editor.bulk.hint")}</DialogDescription>
        </DialogHeader>
        <Textarea
          className="h-72 font-mono text-xs"
          aria-label={t("editor.bulk.title")}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <p className="text-sm" data-testid="bulk-count">
          {t("editor.bulk.changes", { count: changes.length })}
          {unreadable > 0 && ` ${t("editor.bulk.bad", { count: unreadable })}`}
        </p>
        <DialogFooter>
          <Button
            disabled={changes.length === 0}
            onClick={async () => {
              await onApply(changes);
              onOpenChange(false);
            }}
          >
            {t("editor.bulk.apply", { count: changes.length })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

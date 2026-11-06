import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Check,
  EllipsisVertical,
  FileText,
  ListChecks,
  Plus,
  Redo2,
  Search,
  Square,
  Trash2,
  Undo2,
  Volume2,
} from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Progress as Bar } from "@/components/ui/progress";
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
import { Textarea } from "@/components/ui/textarea";
import { FLAG_KEYS } from "../dub/format";
import type { TranscriptLine } from "../dub/types";
import type { AppErrorShape } from "../projects/types";
import { formatMs, parseMs } from "./format";
import { MemoryBadge } from "../status/MemoryBadge";
import { BulkEditDialog } from "./BulkEditDialog";
import { SearchReplace } from "./SearchReplace";
import { SpeakerPanel } from "./SpeakerPanel";
import type { LineEdit, Speaker } from "./types";
import { useEditorContext } from "./EditorContext";

const COLUMNS = 9;

function errorKey(e: AppErrorShape): string {
  const key = (e as { messageKey?: string }).messageKey;
  return key && /^(editor|dub|error)\./.test(key) ? key : "editor.loadFailed";
}

interface RowProps {
  line: TranscriptLine;
  n: number;
  speakers: Speaker[];
  selected: boolean;
  changed: boolean;
  needsPost: boolean;
  disabled: boolean;
  onSelect: (on: boolean) => void;
  onEdit: (edit: LineEdit) => Promise<unknown>;
  onMerge: () => void;
  onSplit: (offset: number) => void;
  onDuplicate: () => void;
  canMerge: boolean;
}

/** Cells commit on leaving the field; an unreadable value goes back to what the line has. */
function LineRow({
  line,
  n,
  speakers,
  selected,
  changed,
  needsPost,
  disabled,
  onSelect,
  onEdit,
  onMerge,
  onSplit,
  onDuplicate,
  canMerge,
}: RowProps) {
  // Where the cursor is in the translation, for "split here".
  const caret = useRef(0);
  const time = (field: "startMs" | "endMs", label: string) => (
    <Input
      key={`${field}${line[field]}`}
      className="h-8 w-24 font-mono text-xs"
      aria-label={t(label, { n })}
      defaultValue={formatMs(line[field])}
      disabled={disabled}
      onBlur={(e) => {
        const ms = parseMs(e.target.value);
        if (ms === null) e.target.value = formatMs(line[field]);
        else if (ms !== line[field]) {
          // A refused time goes back to what the line has.
          const input = e.target;
          void onEdit({ id: line.id, [field]: ms }).then((r) => {
            if (!r) input.value = formatMs(line[field]);
          });
        }
      }}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
    />
  );
  const translated = line.translated ?? "";
  return (
    <>
      <TableCell>
        <Checkbox
          checked={selected}
          aria-label={t("editor.selectLine", { n })}
          onCheckedChange={(c) => onSelect(c === true)}
        />
      </TableCell>
      <TableCell>{time("startMs", "editor.editStart")}</TableCell>
      <TableCell>{time("endMs", "editor.editEnd")}</TableCell>
      <TableCell>
        <Select
          items={speakers.map((s) => ({ value: s.id, label: s.id }))}
          value={line.speakerId}
          disabled={disabled}
          onValueChange={(v) => v && v !== line.speakerId && onEdit({ id: line.id, speakerId: v })}
        >
          <SelectTrigger className="h-8 w-24" aria-label={t("editor.pickSpeaker", { n })}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {speakers.map((s) => (
              <SelectItem key={s.id} value={s.id}>
                {s.id}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </TableCell>
      <TableCell className="whitespace-normal text-xs text-muted-foreground">{line.text}</TableCell>
      <TableCell className="whitespace-normal">
        <Textarea
          key={translated}
          className="min-h-8 resize-none text-sm"
          rows={2}
          aria-label={t("editor.editText", { n })}
          defaultValue={translated}
          disabled={disabled}
          onSelect={(e) => (caret.current = e.currentTarget.selectionStart)}
          onKeyUp={(e) => (caret.current = e.currentTarget.selectionStart)}
          onClick={(e) => (caret.current = e.currentTarget.selectionStart)}
          onBlur={(e) => {
            // A menu click takes the focus away; the cursor is still where it was.
            caret.current = e.target.selectionStart;
            if (e.target.value !== translated)
              void onEdit({ id: line.id, translated: e.target.value });
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              e.currentTarget.blur();
            } else if (e.key === "Escape") {
              e.currentTarget.value = translated;
              e.currentTarget.blur();
            }
          }}
        />
      </TableCell>
      <TableCell className="whitespace-normal">
        <span className="flex flex-wrap gap-1">
          {line.flags.map((f) => (
            <Badge key={f} variant="outline">
              {t(FLAG_KEYS[f])}
            </Badge>
          ))}
        </span>
      </TableCell>
      <TableCell>
        {changed ? (
          <Badge data-testid="changed-badge">{t("editor.changed")}</Badge>
        ) : needsPost ? (
          <Badge variant="secondary" data-testid="changed-badge">
            {t("editor.needsRemake")}
          </Badge>
        ) : line.audio ? (
          <Volume2 className="size-4 text-muted-foreground" aria-label={t("editor.madeOk")} />
        ) : (
          <span className="text-xs text-muted-foreground">{t("editor.noSpeech")}</span>
        )}
      </TableCell>
      <TableCell>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                size="icon"
                variant="ghost"
                disabled={disabled}
                aria-label={t("editor.rowMenu", { n })}
              />
            }
          >
            <EllipsisVertical />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem disabled={!canMerge} onClick={onMerge}>
              {t("editor.mergeNext")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => onSplit(caret.current)}>
              {t("editor.splitCaret")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={onDuplicate}>{t("editor.duplicate")}</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </TableCell>
    </>
  );
}

/** Right column of the editor: a table of the dub's lines that can be edited and made again. */
export function TranscriptionEditor() {
  const ctx = useEditorContext();
  return ctx ? <Editor /> : <EmptyEditor />;
}

function EmptyEditor() {
  return (
    <Card className="h-full min-h-0">
      <CardHeader className="flex flex-row items-center gap-2">
        <FileText className="size-4 text-primary" />
        <CardTitle>{t("editor.transcription")}</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-sm text-muted-foreground">{t("editor.empty")}</p>
      </CardContent>
    </Card>
  );
}

function Editor() {
  const { ed, readOnly, selected, setSelected } = useEditorContext()!;
  const { view } = ed;
  const [searching, setSearching] = useState(false);
  const [bulk, setBulk] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);

  const rows = useVirtualizer({
    count: view.lines.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => 72,
    overscan: 8,
  });

  const changed = useMemo(() => new Set(view.changedIds), [view.changedIds]);
  const needsPost = useMemo(() => new Set(view.needsPostIds), [view.needsPostIds]);

  if (!view.ready || view.lines.length === 0) return <EmptyEditor />;

  const running = view.run !== null;
  const toMake = new Set([...view.changedIds, ...view.needsPostIds]).size;
  const locked = readOnly || running;
  const items = rows.getVirtualItems();
  const padTop = items.length ? items[0].start : 0;
  const padBottom = items.length ? rows.getTotalSize() - items[items.length - 1].end : 0;
  const last = view.lines[view.lines.length - 1];

  const insert = () => {
    const at = view.lines.filter((l) => selected.has(l.id)).pop() ?? last;
    const start = at.endMs + 200;
    void ed.insert({
      afterId: at.id,
      startMs: start,
      endMs: start + 2000,
      speakerId: at.speakerId,
      translated: "",
    });
  };
  const remove = async () => {
    await ed.remove([...selected]);
    setSelected(new Set());
  };
  const toggle = (id: string, on: boolean) => {
    const next = new Set(selected);
    if (on) next.add(id);
    else next.delete(id);
    setSelected(next);
  };
  const percent = view.run && view.run.total > 0 ? (view.run.done / view.run.total) * 100 : 0;

  return (
    <Card className="h-full min-h-0">
      <CardHeader className="flex flex-row flex-wrap items-center gap-2">
        <FileText className="size-4 text-primary" />
        <CardTitle>{t("editor.transcription")}</CardTitle>
        <Button size="sm" variant="ghost" disabled={locked} onClick={insert}>
          <Plus data-icon="inline-start" />
          {t("editor.insert")}
        </Button>
        <Button size="sm" variant="ghost" disabled={locked || selected.size === 0} onClick={remove}>
          <Trash2 data-icon="inline-start" />
          {t("editor.deleteSelected", { count: selected.size })}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={locked || !view.canUndo}
          onClick={() => ed.undo()}
        >
          <Undo2 data-icon="inline-start" />
          {t("editor.undo")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={locked || !view.canRedo}
          onClick={() => ed.redo()}
        >
          <Redo2 data-icon="inline-start" />
          {t("editor.redo")}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setSearching(true)}>
          <Search data-icon="inline-start" />
          {t("editor.searchReplace")}
        </Button>
        <Button size="sm" variant="ghost" disabled={locked} onClick={() => setBulk(true)}>
          <ListChecks data-icon="inline-start" />
          {t("editor.bulk.open")}
        </Button>
        <span className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <MemoryBadge testId="editor-memory" />
          {t("editor.linesCount", { count: view.lines.length })}
        </span>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
        <SpeakerPanel />
        {view.error && (
          <Alert variant="destructive" role="alert" data-testid="editor-error">
            <AlertDescription>
              {t(errorKey(view.error), (view.error.details as Record<string, string>) ?? {})}
            </AlertDescription>
          </Alert>
        )}
        <div
          ref={scroller}
          className="min-h-0 flex-1 overflow-auto rounded-md border"
          data-testid="editor-table"
        >
          <Table className="table-fixed">
            <TableHeader className="sticky top-0 z-10 bg-card">
              <TableRow>
                <TableHead className="w-8" />
                <TableHead className="w-28">{t("editor.colStart")}</TableHead>
                <TableHead className="w-28">{t("editor.colEnd")}</TableHead>
                <TableHead className="w-28">{t("editor.colSpeaker")}</TableHead>
                <TableHead className="w-[22%]">{t("editor.colOriginal")}</TableHead>
                <TableHead className="w-[30%]">{t("editor.colTranslated")}</TableHead>
                <TableHead className="w-24">{t("editor.colFlags")}</TableHead>
                <TableHead className="w-24">{t("editor.colStatus")}</TableHead>
                <TableHead className="w-10" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {padTop > 0 && (
                <tr aria-hidden style={{ height: padTop }}>
                  <td colSpan={COLUMNS} />
                </tr>
              )}
              {items.map((item) => {
                const line = view.lines[item.index];
                return (
                  <TableRow
                    key={line.id}
                    data-index={item.index}
                    ref={rows.measureElement}
                    data-testid="editor-row"
                  >
                    <LineRow
                      line={line}
                      n={item.index + 1}
                      speakers={view.speakers}
                      selected={selected.has(line.id)}
                      changed={changed.has(line.id)}
                      needsPost={needsPost.has(line.id)}
                      disabled={readOnly}
                      onSelect={(on) => toggle(line.id, on)}
                      onEdit={(edit) => ed.update(edit)}
                      canMerge={
                        item.index + 1 < view.lines.length &&
                        view.lines[item.index + 1].speakerId === line.speakerId
                      }
                      onMerge={() => void ed.merge([line.id, view.lines[item.index + 1].id])}
                      onSplit={(offset) => void ed.split(line.id, offset)}
                      onDuplicate={() => void ed.duplicate(line.id)}
                    />
                  </TableRow>
                );
              })}
              {padBottom > 0 && (
                <tr aria-hidden style={{ height: padBottom }}>
                  <td colSpan={COLUMNS} />
                </tr>
              )}
            </TableBody>
          </Table>
        </div>
        {running && view.run && (
          <div className="flex flex-col gap-1" data-testid="editor-run">
            <Bar value={percent} aria-label={t("editor.synthesize")} />
            <span className="text-xs text-muted-foreground">
              {t("editor.progressLines", { done: view.run.done, total: view.run.total })}
            </span>
          </div>
        )}
        {view.left !== null && !running && (
          <p className="text-xs text-muted-foreground" data-testid="editor-left">
            {t("editor.stopped", { count: view.left })}
          </p>
        )}
        <div className="flex gap-2">
          <Button
            className="flex-1"
            disabled={readOnly || running || toMake === 0}
            onClick={() => ed.resynthesize()}
            data-testid="synthesize"
          >
            <Check data-icon="inline-start" />
            {toMake > 0 ? t("editor.synthesizeCount", { count: toMake }) : t("editor.synthNothing")}
          </Button>
          {running && (
            <Button variant="outline" onClick={() => ed.stop()} data-testid="editor-stop">
              <Square data-icon="inline-start" />
              {t("editor.stopRunning")}
            </Button>
          )}
        </div>
      </CardContent>
      <BulkEditDialog
        open={bulk}
        onOpenChange={setBulk}
        lines={view.lines}
        onApply={(texts) => ed.bulk(texts)}
      />
      <SearchReplace
        open={searching}
        onOpenChange={setSearching}
        lines={view.lines}
        search={ed.search}
        replace={ed.replace}
      />
    </Card>
  );
}

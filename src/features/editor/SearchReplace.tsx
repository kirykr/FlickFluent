import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { TranscriptLine } from "../dub/types";
import type { SearchField, SearchMatch, SearchQuery } from "./types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  lines: TranscriptLine[];
  search: (q: SearchQuery) => Promise<SearchMatch[]>;
  replace: (matches: SearchMatch[], replacement: string) => Promise<unknown>;
}

const key = (m: SearchMatch) => `${m.lineId}:${m.field}:${m.start}`;

/** Find text in the original or translated lines and replace a chosen part of the matches. */
export function SearchReplace({ open, onOpenChange, lines, search, replace }: Props) {
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [field, setField] = useState<SearchField>("translated");
  const [matchCase, setMatchCase] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [matches, setMatches] = useState<SearchMatch[]>([]);
  const [chosen, setChosen] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!open || !query) {
      setMatches([]);
      setChosen(new Set());
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      void search({ query, field, matchCase, wholeWord }).then((found) => {
        if (!current) return;
        setMatches(found);
        // Matches in the original text cannot be replaced, so they start unchosen.
        setChosen(new Set(found.filter((m) => m.field === "translated").map(key)));
      });
    }, 120);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [open, query, field, matchCase, wholeWord, search]);

  const fields: { value: SearchField; label: string }[] = [
    { value: "translated", label: t("editor.search.translated") },
    { value: "original", label: t("editor.search.original") },
    { value: "both", label: t("editor.search.both") },
  ];
  const number = new Map(lines.map((l, i) => [l.id, i + 1]));
  const text = (m: SearchMatch) => {
    const line = lines.find((l) => l.id === m.lineId);
    return (m.field === "original" ? line?.text : line?.translated) ?? "";
  };
  const replaceable = matches.filter((m) => m.field === "translated");
  const picked = replaceable.filter((m) => chosen.has(key(m)));

  const run = async (list: SearchMatch[]) => {
    await replace(list, replacement);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl" data-testid="search-dialog">
        <DialogHeader>
          <DialogTitle>{t("editor.search.title")}</DialogTitle>
          <DialogDescription>{t("editor.search.originalNote")}</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="search-find">{t("editor.search.find")}</Label>
            <Input id="search-find" value={query} onChange={(e) => setQuery(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="search-replace">{t("editor.search.replaceWith")}</Label>
            <Input
              id="search-replace"
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="search-field">{t("editor.search.inField")}</Label>
            <Select
              items={fields}
              value={field}
              onValueChange={(v) => v && setField(v as SearchField)}
            >
              <SelectTrigger id="search-field">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {fields.map((f) => (
                  <SelectItem key={f.value} value={f.value}>
                    {f.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col justify-end gap-2 text-sm">
            <Label className="flex items-center gap-2">
              <Checkbox checked={matchCase} onCheckedChange={(c) => setMatchCase(c === true)} />
              {t("editor.search.matchCase")}
            </Label>
            <Label className="flex items-center gap-2">
              <Checkbox checked={wholeWord} onCheckedChange={(c) => setWholeWord(c === true)} />
              {t("editor.search.wholeWord")}
            </Label>
          </div>
        </div>
        <p className="text-sm text-muted-foreground" data-testid="search-count">
          {query
            ? matches.length
              ? t("editor.search.count", { count: matches.length })
              : t("editor.search.none")
            : ""}
        </p>
        <ScrollArea className="max-h-60 rounded-md border">
          <ul className="flex flex-col divide-y text-sm">
            {matches.map((m) => (
              <li key={key(m)} className="flex items-start gap-2 p-2" data-testid="search-match">
                <Checkbox
                  disabled={m.field === "original"}
                  checked={chosen.has(key(m))}
                  aria-label={t("editor.search.selectMatch")}
                  onCheckedChange={(c) =>
                    setChosen((prev) => {
                      const next = new Set(prev);
                      if (c === true) next.add(key(m));
                      else next.delete(key(m));
                      return next;
                    })
                  }
                />
                <span className="min-w-0 flex-1">
                  <span className="block text-xs text-muted-foreground">
                    {t("editor.search.matchRow", {
                      n: number.get(m.lineId) ?? 0,
                      field: t(`editor.search.${m.field}`),
                    })}
                  </span>
                  <Context text={text(m)} m={m} />
                </span>
              </li>
            ))}
          </ul>
        </ScrollArea>
        <DialogFooter>
          <Button variant="outline" disabled={picked.length === 0} onClick={() => run(picked)}>
            {t("editor.search.replaceSelected", { count: picked.length })}
          </Button>
          <Button disabled={replaceable.length === 0} onClick={() => run(replaceable)}>
            {t("editor.search.replaceAll")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** The line's text with the match marked. Offsets are UTF-16, as JavaScript counts them. */
function Context({ text, m }: { text: string; m: SearchMatch }) {
  return (
    <span className="break-words">
      {text.slice(0, m.start)}
      <mark className="rounded bg-primary/20 px-0.5">{text.slice(m.start, m.end)}</mark>
      {text.slice(m.end)}
    </span>
  );
}

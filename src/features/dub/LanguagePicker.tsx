import { useMemo, useState } from "react";
import { ChevronsUpDown } from "lucide-react";
import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { AUTO, languageName, languageOptions, matchesQuery, type PickerRole } from "./languages";
import type { Language } from "./types";

interface Props {
  id: string;
  languages: Language[];
  value: string;
  onChange: (code: string) => void;
  role: PickerRole;
  /** Offer "Detect automatically" (source only). */
  allowAuto?: boolean;
  disabled?: boolean;
  locale?: string;
}

/**
 * A searchable list of the languages the catalogue knows. The list is data: nothing here names
 * a language. Typing filters by name or code; the code stays the value.
 */
export function LanguagePicker({
  id,
  languages,
  value,
  onChange,
  role,
  allowAuto,
  disabled,
  locale = "en",
}: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const options = useMemo(
    () => languageOptions(languages, role, locale),
    [languages, role, locale],
  );
  const shown = options.filter((o) => matchesQuery(o, query));
  const showAuto =
    !!allowAuto &&
    matchesQuery({ code: AUTO, name: t("dub.new.autoDetect"), hintKey: null }, query);
  const label = value === AUTO ? t("dub.new.autoDetect") : languageName(value, locale) || value;

  const pick = (code: string) => {
    onChange(code);
    setOpen(false);
    setQuery("");
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            id={id}
            variant="outline"
            role="combobox"
            aria-expanded={open}
            disabled={disabled}
            className="w-full justify-between font-normal"
          />
        }
      >
        <span className="truncate">{label}</span>
        <ChevronsUpDown className="size-4 opacity-50" />
      </PopoverTrigger>
      <PopoverContent className="w-72 p-0" align="start">
        {/* The list is filtered here, not by the Command, so names and codes both match. */}
        <Command shouldFilter={false}>
          <CommandInput
            value={query}
            onValueChange={setQuery}
            placeholder={t("languages.search")}
            aria-label={t("languages.search")}
          />
          <CommandList>
            <CommandEmpty>{t("languages.none")}</CommandEmpty>
            {showAuto && (
              <CommandItem value={AUTO} data-code={AUTO} onSelect={() => pick(AUTO)}>
                {t("dub.new.autoDetect")}
              </CommandItem>
            )}
            {shown.map((o) => (
              <CommandItem
                key={o.code}
                value={`${o.name} ${o.code}`}
                data-code={o.code}
                onSelect={() => pick(o.code)}
              >
                <span className="flex-1 truncate">{o.name}</span>
                {o.hintKey && <span className="text-xs text-muted-foreground">{t(o.hintKey)}</span>}
              </CommandItem>
            ))}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}

import { useState } from "react";
import { Mars, Venus } from "lucide-react";
import { t } from "../../i18n";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useEditorContext } from "./EditorContext";
import type { SpeakerGender } from "./types";

const GENDERS: SpeakerGender[] = ["male", "female", "unknown"];

/** Each speaker's gender. A correction changes the voice, so that speaker's lines need speech again. */
export function SpeakerPanel() {
  const ctx = useEditorContext();
  const [note, setNote] = useState<{ id: string; count: number } | null>(null);
  if (!ctx || ctx.ed.view.speakers.length === 0) return null;
  const { ed, readOnly } = ctx;
  const busy = readOnly || ed.view.run !== null;

  const change = async (id: string, gender: SpeakerGender) => {
    const r = await ed.setGender(id, gender);
    if (r) setNote({ id, count: ed.view.lines.filter((l) => l.speakerId === id).length });
  };

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs" data-testid="speakers">
      <span className="font-medium">{t("editor.speakers.title")}</span>
      {ed.view.speakers.map((s) => (
        <div key={s.id} className="flex items-center gap-2" data-speaker={s.id}>
          <Label className="text-xs">{s.id}</Label>
          <ToggleGroup
            size="sm"
            variant="outline"
            spacing={0}
            value={[s.gender]}
            disabled={busy}
            aria-label={t("editor.speakers.gender", { id: s.id })}
            onValueChange={(v) =>
              v[0] && v[0] !== s.gender && void change(s.id, v[0] as SpeakerGender)
            }
          >
            {GENDERS.map((g) => (
              <ToggleGroupItem
                key={g}
                value={g}
                aria-label={`${s.id} ${t(`editor.speakers.${g}`)}`}
              >
                {g === "male" ? <Mars /> : g === "female" ? <Venus /> : null}
                {t(`editor.speakers.${g}`)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          <span className="text-muted-foreground">
            {s.genderSource === "user" ? t("editor.speakers.set") : t("editor.speakers.detected")}
          </span>
        </div>
      ))}
      {note && (
        <span className="text-muted-foreground" data-testid="speaker-note">
          {t("editor.speakers.changed", { count: note.count, id: note.id })}
        </span>
      )}
    </div>
  );
}

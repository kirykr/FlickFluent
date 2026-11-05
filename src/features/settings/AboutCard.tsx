import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { modelsCatalogue } from "../../app/ipc";
import type { ModelRecord } from "../models/types";

/**
 * Licence notices for the downloadable parts whose licence is not "do anything": each one
 * appears here and again before its download (FR-016). The full list of components with their
 * licences is in `docs/dependencies.md`.
 */
export function AboutCard() {
  const [models, setModels] = useState<ModelRecord[]>([]);
  useEffect(() => {
    modelsCatalogue()
      .then(setModels)
      .catch(() => setModels([]));
  }, []);

  // One line per notice, naming the parts it covers.
  const notices = new Map<string, string[]>();
  for (const m of models) {
    if (!m.licenceNoticeKey) continue;
    notices.set(m.licenceNoticeKey, [...(notices.get(m.licenceNoticeKey) ?? []), m.name]);
  }

  return (
    <Card data-testid="about">
      <CardHeader>
        <CardTitle>{t("about.title")}</CardTitle>
        <CardDescription>{t("about.body")}</CardDescription>
      </CardHeader>
      <CardContent>
        {notices.size === 0 ? (
          <p className="text-sm text-muted-foreground">{t("about.none")}</p>
        ) : (
          <ul className="flex flex-col gap-3 text-sm">
            {[...notices].map(([key, names]) => (
              <li key={key} data-notice={key}>
                <p>{t(key)}</p>
                <p className="text-xs text-muted-foreground">{names.join(", ")}</p>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

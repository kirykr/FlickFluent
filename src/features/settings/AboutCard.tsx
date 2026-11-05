import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cloudServices, modelsCatalogue } from "../../app/ipc";
import type { CloudService } from "../cloud/types";
import type { ModelRecord } from "../models/types";

/**
 * Licence notices for the downloadable parts whose licence is not "do anything": each one
 * appears here and again before its download (FR-016). The full list of components with their
 * licences is in `docs/dependencies.md`.
 */
export function AboutCard() {
  const [models, setModels] = useState<ModelRecord[]>([]);
  const [services, setServices] = useState<CloudService[]>([]);
  useEffect(() => {
    cloudServices()
      .then((s) => setServices(s ?? []))
      .catch(() => setServices([]));
  }, []);
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
        {services.length > 0 && (
          <div className="mt-4 flex flex-col gap-1 text-sm" data-testid="about-cloud">
            <p>{t("about.cloud")}</p>
            <ul className="list-disc ps-5">
              {services.map((s) => (
                <li key={s.id}>
                  <a className="text-primary underline" href={s.termsUrl} target="_blank" rel="noreferrer">
                    {t("cloud.terms", { service: s.name })}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

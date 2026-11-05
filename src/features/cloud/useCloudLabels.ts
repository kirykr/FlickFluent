import { useEffect, useState } from "react";
import { cloudServices } from "../../app/ipc";
import { t } from "../../i18n";
import type { Stage } from "../dub/types";
import type { CloudService, Engines } from "./types";
import { LOCAL } from "./types";

/** "Cloud: <service>" for the stages that ran, or will run, on a cloud service. */
export function cloudLabelsFor(
  engines: Engines | undefined,
  services: CloudService[],
): Partial<Record<Stage, string>> {
  const out: Partial<Record<Stage, string>> = {};
  const name = (id: string) => services.find((s) => s.id === id)?.name ?? id;
  if (engines && engines.translate !== LOCAL) {
    out.translate = t("cloud.label", { service: name(engines.translate) });
  }
  if (engines && engines.speak !== LOCAL) {
    out.synthesize = t("cloud.label", { service: name(engines.speak) });
  }
  return out;
}

export function useCloudLabels(engines: Engines | undefined) {
  const [services, setServices] = useState<CloudService[]>([]);
  const used = !!engines && (engines.translate !== LOCAL || engines.speak !== LOCAL);
  useEffect(() => {
    if (!used) return;
    cloudServices()
      .then(setServices)
      .catch(() => setServices([]));
  }, [used]);
  return cloudLabelsFor(engines, services);
}

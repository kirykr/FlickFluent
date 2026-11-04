import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { appMemory, systemPercent, type MemorySample } from "./format";

/** Memory use, refreshed by the core every two seconds. Turns to a warning on a crowded computer. */
export function MemoryBadge() {
  const [sample, setSample] = useState<MemorySample | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    listen<MemorySample>("memory:sample", (e) => setSample(e.payload))
      .then((un) => (cancelled ? un() : (off = un)))
      .catch(() => {});
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  if (!sample) return null;
  const warning = sample.level === "warning";
  return (
    <Badge
      variant={warning ? "destructive" : "outline"}
      data-testid="memory"
      data-level={sample.level}
      title={warning ? t("memory.hint") : undefined}
    >
      {t("memory.label")}:{" "}
      {t("memory.detail", { app: appMemory(sample), percent: systemPercent(sample) })}
      {warning && <span> · {t("memory.hint")}</span>}
    </Badge>
  );
}

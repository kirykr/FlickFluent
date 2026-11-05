import { useCallback, useEffect, useState } from "react";
import { t } from "../../i18n";
import { cloudConsentList, cloudConsentWithdraw } from "../../app/ipc";
import { Button } from "@/components/ui/button";
import { roundChars, type CloudService, type ConsentRecord } from "./types";

/** The agreements to send data, each with a way to take it back. */
export function ConsentList({ services }: { services: CloudService[] }) {
  const [records, setRecords] = useState<ConsentRecord[]>([]);

  const load = useCallback(() => {
    cloudConsentList()
      .then((r) => setRecords(r ?? []))
      .catch(() => setRecords([]));
  }, []);
  useEffect(load, [load]);

  const withdraw = async (r: ConsentRecord) => {
    await cloudConsentWithdraw(r.serviceId, r.step);
    load();
  };

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">{t("cloud.consent.title")}</h3>
      {records.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("cloud.consent.empty")}</p>
      )}
      <ul className="flex flex-col gap-2">
        {records.map((r) => (
          <li key={`${r.serviceId}-${r.step}`} className="flex items-center gap-2 text-sm">
            <span>
              {t("cloud.consent.line", {
                service: services.find((s) => s.id === r.serviceId)?.name ?? r.serviceId,
                step: t(`cloud.stepLabel.${r.step}`),
                chars: roundChars(r.agreedChars),
              })}
            </span>
            <Button size="sm" variant="outline" className="ms-auto" onClick={() => withdraw(r)}>
              {t("cloud.consent.withdraw")}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}

import { t } from "../../i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { roundChars, type CloudUsage } from "./types";

/** Which steps of a finished project ran on a service, and how much text was sent. */
export function UsageList({ usage }: { usage: CloudUsage[] }) {
  if (usage.length === 0) return null;
  return (
    <Card size="sm" data-testid="cloud-usage">
      <CardHeader>
        <CardTitle>{t("cloud.usage.title")}</CardTitle>
      </CardHeader>
      <CardContent>
        <ul className="flex flex-col gap-1 text-sm">
          {usage.map((u) => (
            <li key={`${u.step}-${u.serviceId}`}>
              {t("cloud.usage.line", {
                step: t(`cloud.stepLabel.${u.step}`),
                service: u.serviceName,
                chars: roundChars(u.chars),
                requests: u.requests,
              })}
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  );
}

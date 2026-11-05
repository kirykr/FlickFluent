import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { cloudConsentGrant, cloudNotice } from "../../app/ipc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { roundChars, type CloudPreflight, type Notice } from "./types";

interface Props {
  /** The cloud step that needs an agreement, or null when no question is open. */
  item: CloudPreflight | null;
  onAgree: () => void;
  onDecline: () => void;
}

/** Says which service gets what before anything leaves the computer, and waits for a yes. */
export function ConsentDialog({ item, onAgree, onDecline }: Props) {
  const [notice, setNotice] = useState<Notice | null>(null);
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setNotice(null);
    setFailed(false);
    if (!item) return;
    let current = true;
    cloudNotice(item.serviceId, item.step, item.estimatedChars)
      .then((n) => current && setNotice(n))
      .catch(() => current && setFailed(true));
    return () => {
      current = false;
    };
  }, [item]);

  const agree = async () => {
    if (!item) return;
    setBusy(true);
    try {
      await cloudConsentGrant(item.serviceId, item.step, item.estimatedChars);
      onAgree();
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={item !== null} onOpenChange={(o) => !o && onDecline()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("cloud.notice.title", { service: item?.serviceName ?? "" })}</DialogTitle>
          <DialogDescription>
            {item?.reason
              ? t(`cloud.notice.reason.${item.reason}`, { service: item.serviceName })
              : ""}
          </DialogDescription>
        </DialogHeader>
        {failed && (
          <Alert variant="destructive" role="alert">
            <AlertDescription>{t("settings.error.generic")}</AlertDescription>
          </Alert>
        )}
        {notice && item && (
          <div className="flex flex-col gap-3 text-sm">
            <section aria-label={t("cloud.notice.sends")}>
              <p className="font-medium">
                {t("cloud.stepLabel." + notice.step)} · {t("cloud.notice.sends")}
              </p>
              <ul className="list-disc ps-5">
                {notice.sends.map((k) => (
                  <li key={k}>{t(k)}</li>
                ))}
              </ul>
              <p data-testid="consent-amount">
                {t("cloud.notice.amount", { chars: roundChars(notice.estimatedChars) })}
              </p>
            </section>
            <ul className="flex flex-col gap-1 text-muted-foreground">
              {notice.doesNotSend.map((k) => (
                <li key={k}>{t(k)}</li>
              ))}
            </ul>
            <a
              className="text-primary underline"
              href={notice.termsUrl}
              target="_blank"
              rel="noreferrer"
            >
              {t("cloud.terms", { service: notice.serviceName })}
            </a>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onDecline}>
            {t("cloud.notice.notNow")}
          </Button>
          <Button disabled={!notice || busy} onClick={agree}>
            {t("cloud.notice.agree")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

import { useState } from "react";
import { t } from "../../i18n";
import { cloudKeyAdd } from "../../app/ipc";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { isStoreUnavailable, keyErrorKey } from "./errors";
import type { CloudService } from "./types";

interface Props {
  services: CloudService[];
  open: boolean;
  onClose: () => void;
  /** Called after a key was saved. */
  onAdded: () => void;
}

export function KeyDialog({ services, open, onClose, onAdded }: Props) {
  const [serviceId, setServiceId] = useState(services[0]?.id ?? "");
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState("");
  const [region, setRegion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [offerSession, setOfferSession] = useState(false);
  const [busy, setBusy] = useState(false);

  const service = services.find((s) => s.id === (serviceId || services[0]?.id));

  const close = () => {
    // The key is cleared from the screen as soon as the dialog closes.
    setSecret("");
    setLabel("");
    setRegion("");
    setError(null);
    setOfferSession(false);
    onClose();
  };

  const save = async (sessionOnly: boolean) => {
    if (!service) return;
    setBusy(true);
    setError(null);
    try {
      await cloudKeyAdd({
        serviceId: service.id,
        label,
        secret,
        region: service.needsRegion ? region : undefined,
        sessionOnly,
      });
      onAdded();
      close();
    } catch (e) {
      setError(keyErrorKey(e));
      setOfferSession(isStoreUnavailable(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("cloud.addTitle")}</DialogTitle>
          <DialogDescription>{t("cloud.addIntro")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="cloud-service">{t("cloud.field.service")}</Label>
            <Select value={service?.id ?? ""} onValueChange={(v) => v && setServiceId(v)}>
              <SelectTrigger id="cloud-service" className="w-full">
                <SelectValue>{service?.name}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {services.map((s) => (
                  <SelectItem key={s.id} value={s.id}>
                    {s.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="cloud-label">{t("cloud.field.label")}</Label>
            <Input
              id="cloud-label"
              value={label}
              maxLength={40}
              onChange={(e) => setLabel(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="cloud-key">{t("cloud.field.key")}</Label>
            <Input
              id="cloud-key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
            />
          </div>
          {service?.needsRegion && (
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="cloud-region">{t("cloud.field.region")}</Label>
              <Input
                id="cloud-region"
                value={region}
                onChange={(e) => setRegion(e.target.value)}
              />
            </div>
          )}
          {error && (
            <Alert variant="destructive" role="alert">
              <AlertDescription>{t(error)}</AlertDescription>
            </Alert>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={close}>
            {t("cloud.cancel")}
          </Button>
          {offerSession && (
            <Button variant="secondary" disabled={busy} onClick={() => save(true)}>
              {t("cloud.sessionOnly")}
            </Button>
          )}
          <Button disabled={busy || !label.trim() || !secret.trim()} onClick={() => save(false)}>
            {t("cloud.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

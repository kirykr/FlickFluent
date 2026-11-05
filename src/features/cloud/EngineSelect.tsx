import { useCallback, useEffect, useState } from "react";
import { t } from "../../i18n";
import { cloudEnginesGet, cloudEnginesSet } from "../../app/ipc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { asAppError } from "../projects/errors";
import { LOCAL, type CloudKey, type CloudService, type CloudStep, type Engines } from "./types";

const STEPS: CloudStep[] = ["translate", "speak"];

interface Props {
  services: CloudService[];
  keys: CloudKey[];
}

/** Per step: run on this computer (the default) or on a service that has a verified key. */
export function EngineSelect({ services, keys }: Props) {
  const [engines, setEngines] = useState<Engines>({ translate: LOCAL, speak: LOCAL });
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    cloudEnginesGet()
      .then(setEngines)
      .catch(() => setError("settings.error.generic"));
  }, []);
  useEffect(load, [load]);

  const verified = (serviceId: string) =>
    keys.some((k) => k.serviceId === serviceId && k.state === "verified");

  const choose = async (step: CloudStep, engine: string) => {
    setError(null);
    // A service with no verified key cannot be chosen; say what to do instead.
    if (engine !== LOCAL && !verified(engine)) {
      setError("cloud.engine.unverified");
      return;
    }
    try {
      setEngines(await cloudEnginesSet(step, engine));
    } catch (e) {
      setError(
        asAppError(e).code === "CLOUD_NO_KEY" ? "cloud.engine.unverified" : "settings.error.generic",
      );
    }
  };

  const nameOf = (id: string) =>
    id === LOCAL ? t("cloud.engine.local") : (services.find((s) => s.id === id)?.name ?? id);

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-medium">{t("cloud.engine.title")}</h3>
      {error && (
        <Alert variant="destructive" role="alert">
          <AlertDescription>{t(error)}</AlertDescription>
        </Alert>
      )}
      {STEPS.map((step) => {
        const id = `engine-${step}`;
        return (
          <div key={step} className="flex flex-col gap-1.5">
            <Label htmlFor={id}>{t("cloud.engine.label", { step: t(`cloud.stepLabel.${step}`) })}</Label>
            <Select value={engines[step]} onValueChange={(v) => v && void choose(step, v)}>
              <SelectTrigger id={id} className="w-full">
                <SelectValue>{nameOf(engines[step])}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={LOCAL}>{t("cloud.engine.local")}</SelectItem>
                {services
                  .filter((s) => s.steps.includes(step))
                  .map((s) => (
                    <SelectItem key={s.id} value={s.id}>
                      {s.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </div>
        );
      })}
    </div>
  );
}

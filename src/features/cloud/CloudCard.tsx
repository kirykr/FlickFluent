import { useState } from "react";
import { t } from "../../i18n";
import { cloudKeyRemove, cloudKeysReorder, cloudKeyVerify } from "../../app/ipc";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { ConsentList } from "./ConsentList";
import { EngineSelect } from "./EngineSelect";
import { KeyDialog } from "./KeyDialog";
import type { CloudKey, VerifyResult } from "./types";
import { useCloud } from "./useCloud";

/** Settings card: add, verify and remove the user's own keys for cloud services. */
export function CloudCard() {
  const { services, keys, failed, reload } = useCloud();
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<CloudKey | null>(null);
  const [checking, setChecking] = useState<string | null>(null);
  const [results, setResults] = useState<Record<string, VerifyResult>>({});
  const [error, setError] = useState(false);

  const serviceName = (id: string) => services.find((s) => s.id === id)?.name ?? id;

  const verify = async (key: CloudKey) => {
    setChecking(key.id);
    setError(false);
    try {
      const result = await cloudKeyVerify(key.id);
      setResults((r) => ({ ...r, [key.id]: result }));
      await reload();
    } catch {
      setError(true);
    } finally {
      setChecking(null);
    }
  };

  // Move a key one place within its service; the first key is tried first.
  const move = async (key: CloudKey, by: -1 | 1) => {
    const ids = keys.filter((k) => k.serviceId === key.serviceId).map((k) => k.id);
    const at = ids.indexOf(key.id);
    const to = at + by;
    if (at < 0 || to < 0 || to >= ids.length) return;
    [ids[at], ids[to]] = [ids[to], ids[at]];
    try {
      await cloudKeysReorder(key.serviceId, ids);
      await reload();
    } catch {
      setError(true);
    }
  };

  const confirmRemove = async () => {
    if (!removing) return;
    const id = removing.id;
    setRemoving(null);
    try {
      await cloudKeyRemove(id);
      setResults(({ [id]: _gone, ...rest }) => rest);
      await reload();
    } catch {
      setError(true);
    }
  };

  const message = (key: CloudKey, r: VerifyResult) => {
    const service = serviceName(key.serviceId);
    return r.ok
      ? t("cloud.verifyResult.ok", { service })
      : t(`cloud.verifyResult.${r.reason}`, { service });
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("cloud.title")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <p className="text-sm text-muted-foreground">{t("cloud.intro")}</p>
        {(failed || error) && (
          <Alert variant="destructive" role="alert">
            <AlertDescription>{t("settings.error.generic")}</AlertDescription>
          </Alert>
        )}
        {keys.length === 0 && <p className="text-sm">{t("cloud.empty")}</p>}
        <ul className="flex flex-col gap-3">
          {keys.map((key) => {
            const result = results[key.id];
            return (
              <li key={key.id} className="flex flex-col gap-1.5 rounded-md border p-3">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{key.label}</span>
                  <span className="text-sm text-muted-foreground">
                    {serviceName(key.serviceId)}
                    {key.region ? ` · ${key.region}` : ""}
                  </span>
                  <span className="text-sm">{t("cloud.keyMask", { last4: key.last4 })}</span>
                  <Badge variant={key.state === "verified" ? "default" : "secondary"}>
                    {t(`cloud.state.${key.state}`)}
                  </Badge>
                  {key.storage === "session" && (
                    <Badge variant="outline">{t("cloud.sessionNote")}</Badge>
                  )}
                  <span className="ms-auto flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      aria-label={t("cloud.moveUp")}
                      disabled={!keys.some((k) => k.serviceId === key.serviceId && k.order < key.order)}
                      onClick={() => move(key, -1)}
                    >
                      ↑
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      aria-label={t("cloud.moveDown")}
                      disabled={!keys.some((k) => k.serviceId === key.serviceId && k.order > key.order)}
                      onClick={() => move(key, 1)}
                    >
                      ↓
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={checking === key.id}
                      onClick={() => verify(key)}
                    >
                      {checking === key.id ? (
                        <>
                          <Spinner /> {t("cloud.verifying")}
                        </>
                      ) : (
                        t("cloud.verify")
                      )}
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => setRemoving(key)}>
                      {t("cloud.remove")}
                    </Button>
                  </span>
                </div>
                {result && (
                  <Alert variant={result.ok ? "default" : "destructive"} role="status">
                    <AlertDescription>{message(key, result)}</AlertDescription>
                  </Alert>
                )}
              </li>
            );
          })}
        </ul>
        <div>
          <Button onClick={() => setAdding(true)} disabled={services.length === 0}>
            {t("cloud.add")}
          </Button>
        </div>
        {services.length > 0 && <EngineSelect services={services} keys={keys} />}
        {services.length > 0 && <ConsentList services={services} />}
      </CardContent>

      {services.length > 0 && (
        <KeyDialog
          key={adding ? "open" : "closed"}
          services={services}
          open={adding}
          onClose={() => setAdding(false)}
          onAdded={() => void reload()}
        />
      )}

      <Dialog open={removing !== null} onOpenChange={(o) => !o && setRemoving(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("cloud.removeTitle")}</DialogTitle>
            <DialogDescription>{t("cloud.removeBody")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRemoving(null)}>
              {t("cloud.cancel")}
            </Button>
            <Button variant="destructive" onClick={confirmRemove}>
              {t("cloud.remove")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

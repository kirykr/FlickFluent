import { useCallback, useEffect, useState } from "react";
import { cloudKeysList, cloudServices } from "../../app/ipc";
import type { CloudKey, CloudService } from "./types";

/** The cloud services and the user's keys, reloaded after every change. */
export function useCloud() {
  const [services, setServices] = useState<CloudService[]>([]);
  const [keys, setKeys] = useState<CloudKey[]>([]);
  const [failed, setFailed] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [s, k] = await Promise.all([cloudServices(), cloudKeysList()]);
      setServices(s ?? []);
      setKeys(k ?? []);
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { services, keys, failed, reload };
}

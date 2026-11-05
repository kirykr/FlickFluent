// The Tauri core is replaced by a fake that keeps the cloud keys in localStorage (a reload
// stands in for restarting the app). The fake never keeps the secret: like the real core it
// stores only the last four characters. The real rules (store, duplicates, rotation, consent)
// are tested in core/tests/cloud_*.rs.

declare global {
  interface Window {
    __fakeCloud: {
      /** What "Verify connection" answers next: `ok` or a failure reason. */
      verify: "ok" | "rejected" | "noCredit" | "offline" | "unreachable" | "unexpected";
    };
  }
}

export function installFakeCloudCore() {
  const fake = { verify: "ok" as Window["__fakeCloud"]["verify"] };
  (window as unknown as { __fakeCloud: typeof fake }).__fakeCloud = fake;

  type Key = {
    id: string;
    serviceId: string;
    label: string;
    region: string | null;
    last4: string;
    order: number;
    state: "untested" | "verified" | "rejected";
    verifiedAt: string | null;
    storage: "os" | "session";
  };
  const load = (): Key[] => JSON.parse(localStorage.getItem("fakeCloudKeys") ?? "[]");
  const store = (k: Key[]) => localStorage.setItem("fakeCloudKeys", JSON.stringify(k));

  const SERVICES = [
    {
      id: "google-gemini",
      name: "Google Gemini API",
      steps: ["translate", "speak"],
      needsRegion: false,
      termsUrl: "https://ai.google.dev/gemini-api/terms",
      languages: { translate: ["fr", "en", "km"], speak: ["fr", "en", "km"] },
    },
    {
      id: "azure-ai",
      name: "Azure AI services",
      steps: ["translate", "speak"],
      needsRegion: true,
      termsUrl: "https://www.microsoft.com/licensing/terms",
      languages: { translate: ["fr", "en"], speak: ["fr", "en"] },
    },
  ];

  const DEFAULTS = {
    settingsVersion: 1,
    theme: "system",
    general: { uiLanguage: "en", defaultSaveFolder: null },
    hotkeys: {},
    analyticsEnabled: false,
    modelsFolder: null,
    lastCompatMode: null,
  };

  const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
    run_compat_check: () => ({
      mode: "fullLocal",
      items: [],
      missing: [],
      blockedMessage: null,
      provisional: false,
      generatedAt: "",
    }),
    project_recent: () => [],
    settings_get: () => ({ settings: DEFAULTS, notice: null }),
    cloud_services: () => SERVICES,
    cloud_keys_list: () => load(),
    cloud_key_add: (a) => {
      const keys = load();
      const secret = String(a.secret ?? "").replace(/\s+/g, "");
      const service = SERVICES.find((s) => s.id === a.serviceId);
      if (!service || !secret || !String(a.label ?? "").trim()) throw { code: "KEY_INVALID" };
      if (service.needsRegion && !String(a.region ?? "").trim()) throw { code: "KEY_INVALID" };
      if (keys.some((k) => k.serviceId === a.serviceId && k.label === a.label)) {
        throw { code: "KEY_DUPLICATE" };
      }
      const key: Key = {
        id: `k${keys.length + 1}-${Date.now()}`,
        serviceId: String(a.serviceId),
        label: String(a.label),
        region: service.needsRegion ? String(a.region) : null,
        last4: secret.slice(-4),
        order: keys.filter((k) => k.serviceId === a.serviceId).length,
        state: "untested",
        verifiedAt: null,
        storage: a.sessionOnly ? "session" : "os",
      };
      store([...keys, key]);
      return key;
    },
    cloud_key_verify: (a) => {
      const keys = load();
      const key = keys.find((k) => k.id === a.keyId);
      if (!key) throw { code: "KEY_INVALID" };
      if (fake.verify === "ok") {
        key.state = "verified";
        key.verifiedAt = new Date().toISOString();
        store(keys);
        return { ok: true, languages: { translate: ["fr", "en"], speak: ["fr", "en"] } };
      }
      if (fake.verify === "rejected" || fake.verify === "noCredit") {
        key.state = "rejected";
        store(keys);
      }
      return { ok: false, reason: fake.verify, message: "from the service" };
    },
    cloud_key_remove: (a) => {
      store(load().filter((k) => k.id !== a.keyId));
      return null;
    },
    cloud_keys_reorder: () => load(),
  };

  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: Record<string, unknown>) => {
      const run = commands[cmd];
      if (!run) throw new Error(`fake core: unhandled command ${cmd}`);
      return run(args ?? {});
    },
  };
}

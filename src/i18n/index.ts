import i18next from "i18next";
import en from "./en.json";

// All user-visible text lives in the catalogue so it can be translated later (FR-020).
i18next.init({
  lng: "en",
  fallbackLng: "en",
  resources: { en: { translation: en } },
  interpolation: { escapeValue: false },
  initImmediate: false,
});

export const t = (key: string, options?: Record<string, unknown>): string =>
  i18next.t(key, options) as string;

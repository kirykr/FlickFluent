import { asAppError } from "../projects/errors";

const KEYS: Record<string, string> = {
  KEY_INVALID: "dub.error.keyInvalid",
  KEY_DUPLICATE: "dub.error.keyDuplicate",
  KEY_STORE_UNAVAILABLE: "dub.error.keyStoreUnavailable",
};

/** The i18n key for an error from a key command. */
export function keyErrorKey(e: unknown): string {
  return KEYS[asAppError(e).code] ?? "settings.error.generic";
}

export const isStoreUnavailable = (e: unknown): boolean =>
  asAppError(e).code === "KEY_STORE_UNAVAILABLE";

export type CloudStep = "translate" | "speak";
export type KeyState = "untested" | "verified" | "rejected";
export type VerifyReason = "rejected" | "noCredit" | "offline" | "unreachable" | "unexpected";

export interface CloudService {
  id: string;
  name: string;
  steps: CloudStep[];
  needsRegion: boolean;
  termsUrl: string;
  languages: { translate: string[]; speak: string[] };
}

/** A key as the core sends it: never the secret, only the last four characters. */
export interface CloudKey {
  id: string;
  serviceId: string;
  label: string;
  region: string | null;
  last4: string;
  order: number;
  state: KeyState;
  verifiedAt: string | null;
  storage: "os" | "session";
}

export type VerifyResult =
  | { ok: true; languages: { translate: string[]; speak: string[] } }
  | { ok: false; reason: VerifyReason; message: string | null };

export interface AddKeyInput {
  serviceId: string;
  label: string;
  secret: string;
  region?: string;
  sessionOnly?: boolean;
}

export interface Engines {
  translate: string;
  speak: string;
}

/** What a cloud step in the next run will do. */
export interface CloudPreflight {
  step: CloudStep;
  serviceId: string;
  serviceName: string;
  estimatedChars: number;
  needsConsent: boolean;
  reason: "first" | "noticeChanged" | "largerRun" | null;
  hasKey: boolean;
}

export interface Notice {
  serviceName: string;
  step: CloudStep;
  /** i18n keys: what is sent. */
  sends: string[];
  /** i18n keys: what is not sent, and who bills. */
  doesNotSend: string[];
  termsUrl: string;
  estimatedChars: number;
  noticeVersion: number;
}

export interface ConsentRecord {
  serviceId: string;
  step: CloudStep;
  agreedAt: string;
  agreedChars: number;
  noticeVersion: number;
}

export interface CloudUsage {
  step: CloudStep;
  serviceId: string;
  serviceName: string;
  chars: number;
  requests: number;
  keysUsed: string[];
  switches: number;
  waitedSeconds: number;
  runId: string;
}

export const LOCAL = "local";

/** The amount as shown: "about N" to the nearest hundred. */
export const roundChars = (n: number): number => Math.round(n / 100) * 100;

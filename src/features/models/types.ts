export type ModelState =
  "notInstalled" | "downloading" | "paused" | "verifying" | "installed" | "failed";

export type ModelStep =
  | "runtime"
  | "ffmpeg"
  | "separate"
  | "transcribe"
  | "translate"
  | "speak"
  | "vad"
  | "speaker"
  | "data"
  | "test";
export type MachineClass = "minimum" | "mid" | "fullLocal";

/** Why this computer cannot use a model. */
export interface Unavailable {
  reason: "platform" | "machine";
  /** For `machine`: the class the model needs. */
  needs: MachineClass | null;
}

export interface ModelRecord {
  id: string;
  name: string;
  purpose: string;
  licence: string;
  licenceClass: "permissive" | "copyleftFile" | "nonCommercial";
  /** i18n key of the licence notice to show before download, when the licence needs one. */
  licenceNoticeKey: string | null;
  sizeBytes: number;
  step: ModelStep;
  /** Language codes it handles; empty when it is not tied to a language. */
  languages: string[];
  machineClass: MachineClass;
  unavailable: Unavailable | null;
  state: ModelState;
  bytesDone: number;
  errorCode: string | null;
}

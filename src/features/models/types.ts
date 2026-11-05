export type ModelState =
  "notInstalled" | "downloading" | "paused" | "verifying" | "installed" | "failed";

export interface ModelRecord {
  id: string;
  name: string;
  purpose: string;
  licence: string;
  licenceClass: "permissive" | "copyleftFile" | "nonCommercial";
  /** i18n key of the licence notice to show before download, when the licence needs one. */
  licenceNoticeKey: string | null;
  sizeBytes: number;
  state: ModelState;
  bytesDone: number;
  errorCode: string | null;
}

export type ModelState =
  "notInstalled" | "downloading" | "paused" | "verifying" | "installed" | "failed";

export interface ModelRecord {
  id: string;
  name: string;
  purpose: string;
  licence: string;
  sizeBytes: number;
  state: ModelState;
  bytesDone: number;
  errorCode: string | null;
}

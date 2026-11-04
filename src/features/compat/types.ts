export type Mode = "fullLocal" | "hybrid" | "blocked";
export type ItemStatus = "ok" | "belowMinimum" | "belowFull" | "unknown";

export interface CheckItem {
  key: string;
  required: string;
  detected: string;
  status: ItemStatus;
  neededLabel: string;
  foundLabel: string;
}

export interface MachineCapabilityReport {
  items: CheckItem[];
  mode: Mode;
  missing: CheckItem[];
  blockedMessage: string | null;
  provisional: boolean;
  generatedAt: string;
}

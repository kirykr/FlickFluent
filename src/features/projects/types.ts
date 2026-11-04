export interface ProjectHandle {
  id: string;
  name: string;
  path: string;
  readOnly: boolean;
  dirty: boolean;
  /** Placeholder project content until later features add real data. */
  notes: string;
}

export interface OpenResult {
  handle: ProjectHandle;
  recoveryAvailable: boolean;
}

export interface RecentEntry {
  path: string;
  name: string;
  openedAt: string;
  available: boolean;
}

/** Error shape sent by the core. */
export interface AppErrorShape {
  code: string;
  messageKey?: string;
  details?: { backupAvailable?: boolean } | null;
}

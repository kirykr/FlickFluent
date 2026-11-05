import { invoke } from "@tauri-apps/api/core";
import type {
  AddKeyInput,
  CloudKey,
  CloudService,
  CloudStep,
  ConsentRecord,
  Engines,
  Notice,
  VerifyResult,
} from "../features/cloud/types";
import type { MachineCapabilityReport } from "../features/compat/types";
import type { SettingsView } from "../features/settings/types";
import type { ModelRecord } from "../features/models/types";
import type { OpenResult, ProjectHandle, RecentEntry } from "../features/projects/types";
import type {
  DubOptions,
  DubStatus,
  ExportKind,
  ExportResult,
  Language,
  PairCheck,
  Preflight,
  SourceInfo,
} from "../features/dub/types";

export const runCompatCheck = () => invoke<MachineCapabilityReport>("run_compat_check");
export const getLastCompatReport = () =>
  invoke<MachineCapabilityReport | null>("get_last_compat_report");

export const projectSuggestFileName = (name: string) =>
  invoke<string>("project_suggest_file_name", { name });
export const projectNew = (name: string, path: string) =>
  invoke<ProjectHandle>("project_new", { name, path });
export const projectOpen = (path: string) => invoke<OpenResult>("project_open", { path });
export const projectRecover = (path: string) => invoke<ProjectHandle>("project_recover", { path });
export const projectDiscardRecovery = (path: string) =>
  invoke<void>("project_discard_recovery", { path });
export const projectSave = (handle: string, path?: string) =>
  invoke<ProjectHandle>("project_save", { handle, path: path ?? null });
export const projectSetNotes = (handle: string, notes: string) =>
  invoke<ProjectHandle>("project_set_notes", { handle, notes });
export const projectClose = (handle: string) => invoke<void>("project_close", { handle });
export const projectRecent = () => invoke<RecentEntry[]>("project_recent");
export const projectDelete = (path: string) => invoke<void>("project_delete", { path });

export const settingsGet = () => invoke<SettingsView>("settings_get");
export const settingsSet = (patch: Record<string, unknown>) =>
  invoke<SettingsView>("settings_set", { patch });
export const settingsReset = () => invoke<SettingsView>("settings_reset");

export const modelsCatalogue = () => invoke<ModelRecord[]>("models_catalogue");
export const modelDownloadStart = (id: string) => invoke<void>("model_download_start", { id });
export const modelDownloadPause = (id: string) => invoke<void>("model_download_pause", { id });
export const modelDownloadResume = (id: string) => invoke<void>("model_download_resume", { id });
export const modelDownloadCancel = (id: string) => invoke<void>("model_download_cancel", { id });
export const modelDelete = (id: string) => invoke<void>("model_delete", { id });

export const dubProbeSource = (path: string) => invoke<SourceInfo>("dub_probe_source", { path });
export const dubConfigure = (
  handle: string,
  source: string,
  sourceLanguage: string,
  targetLanguage: string,
  options: DubOptions,
  confirmSame = false,
) =>
  invoke<ProjectHandle>("dub_configure", {
    handle,
    source,
    sourceLanguage,
    targetLanguage,
    options,
    confirmSame,
  });
export const dubPreflight = (handle: string) => invoke<Preflight>("dub_preflight", { handle });
export const dubStart = (handle: string) =>
  invoke<string>("dub_start", { handle, fallbackChoices: null });
export const dubResume = (handle: string) => invoke<string>("dub_resume", { handle });
export const dubCancel = (runId: string) => invoke<void>("dub_cancel", { runId });
export const dubStatus = (handle: string) => invoke<DubStatus>("dub_status", { handle });
export const dubExport = (
  handle: string,
  kind: ExportKind,
  fileName: string,
  folder: string,
  overwrite = false,
) => invoke<ExportResult>("dub_export", { handle, kind, fileName, folder, overwrite });
export const dubDiscardRun = (handle: string) => invoke<void>("dub_discard_run", { handle });

export const languagesList = (optIn: string[] = []) =>
  invoke<Language[]>("languages_list", { optIn });
export const languagesCheckPair = (source: string, target: string, optIn: string[] = []) =>
  invoke<PairCheck>("languages_check_pair", { source, target, optIn });

export const cloudServices = () => invoke<CloudService[]>("cloud_services");
export const cloudKeysList = () => invoke<CloudKey[]>("cloud_keys_list");
export const cloudKeyAdd = (input: AddKeyInput) =>
  invoke<CloudKey>("cloud_key_add", { ...input });
export const cloudKeyVerify = (keyId: string) =>
  invoke<VerifyResult>("cloud_key_verify", { keyId });
export const cloudKeyRemove = (keyId: string) => invoke<void>("cloud_key_remove", { keyId });
export const cloudKeysReorder = (serviceId: string, keyIds: string[]) =>
  invoke<CloudKey[]>("cloud_keys_reorder", { serviceId, keyIds });
export const cloudEnginesGet = () => invoke<Engines>("cloud_engines_get");
export const cloudEnginesSet = (step: CloudStep, engine: string) =>
  invoke<Engines>("cloud_engines_set", { step, engine });
export const cloudNotice = (serviceId: string, step: CloudStep, estimatedChars: number) =>
  invoke<Notice>("cloud_notice", { serviceId, step, estimatedChars });
export const cloudConsentList = () => invoke<ConsentRecord[]>("cloud_consent_list");
export const cloudConsentGrant = (serviceId: string, step: CloudStep, estimatedChars: number) =>
  invoke<ConsentRecord>("cloud_consent_grant", { serviceId, step, estimatedChars });
export const cloudConsentWithdraw = (serviceId: string, step: CloudStep) =>
  invoke<void>("cloud_consent_withdraw", { serviceId, step });

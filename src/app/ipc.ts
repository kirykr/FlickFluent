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
  EditorState,
  EditResult,
  FitMode,
  FitReport,
  ImportChoice,
  ImportPreview,
  CaptionTemplate,
  SubtitleFormat,
  SubtitleSource,
  SpeakerGender,
  LineEdit,
  LineInsert,
  SearchMatch,
  SearchQuery,
} from "../features/editor/types";
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
export const dubStart = (handle: string, discardEdits = false) =>
  invoke<string>("dub_start", { handle, fallbackChoices: null, discardEdits });
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

export const editorOpen = (handle: string) => invoke<EditorState>("editor_open", { handle });
export const editorState = (handle: string) => invoke<EditorState>("editor_state", { handle });
export const editorUpdateLine = (handle: string, edit: LineEdit) =>
  invoke<EditResult>("editor_update_line", { handle, edit });
export const editorInsertLine = (handle: string, edit: LineInsert) =>
  invoke<EditResult>("editor_insert_line", { handle, edit });
export const editorDeleteLines = (handle: string, ids: string[]) =>
  invoke<EditResult>("editor_delete_lines", { handle, ids });
export const editorUndo = (handle: string) => invoke<EditResult>("editor_undo", { handle });
export const editorRedo = (handle: string) => invoke<EditResult>("editor_redo", { handle });
export const editorSearch = (handle: string, q: SearchQuery) =>
  invoke<{ matches: SearchMatch[] }>("editor_search", {
    handle,
    query: q.query,
    field: q.field,
    matchCase: q.matchCase,
    wholeWord: q.wholeWord,
  });
export const editorReplace = (handle: string, matches: SearchMatch[], replacement: string) =>
  invoke<EditResult>("editor_replace", { handle, matches, replacement });
export const editorResynthesize = (handle: string, ids?: string[]) =>
  invoke<string>("editor_resynthesize", { handle, ids: ids ?? null });
export const editorFit = (handle: string, mode: FitMode, ids?: string[]) =>
  invoke<FitReport>("editor_fit", { handle, mode, ids: ids ?? null });
export const subtitlesExport = (
  handle: string,
  source: SubtitleSource,
  format: SubtitleFormat,
  path: string,
) => invoke<{ cueCount: number }>("subtitles_export", { handle, source, format, path });
export const subtitlesImportPreview = (handle: string, path: string) =>
  invoke<ImportPreview>("subtitles_import_preview", { handle, path });
export const subtitlesImportApply = (handle: string, previewId: string, choice: ImportChoice) =>
  invoke<EditResult>("subtitles_import_apply", { handle, previewId, choice });
export const captionTemplatesList = () => invoke<CaptionTemplate[]>("caption_templates_list");
export const captionTemplateSet = (handle: string, id: string) =>
  invoke<void>("caption_template_set", { handle, id });
export const editorMergeLines = (handle: string, ids: string[]) =>
  invoke<EditResult>("editor_merge_lines", { handle, ids });
export const editorSplitLine = (handle: string, id: string, textOffset: number) =>
  invoke<EditResult>("editor_split_line", { handle, id, textOffset });
export const editorDuplicateLine = (handle: string, id: string) =>
  invoke<EditResult>("editor_duplicate_line", { handle, id });
export const editorBulkText = (handle: string, texts: { id: string; translated: string }[]) =>
  invoke<EditResult>("editor_bulk_text", { handle, texts });
export const editorSetSpeakerGender = (handle: string, speakerId: string, gender: SpeakerGender) =>
  invoke<EditResult>("editor_set_speaker_gender", { handle, speakerId, gender });
export const editorStop = (handle: string) => invoke<void>("editor_stop", { handle });

export const languagesList = (optIn: string[] = []) =>
  invoke<Language[]>("languages_list", { optIn });
export const languagesCheckPair = (source: string, target: string, optIn: string[] = []) =>
  invoke<PairCheck>("languages_check_pair", { source, target, optIn });

export const cloudServices = () => invoke<CloudService[]>("cloud_services");
export const cloudKeysList = () => invoke<CloudKey[]>("cloud_keys_list");
export const cloudKeyAdd = (input: AddKeyInput) => invoke<CloudKey>("cloud_key_add", { ...input });
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

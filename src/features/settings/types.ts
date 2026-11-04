export type Theme = "light" | "dark" | "system";

export interface Settings {
  settingsVersion: number;
  theme: Theme;
  general: { uiLanguage: string; defaultSaveFolder: string | null };
  /** Action name to combo, for example `project.save` to `Mod+S`. */
  hotkeys: Record<string, string>;
  analyticsEnabled: boolean;
  modelsFolder: string | null;
  lastCompatMode: "fullLocal" | "hybrid" | "blocked" | null;
}

export interface SettingsView {
  settings: Settings;
  /** Message key shown once, for example when the settings file had to be replaced. */
  notice: string | null;
}

/** Actions that can have a shortcut, in display order. */
export const HOTKEY_ACTIONS = ["project.new", "project.open", "project.save", "project.close"];

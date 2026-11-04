import { ProjectsScreen } from "../features/projects/ProjectsScreen";
import { ModelsScreen } from "../features/models/ModelsScreen";
import { SettingsScreen } from "../features/settings/SettingsScreen";
import type { Settings } from "../features/settings/types";
import type { Mode } from "../features/compat/types";

export type Page = "projects" | "models" | "settings";

interface Props {
  page: Page;
  settings: Settings;
  onSettings: (s: Settings) => void;
  onRecheck: () => Promise<Mode>;
}

/** Both screens stay mounted so an open project is not lost when the user visits settings. */
export function Routes({ page, settings, onSettings, onRecheck }: Props) {
  return (
    <>
      <div className="h-full" hidden={page !== "projects"}>
        <ProjectsScreen defaultSaveFolder={settings.general.defaultSaveFolder} />
      </div>
      <div hidden={page !== "models"}>
        <ModelsScreen />
      </div>
      <div hidden={page !== "settings"}>
        <SettingsScreen settings={settings} onSettings={onSettings} onRecheck={onRecheck} />
      </div>
    </>
  );
}

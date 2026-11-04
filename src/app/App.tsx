import { Folders, Monitor, Moon, Settings as SettingsIcon, Sun, Boxes } from "lucide-react";
import { useEffect, useState } from "react";
import { t } from "../i18n";
import { CompatMessage, CompatScreen, HybridNotice } from "../features/compat/CompatScreen";
import type { MachineCapabilityReport, Mode } from "../features/compat/types";
import { actionForEvent } from "../features/settings/hotkeys";
import type { Settings, Theme } from "../features/settings/types";
import { runCompatCheck, settingsGet, settingsSet } from "./ipc";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { Routes, type Page } from "./routes";
import { MemoryBadge } from "../features/status/MemoryBadge";
import { StatusBar } from "./StatusBar";

/** Used until the core answers, or if it cannot. Mirrors the core defaults. */
const FALLBACK_SETTINGS: Settings = {
  settingsVersion: 1,
  theme: "system",
  general: { uiLanguage: "en", defaultSaveFolder: null },
  hotkeys: {},
  analyticsEnabled: false,
  modelsFolder: null,
  lastCompatMode: null,
};

const NAV_ICONS = { projects: Folders, models: Boxes, settings: SettingsIcon } as const;
const THEMES = [
  { value: "light", icon: Sun, label: "app.theme.light" },
  { value: "dark", icon: Moon, label: "app.theme.dark" },
  { value: "system", icon: Monitor, label: "app.theme.system" },
] as const;

export default function App() {
  const [report, setReport] = useState<MachineCapabilityReport | null>(null);
  const [failed, setFailed] = useState(false);
  const [noticeDismissed, setNoticeDismissed] = useState(false);
  const [page, setPage] = useState<Page>("projects");
  const [settings, setSettings] = useState<Settings>(FALLBACK_SETTINGS);
  const [settingsNotice, setSettingsNotice] = useState<string | null>(null);

  // The check runs on every launch. It is quick, and it keeps the mode right after an upgrade.
  useEffect(() => {
    runCompatCheck()
      .then(setReport)
      .catch(() => setFailed(true));
  }, []);

  useEffect(() => {
    settingsGet()
      .then((view) => {
        setSettings(view.settings);
        // The core reports a notice only once, and dev mode runs this effect twice.
        setSettingsNotice((prev) => view.notice ?? prev);
      })
      .catch(() => setSettings(FALLBACK_SETTINGS));
  }, []);

  // Theme: `system` removes the override so the computer's own light or dark setting applies.
  useEffect(() => {
    const root = document.documentElement;
    if (settings.theme === "system") delete root.dataset.theme;
    else root.dataset.theme = settings.theme;
  }, [settings.theme]);

  // Keyboard shortcuts. The settings screen records new ones in the capture phase and stops the
  // event, so this listener never sees a key that is being recorded.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const action = actionForEvent(settings.hotkeys, e);
      if (!action) return;
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("flick:action", { detail: action }));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settings.hotkeys]);

  const recheck = async (): Promise<Mode> => {
    const next = await runCompatCheck();
    setReport(next); // a blocked result replaces the whole app with the blocked screen
    return next.mode;
  };

  // The header toggle saves through the same call as the settings screen.
  const chooseTheme = (theme: Theme) =>
    settingsSet({ theme })
      .then((view) => setSettings(view.settings))
      .catch(() => {});

  if (failed) return <CompatMessage>{t("compat.checkFailed")}</CompatMessage>;
  if (!report) return <CompatMessage>{t("compat.checking")}</CompatMessage>;
  if (report.mode === "blocked") return <CompatScreen report={report} />;

  return (
    <TooltipProvider>
      <div className="grid h-full grid-rows-[auto_1fr_auto]">
        <header className="flex items-center gap-4 border-b bg-card px-4 py-2">
          <Avatar>
            <AvatarFallback className="bg-primary font-semibold text-primary-foreground">
              F
            </AvatarFallback>
          </Avatar>
          <div className="leading-tight">
            <strong className="text-sm">{t("app.name")}</strong>
            <p className="text-xs text-muted-foreground">{t("app.tagline")}</p>
          </div>
          <Badge>PRO</Badge>
          <nav className="mx-auto">
            <ButtonGroup>
              {(["projects", "models", "settings"] as const).map((p) => {
                const Icon = NAV_ICONS[p];
                return (
                  <Button
                    key={p}
                    variant={page === p ? "default" : "outline"}
                    aria-pressed={page === p}
                    onClick={() => setPage(p)}
                  >
                    <Icon data-icon="inline-start" />
                    {t(`nav.${p}`)}
                  </Button>
                );
              })}
            </ButtonGroup>
          </nav>
          <ButtonGroup>
            {THEMES.map(({ value, icon: Icon, label }) => (
              <Tooltip key={value}>
                <TooltipTrigger
                  render={
                    <Button
                      size="icon"
                      variant={settings.theme === value ? "default" : "outline"}
                      aria-label={t(label)}
                      aria-pressed={settings.theme === value}
                      onClick={() => chooseTheme(value)}
                    />
                  }
                >
                  <Icon />
                </TooltipTrigger>
                <TooltipContent>{t(label)}</TooltipContent>
              </Tooltip>
            ))}
          </ButtonGroup>
        </header>
        <div className="flex min-h-0 flex-col gap-3 overflow-auto p-3">
          {settingsNotice && (
            <Alert role="status">
              <AlertDescription>{t(settingsNotice)}</AlertDescription>
              <AlertAction>
                <Button size="sm" variant="outline" onClick={() => setSettingsNotice(null)}>
                  {t("settings.dismiss")}
                </Button>
              </AlertAction>
            </Alert>
          )}
          {report.mode === "hybrid" && !noticeDismissed && (
            <HybridNotice onDismiss={() => setNoticeDismissed(true)} />
          )}
          <div className="min-h-0 flex-1">
            <Routes page={page} settings={settings} onSettings={setSettings} onRecheck={recheck} />
          </div>
        </div>
        <StatusBar mode={report.mode}>
          <MemoryBadge />
        </StatusBar>
      </div>
    </TooltipProvider>
  );
}

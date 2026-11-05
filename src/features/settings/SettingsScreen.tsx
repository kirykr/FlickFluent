import { AboutCard } from "./AboutCard";
import { CloudCard } from "../cloud/CloudCard";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { settingsReset, settingsSet } from "../../app/ipc";
import { asAppError } from "../projects/errors";
import type { AppErrorShape } from "../projects/types";
import type { Mode } from "../compat/types";
import { comboFromEvent, displayCombo } from "./hotkeys";
import { HOTKEY_ACTIONS, type Settings, type Theme } from "./types";

interface Props {
  settings: Settings;
  onSettings: (s: Settings) => void;
  /** Re-runs the hardware check; resolves with the new mode. */
  onRecheck: () => Promise<Mode>;
}

interface Conflict {
  action: string;
  combo: string;
  owner: string;
}

export function SettingsScreen({ settings, onSettings, onRecheck }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [capturing, setCapturing] = useState<string | null>(null);
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [rechecked, setRechecked] = useState<Mode | null>(null);
  const [folder, setFolder] = useState(settings.general.defaultSaveFolder ?? "");

  // Keep the folder box in step if the settings change from elsewhere (for example a reset).
  useEffect(() => setFolder(settings.general.defaultSaveFolder ?? ""), [settings]);

  const apply = async (patch: Record<string, unknown>): Promise<AppErrorShape | null> => {
    setError(null);
    try {
      onSettings((await settingsSet(patch)).settings);
      return null;
    } catch (e) {
      const err = asAppError(e);
      if (err.code !== "HOTKEY_CONFLICT") {
        setError(
          err.code === "INVALID_SETTING" ? "settings.error.invalid" : "settings.error.generic",
        );
      }
      return err;
    }
  };

  // While a shortcut is being recorded, take every key press before the rest of the app sees it.
  useEffect(() => {
    if (!capturing) return;
    const action = capturing;
    const onKey = async (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.key === "Escape") return setCapturing(null);
      const combo = comboFromEvent(e);
      if (!combo) return; // a lone modifier: keep waiting
      setCapturing(null);
      const err = await apply({ hotkeys: { [action]: combo } });
      if (err?.code === "HOTKEY_CONFLICT") {
        const d = (err.details ?? {}) as { action?: string; combo?: string };
        setConflict({ action, combo: d.combo ?? combo, owner: d.action ?? "" });
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [capturing]);

  const resolveConflict = async () => {
    if (!conflict) return;
    await apply({ hotkeys: { [conflict.action]: conflict.combo, [conflict.owner]: null } });
    setConflict(null);
  };

  const reset = async () => {
    if (!window.confirm(t("settings.confirmReset"))) return;
    setError(null);
    try {
      onSettings((await settingsReset()).settings);
    } catch {
      setError("settings.error.generic");
    }
  };

  const recheck = async () => setRechecked(await onRecheck());

  return (
    <section className="mx-auto flex max-w-2xl flex-col gap-4 py-4">
      <h1 className="text-xl font-semibold">{t("settings.title")}</h1>
      {error && (
        <Alert variant="destructive" role="alert">
          <AlertDescription>{t(error)}</AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.general")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-sm">
            {t("settings.language")}: {t("settings.languageValue")}
          </p>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="default-folder">{t("settings.defaultFolder")}</Label>
            <Input
              id="default-folder"
              value={folder}
              onChange={(e) => setFolder(e.target.value)}
              onBlur={() => apply({ general: { defaultSaveFolder: folder.trim() || null } })}
            />
            <p className="text-xs text-muted-foreground">{t("settings.defaultFolderHint")}</p>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="theme">{t("settings.theme")}</Label>
            {/* A native select: the shadcn Select has no real <select>, and the e2e tests drive this one. */}
            <select
              id="theme"
              className="h-8 w-48 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
              value={settings.theme}
              onChange={(e) => apply({ theme: e.target.value as Theme })}
            >
              <option value="system">{t("settings.themeSystem")}</option>
              <option value="light">{t("settings.themeLight")}</option>
              <option value="dark">{t("settings.themeDark")}</option>
            </select>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.hotkeys")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Table>
            <TableBody>
              {HOTKEY_ACTIONS.map((action) => (
                <TableRow key={action}>
                  <TableCell>{t(`settings.action.${action}`)}</TableCell>
                  <TableCell data-testid={`combo-${action}`} className="font-mono text-xs">
                    {capturing === action
                      ? t("settings.pressKeys")
                      : settings.hotkeys[action]
                        ? displayCombo(settings.hotkeys[action])
                        : t("settings.unassigned")}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="outline"
                      size="sm"
                      aria-label={`${t("settings.change")}: ${t(`settings.action.${action}`)}`}
                      onClick={() => setCapturing(action)}
                    >
                      {t("settings.change")}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          {conflict && (
            <Alert role="alertdialog">
              <AlertTitle>{t("settings.conflictTitle")}</AlertTitle>
              <AlertDescription>
                <p>
                  {t("settings.conflictBody", {
                    combo: displayCombo(conflict.combo),
                    owner: t(`settings.action.${conflict.owner}`),
                    action: t(`settings.action.${conflict.action}`),
                  })}
                </p>
                <div className="mt-2 flex gap-2">
                  <Button onClick={resolveConflict}>{t("settings.conflictUse")}</Button>
                  <Button variant="outline" onClick={() => setConflict(null)}>
                    {t("settings.conflictKeep")}
                  </Button>
                </div>
              </AlertDescription>
            </Alert>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.analytics")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <Switch
              aria-label={t("settings.analytics")}
              checked={settings.analyticsEnabled}
              onCheckedChange={(checked) => apply({ analyticsEnabled: checked })}
            />
            <span className="text-sm">{t("settings.analytics")}</span>
          </div>
          {settings.analyticsEnabled && (
            <p className="text-xs text-muted-foreground">{t("settings.analyticsDetail")}</p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.compat")}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-3">
          <Button variant="outline" onClick={recheck}>
            {t("settings.recheck")}
          </Button>
          {rechecked && (
            <span className="text-sm">
              {t("settings.rechecked", { mode: t(`mode.${rechecked}`) })}
            </span>
          )}
          <Button variant="destructive" className="ml-auto" onClick={reset}>
            {t("settings.reset")}
          </Button>
        </CardContent>
      </Card>
      <CloudCard />
      <AboutCard />
    </section>
  );
}

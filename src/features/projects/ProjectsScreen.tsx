import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Trash2 } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { DubScreen } from "../dub/DubScreen";
import { EditorLayout } from "../editor/EditorLayout";
import {
  projectClose,
  projectDelete,
  projectDiscardRecovery,
  projectNew,
  projectOpen,
  projectRecent,
  projectRecover,
  projectSave,
  projectSetNotes,
  projectSuggestFileName,
} from "../../app/ipc";
import { asAppError, canRestoreBackup, errorKey } from "./errors";
import type { AppErrorShape, ProjectHandle, RecentEntry } from "./types";

const FILTERS = [{ name: "FlickFluent project", extensions: ["flick"] }];

export function ProjectsScreen({ defaultSaveFolder }: { defaultSaveFolder?: string | null }) {
  const [current, setCurrent] = useState<ProjectHandle | null>(null);
  const [dubOpen, setDubOpen] = useState(false);
  const [recent, setRecent] = useState<RecentEntry[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<AppErrorShape | null>(null);
  /** File that has a recovery copy waiting for a yes or no. */
  const [recoveryFor, setRecoveryFor] = useState<string | null>(null);
  /** Damaged file that has a backup the user can restore. */
  const [damagedPath, setDamagedPath] = useState<string | null>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  const actions = useRef<Record<string, () => void>>({});

  const refresh = useCallback(() => {
    projectRecent()
      .then(setRecent)
      .catch(() => setRecent([]));
  }, []);
  useEffect(refresh, [refresh]);

  // Every action runs through here so errors always end up on screen.
  const attempt = async (action: () => Promise<void>, path?: string) => {
    setError(null);
    try {
      await action();
    } catch (e) {
      const err = asAppError(e);
      setError(err);
      if (path && canRestoreBackup(err)) setDamagedPath(path);
    }
  };

  const openPath = (path: string) =>
    attempt(async () => {
      setDamagedPath(null);
      const { handle, recoveryAvailable } = await projectOpen(path);
      setCurrent(handle);
      setRecoveryFor(recoveryAvailable ? path : null);
      refresh();
    }, path);

  const create = () =>
    attempt(async () => {
      const fileName = await projectSuggestFileName(name);
      const path = await save({
        defaultPath: defaultSaveFolder
          ? `${defaultSaveFolder.replace(/[\\/]+$/, "")}/${fileName}`
          : fileName,
        filters: FILTERS,
      });
      if (!path) return;
      setCurrent(await projectNew(name.trim(), path));
      setName("");
      refresh();
    });

  const openDialog = () =>
    attempt(async () => {
      const path = await open({ multiple: false, filters: FILTERS });
      if (typeof path === "string") await openPath(path);
    });

  const restore = (path: string) =>
    attempt(async () => {
      setCurrent(await projectRecover(path));
      setRecoveryFor(null);
      setDamagedPath(null);
      setError(null);
      refresh();
    });

  const discard = (path: string) =>
    attempt(async () => {
      await projectDiscardRecovery(path);
      setRecoveryFor(null);
    });

  const saveAs = () =>
    attempt(async () => {
      if (!current) return;
      const path = await save({ defaultPath: current.path, filters: FILTERS });
      if (path) setCurrent(await projectSave(current.id, path));
      refresh();
    });

  const closeProject = () =>
    attempt(async () => {
      if (current) await projectClose(current.id);
      setCurrent(null);
      setRecoveryFor(null);
      refresh();
    });

  const remove = (entry: RecentEntry) =>
    attempt(async () => {
      if (!window.confirm(t("projects.confirmDelete"))) return;
      await projectDelete(entry.path);
      refresh();
    });

  // Keyboard shortcuts arrive as `flick:action` events from the app shell. The ref keeps the
  // handlers current without re-registering the listener on every render.
  actions.current = {
    "project.new": () => nameInput.current?.focus(),
    "project.open": () => void openDialog(),
    "project.save": () => {
      if (current && !current.readOnly) {
        void attempt(async () => setCurrent(await projectSave(current.id)));
      }
    },
    "project.close": () => void closeProject(),
  };
  useEffect(() => {
    const onAction = (e: Event) => actions.current[(e as CustomEvent<string>).detail]?.();
    window.addEventListener("flick:action", onAction);
    return () => window.removeEventListener("flick:action", onAction);
  }, []);

  const notices = (
    <>
      {error && (
        <Alert variant="destructive" role="alert">
          <AlertDescription>{t(errorKey(error))}</AlertDescription>
        </Alert>
      )}

      {damagedPath && (
        <Alert role="alertdialog">
          <AlertTitle>{t("projects.damagedTitle")}</AlertTitle>
          <AlertDescription>
            <p>{t("projects.damagedBackup")}</p>
            <Button className="mt-2" onClick={() => restore(damagedPath)}>
              {t("projects.recoverBackup")}
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {recoveryFor && (
        <Alert role="alertdialog">
          <AlertTitle>{t("projects.recoveryTitle")}</AlertTitle>
          <AlertDescription>
            <p>{t("projects.recoveryBody")}</p>
            <div className="mt-2 flex gap-2">
              <Button onClick={() => restore(recoveryFor)}>{t("projects.recoverYes")}</Button>
              <Button variant="outline" onClick={() => discard(recoveryFor)}>
                {t("projects.recoverNo")}
              </Button>
            </div>
          </AlertDescription>
        </Alert>
      )}
    </>
  );

  if (current) {
    const saveCurrent = () => attempt(async () => setCurrent(await projectSave(current.id)));
    return (
      <section className="flex h-full min-h-0 flex-col gap-3">
        {notices}
        <EditorLayout
          main={
            dubOpen ? (
              <DubScreen key={current.id} handle={current.id} readOnly={current.readOnly} />
            ) : undefined
          }
          header={
            <Card size="sm">
              <CardContent className="flex flex-wrap items-start gap-x-6 gap-y-2">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="truncate text-base font-semibold">{current.name}</h2>
                    <Badge
                      variant={current.dirty ? "default" : "secondary"}
                      data-testid="save-state"
                    >
                      {current.dirty ? t("projects.unsaved") : t("projects.saved")}
                    </Badge>
                    <span className="flex-1" />
                    <Button
                      size="sm"
                      variant={dubOpen ? "secondary" : "default"}
                      onClick={() => setDubOpen((v) => !v)}
                      data-testid="dub-toggle"
                    >
                      {dubOpen ? t("dub.backToEditor") : t("dub.open")}
                    </Button>
                    <Button size="sm" disabled={current.readOnly} onClick={saveCurrent}>
                      {t("projects.save")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={current.readOnly}
                      onClick={saveAs}
                    >
                      {t("projects.saveAs")}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={closeProject}>
                      {t("projects.close")}
                    </Button>
                  </div>
                  <p className="truncate text-xs text-muted-foreground">{current.path}</p>
                  {current.readOnly && (
                    <p className="mt-1 text-xs text-destructive">{t("projects.readOnly")}</p>
                  )}
                  <p className="mt-1 text-xs text-muted-foreground">{t("projects.notesHint")}</p>
                </div>
                <div className="flex w-full max-w-md flex-col gap-1">
                  <Label htmlFor="project-notes">{t("projects.notes")}</Label>
                  <Textarea
                    id="project-notes"
                    className="h-14 resize-none"
                    value={current.notes}
                    disabled={current.readOnly}
                    onChange={(e) =>
                      attempt(async () =>
                        setCurrent(await projectSetNotes(current.id, e.target.value)),
                      )
                    }
                  />
                </div>
              </CardContent>
            </Card>
          }
        />
      </section>
    );
  }

  return (
    <section className="mx-auto flex max-w-2xl flex-col gap-4 py-4">
      <h1 className="text-xl font-semibold">{t("projects.title")}</h1>
      {notices}

      <Card>
        <CardHeader>
          <CardTitle>{t("projects.newTitle")}</CardTitle>
          <CardDescription>{t("app.tagline")}</CardDescription>
        </CardHeader>
        <CardContent className="flex gap-2">
          <Input
            ref={nameInput}
            aria-label={t("projects.namePlaceholder")}
            placeholder={t("projects.namePlaceholder")}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <Button disabled={!name.trim()} onClick={create}>
            <Plus data-icon="inline-start" />
            {t("projects.create")}
          </Button>
          <Button variant="outline" onClick={openDialog}>
            <FolderOpen data-icon="inline-start" />
            {t("projects.open")}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("projects.recent")}</CardTitle>
        </CardHeader>
        <CardContent>
          {recent.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("projects.noRecent")}</p>
          ) : (
            <ul className="flex flex-col divide-y">
              {recent.map((r) => (
                <li key={r.path} className="flex items-center gap-3 py-2">
                  <Button variant="link" disabled={!r.available} onClick={() => openPath(r.path)}>
                    {r.name}
                  </Button>
                  <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
                    {r.available ? r.path : t("projects.unavailable")}
                  </span>
                  <Button variant="ghost" size="sm" onClick={() => remove(r)}>
                    <Trash2 data-icon="inline-start" />
                    {t("projects.delete")}
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </section>
  );
}

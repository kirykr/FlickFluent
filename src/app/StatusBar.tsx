import type { ReactNode } from "react";
import { t } from "../i18n";
import type { Mode } from "../features/compat/types";

/** Bottom bar. `children` is the slot for the memory badge (feature 001, story 5). */
export function StatusBar({ mode, children }: { mode: Mode; children?: ReactNode }) {
  return (
    <footer className="flex items-center gap-3 border-t bg-card px-4 py-1.5 text-xs text-muted-foreground">
      <span data-testid="mode">{t(`mode.${mode}`)}</span>
      <span className="flex-1" />
      {children}
    </footer>
  );
}

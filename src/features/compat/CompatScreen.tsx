import { OctagonAlert } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { itemLabelKey, rowsToShow } from "./format";
import type { MachineCapabilityReport } from "./types";

/** Message shown in place of the app while the machine check runs or fails. */
export function CompatMessage({ children }: { children: string }) {
  return (
    <main className="grid h-full place-items-center p-6 text-muted-foreground">{children}</main>
  );
}

/** Full-window screen for a blocked machine. There is no way to continue from here. */
export function CompatScreen({ report }: { report: MachineCapabilityReport }) {
  const rows = rowsToShow(report.missing);
  return (
    <main className="mx-auto flex h-full max-w-2xl items-center p-6" role="alert">
      <Card className="w-full">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-lg">
            <OctagonAlert className="size-5 text-destructive" />
            <h1>{t("compat.blockedTitle")}</h1>
          </CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p>{report.blockedMessage}</p>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("compat.tableItem")}</TableHead>
                <TableHead>{t("compat.tableNeeded")}</TableHead>
                <TableHead>{t("compat.tableFound")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.key}>
                  <TableCell>{t(itemLabelKey(row))}</TableCell>
                  <TableCell>{row.neededLabel}</TableCell>
                  <TableCell>{row.foundLabel}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <p>{t("compat.blockedHint")}</p>
          {report.provisional && (
            <p className="text-xs text-muted-foreground">{t("compat.provisional")}</p>
          )}
        </CardContent>
      </Card>
    </main>
  );
}

/** Dismissible notice for a machine that runs in hybrid mode. */
export function HybridNotice({ onDismiss }: { onDismiss: () => void }) {
  return (
    <Alert role="status">
      <AlertTitle>{t("compat.hybridTitle")}</AlertTitle>
      <AlertDescription>{t("compat.hybridNotice")}</AlertDescription>
      <AlertAction>
        <Button size="sm" variant="outline" onClick={onDismiss}>
          {t("compat.hybridDismiss")}
        </Button>
      </AlertAction>
    </Alert>
  );
}

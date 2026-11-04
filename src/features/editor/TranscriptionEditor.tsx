import { FileText, Mars, Pencil, Plus, Search, Trash2, Venus, Volume2 } from "lucide-react";
import { t } from "../../i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ComingSoon } from "./ComingSoon";
import { SAMPLE_LINES } from "./placeholder";

/** Right column of the editor: toolbar and a table of sample lines. */
export function TranscriptionEditor() {
  return (
    <Card className="h-full min-h-0">
      <CardHeader className="flex flex-row flex-wrap items-center gap-2">
        <FileText className="size-4 text-primary" />
        <CardTitle>{t("editor.transcription")}</CardTitle>
        {[
          { icon: Plus, key: "insert" },
          { icon: Pencil, key: "edit" },
          { icon: Trash2, key: "delete" },
          { icon: Search, key: "searchReplace" },
        ].map(({ icon: Icon, key }) => (
          <ComingSoon key={key}>
            <Button size="sm" variant="ghost" disabled>
              <Icon data-icon="inline-start" />
              {t(`editor.${key}`)}
            </Button>
          </ComingSoon>
        ))}
        <span className="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
          {t("editor.voiceActor")}
          <Badge variant="secondary">
            <Mars /> {t("editor.male")}
          </Badge>
          <Badge variant="secondary">
            <Venus /> {t("editor.female")}
          </Badge>
        </span>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
        <ScrollArea className="min-h-0 flex-1">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-8">
                  <Checkbox disabled checked aria-label={t("editor.selectAll")} />
                </TableHead>
                <TableHead>{t("editor.colStart")}</TableHead>
                <TableHead>{t("editor.colEnd")}</TableHead>
                <TableHead className="w-full">{t("editor.colText")}</TableHead>
                <TableHead>{t("editor.colVoice")}</TableHead>
                <TableHead>{t("editor.colStatus")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {SAMPLE_LINES.map((line, i) => (
                <TableRow key={line.start}>
                  <TableCell>
                    <Checkbox disabled checked aria-label={t("editor.selectLine", { n: i + 1 })} />
                  </TableCell>
                  <TableCell className="font-mono text-xs text-primary">{line.start}</TableCell>
                  <TableCell className="font-mono text-xs text-primary">{line.end}</TableCell>
                  <TableCell className="whitespace-normal">{line.text}</TableCell>
                  <TableCell>
                    <Badge variant="outline">
                      {line.voice === "male" ? <Mars /> : <Venus />}
                      {t(`editor.${line.voice}`)}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <Volume2 className="size-4 text-muted-foreground" />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </ScrollArea>
        <p className="text-xs text-muted-foreground">{t("editor.sample")}</p>
        <ComingSoon className="block">
          <Button disabled size="lg" className="w-full">
            {t("editor.exportProject")}
          </Button>
        </ComingSoon>
      </CardContent>
    </Card>
  );
}

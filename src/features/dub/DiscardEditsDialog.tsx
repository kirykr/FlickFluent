import { t } from "../../i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface Props {
  /** How many edited lines a new dub would replace; 0 keeps the dialog closed. */
  count: number;
  open: boolean;
  onKeep: () => void;
  onReplace: () => void;
}

/** A new dub replaces every line. Says how many edited ones that includes, and waits for a yes. */
export function DiscardEditsDialog({ count, open, onKeep, onReplace }: Props) {
  return (
    <Dialog open={open && count > 0} onOpenChange={(o) => !o && onKeep()}>
      <DialogContent data-testid="discard-edits">
        <DialogHeader>
          <DialogTitle>{t("dub.new.discard.title")}</DialogTitle>
          <DialogDescription>{t("dub.new.discard.body", { count })}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onKeep}>
            {t("dub.new.discard.keep")}
          </Button>
          <Button variant="destructive" onClick={onReplace}>
            {t("dub.new.discard.replace")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

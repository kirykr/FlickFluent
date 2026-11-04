import type { ReactElement } from "react";
import { t } from "../../i18n";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** Wraps a disabled control so it still explains itself on hover. */
export function ComingSoon({
  children,
  className,
}: {
  children: ReactElement;
  className?: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger render={<span className={className} />}>{children}</TooltipTrigger>
      <TooltipContent>{t("editor.comingSoon")}</TooltipContent>
    </Tooltip>
  );
}

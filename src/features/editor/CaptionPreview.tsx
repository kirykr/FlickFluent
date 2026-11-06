import { t } from "../../i18n";
import type { CaptionTemplate } from "./types";

/** `#rrggbbaa` as a CSS colour; `#rrggbb` is passed through. */
export function cssColour(c: string): string {
  if (c.length !== 9) return c;
  const n = (i: number) => parseInt(c.slice(i, i + 2), 16);
  return `rgba(${n(1)}, ${n(3)}, ${n(5)}, ${(n(7) / 255).toFixed(2)})`;
}

/** The current line drawn over the video frame the way the template styles it. */
export function CaptionPreview({
  template,
  text,
}: {
  template: CaptionTemplate | undefined;
  text: string;
}) {
  if (!template || !text) return null;
  const w = template.outlineWidth;
  const o = cssColour(template.outlineColor);
  const outline =
    w > 0
      ? `-${w}px -${w}px 0 ${o}, ${w}px -${w}px 0 ${o}, -${w}px ${w}px 0 ${o}, ${w}px ${w}px 0 ${o}`
      : undefined;
  return (
    <div
      className="pointer-events-none absolute inset-x-0 flex justify-center px-2"
      style={{ [template.position]: `${template.marginPercent}%` }}
      data-testid="caption-preview"
      data-template={template.id}
      aria-label={t("editor.captions.preview")}
    >
      <span
        className="rounded px-1.5 text-center whitespace-pre-line"
        style={{
          fontFamily: template.fontFamily,
          // The frame is a size container, so `cqh` is a share of the frame's height.
          fontSize: `${template.sizePercent}cqh`,
          lineHeight: 1.25,
          color: cssColour(template.color),
          textShadow: outline,
          background: template.background ? cssColour(template.background) : undefined,
        }}
      >
        {text}
      </span>
    </div>
  );
}

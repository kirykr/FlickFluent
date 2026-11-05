import { TriangleAlert } from "lucide-react";
import { t } from "../../i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { pairResolved } from "./languages";
import type { FallbackChoices, FallbackKind, PairCheck as Check, PairFallback } from "./types";

/** The app listens for this and shows the Settings page. */
export const OPEN_SETTINGS = "flickfluent:open-settings";

interface Props {
  check: Check | null;
  choices: FallbackChoices;
  onChoices: (next: FallbackChoices) => void;
  /** Non-commercial models the user has already opted in to, for the checkbox state. */
  readOnly?: boolean;
  /** Cloud service names by id, for the cloud fallback label. */
  serviceNames?: Record<string, string>;
}

const FLAG: Partial<Record<FallbackKind, "subtitlesOnly" | "genericVoice" | "pivotViaEnglish">> = {
  subtitlesOnly: "subtitlesOnly",
  genericVoice: "genericVoice",
  pivotViaEnglish: "pivotViaEnglish",
};

/**
 * Names the steps a language pair cannot do and offers the fallbacks, before anything is
 * processed. Start stays blocked until the user picks a fix for each limited step.
 */
export function PairCheck({ check, choices, onChoices, readOnly, serviceNames = {} }: Props) {
  if (!check) return null;
  const limited = check.steps.filter((s) => !s.supported);
  const weak = check.steps.filter((s) => s.supported && s.quality === "weak");

  if (limited.length === 0) {
    return weak.length === 0 ? null : (
      <Alert data-testid="pair-weak">
        <AlertDescription>
          {weak.map((s) => t(`languages.pair.steps.${s.step}`)).join(", ")}:{" "}
          {t("languages.pair.low")}
        </AlertDescription>
      </Alert>
    );
  }

  const stepKey = (f: PairFallback): "translate" | "speak" | null =>
    f.step === "translate" || f.step === "speak" ? f.step : null;

  const checked = (f: PairFallback) => {
    if (f.kind === "optInNonCommercial") return f.modelIds.some((id) => choices.optIn.includes(id));
    if (f.kind === "cloudEngine") {
      const step = stepKey(f);
      return !!step && (f.serviceIds ?? []).includes(choices.engines?.[step] ?? "local");
    }
    const flag = FLAG[f.kind];
    return !!flag && choices[flag];
  };

  const toggle = (f: PairFallback, on: boolean) => {
    if (f.kind === "optInNonCommercial") {
      const rest = choices.optIn.filter((id) => !f.modelIds.includes(id));
      onChoices({ ...choices, optIn: on ? [...rest, ...f.modelIds] : rest });
      return;
    }
    if (f.kind === "cloudEngine") {
      const step = stepKey(f);
      if (!step) return;
      const engines = { translate: "local", speak: "local", ...choices.engines };
      engines[step] = on ? (f.serviceIds ?? [])[0] ?? "local" : "local";
      onChoices({ ...choices, engines });
      return;
    }
    const key = FLAG[f.kind];
    if (key) onChoices({ ...choices, [key]: on });
  };

  const resolved = pairResolved(check, choices);
  return (
    <Alert variant={resolved ? "default" : "destructive"} role="alert" data-testid="pair-check">
      <TriangleAlert />
      <AlertTitle>{t("languages.pair.title")}</AlertTitle>
      <AlertDescription className="flex flex-col gap-3">
        <ul className="list-disc pl-4">
          {limited.map((s) => (
            <li key={s.step} data-step={s.step}>
              <strong>{t(`languages.pair.steps.${s.step}`)}</strong>: {s.reason ? t(s.reason) : ""}
            </li>
          ))}
        </ul>
        {check.fallbacks.length > 0 && <p>{t("languages.pair.body")}</p>}
        <div className="flex flex-col gap-3">
          {check.fallbacks.map((f) => (
            <div
              key={`${f.kind}-${f.step ?? ""}`}
              className="flex flex-col gap-1"
              data-fallback={f.kind}
            >
              <div className="flex items-start gap-2">
                {f.kind === "setupCloudKey" ? (
                  <div className="flex flex-col gap-1">
                    <span>{t(f.labelKey)}</span>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => window.dispatchEvent(new CustomEvent(OPEN_SETTINGS))}
                    >
                      {t("languages.fallback.openSettings")}
                    </Button>
                  </div>
                ) : (
                  <>
                    <Checkbox
                      id={`fb-${f.kind}-${f.step ?? ""}`}
                      checked={checked(f)}
                      disabled={readOnly}
                      onCheckedChange={(on) => toggle(f, on === true)}
                    />
                    <Label htmlFor={`fb-${f.kind}-${f.step ?? ""}`} className="font-normal">
                      {t(f.labelKey, {
                        service: (f.serviceIds ?? []).map((id) => serviceNames[id] ?? id).join(", "),
                      })}
                    </Label>
                  </>
                )}
              </div>
              {f.kind === "optInNonCommercial" && (
                <div className="ml-6 text-xs" data-testid="optin-notice">
                  {f.noticeKeys.map((k) => (
                    <p key={k}>{t(k)}</p>
                  ))}
                  <p className="font-medium">{t("languages.pair.accept")}</p>
                </div>
              )}
            </div>
          ))}
        </div>
        {!resolved && check.fallbacks.length > 0 && (
          <p className="font-medium">{t("languages.pair.required")}</p>
        )}
      </AlertDescription>
    </Alert>
  );
}

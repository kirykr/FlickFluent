import type { FallbackChoices, Language, PairCheck } from "./types";

export type PickerRole = "source" | "target";

export interface LanguageOption {
  code: string;
  /** Name from locale data; empty for the auto-detect entry. */
  name: string;
  /** i18n key of a short note when the language is limited for this role, else null. */
  hintKey: string | null;
}

export const AUTO = "auto";

/** `Intl.DisplayNames` name of a code in the user's language; falls back to the code. */
export function languageName(code: string, locale = "en"): string {
  if (code === AUTO) return "";
  try {
    return new Intl.DisplayNames([locale], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** What is limited about a language for the role it is picked in. */
export function hintKey(lang: Language, role: PickerRole): string | null {
  if (role === "source") {
    if (!lang.transcribe) return "languages.hint.noRecogniser";
    return lang.quality.transcribe === "weak" ? "languages.hint.weak" : null;
  }
  if (!lang.translateTo) return "languages.hint.noTranslator";
  if (lang.speak.voices === 0) return "languages.hint.noVoice";
  return lang.quality.translate === "weak" ? "languages.hint.weak" : null;
}

/** Every catalogue language, named and sorted by name. The code stays the key. */
export function languageOptions(
  languages: Language[],
  role: PickerRole,
  locale = "en",
): LanguageOption[] {
  return languages
    .map((l) => ({ code: l.code, name: languageName(l.code, locale), hintKey: hintKey(l, role) }))
    .sort((a, b) => a.name.localeCompare(b.name, locale));
}

/** Case- and accent-insensitive match on the name or the code. An empty query matches all. */
export function matchesQuery(option: LanguageOption, query: string): boolean {
  const fold = (s: string) =>
    s
      .normalize("NFD")
      .replace(/\p{Diacritic}/gu, "")
      .toLowerCase();
  const q = fold(query.trim());
  if (!q) return true;
  return fold(option.name).includes(q) || option.code.toLowerCase().startsWith(q);
}

/**
 * Has the user chosen a fix for every step the pair cannot do? Translation needs a pivot or an
 * opt-in; speech needs subtitles only, a generic voice or an opt-in. A fully supported pair is
 * always resolved.
 */
export function pairResolved(check: PairCheck | null, choices: FallbackChoices): boolean {
  if (!check) return false;
  const missing = (step: PairCheck["steps"][number]["step"]) =>
    check.steps.some((s) => s.step === step && !s.supported);
  const optedIn = choices.optIn.length > 0;
  if (missing("transcribe")) return false; // nothing here can fix speech recognition
  if (missing("translate") && !choices.pivotViaEnglish && !optedIn) return false;
  if (missing("speak") && !choices.subtitlesOnly && !choices.genericVoice && !optedIn) return false;
  return true;
}

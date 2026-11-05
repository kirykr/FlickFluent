// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LanguagePicker } from "./LanguagePicker";
import { hintKey, languageOptions, matchesQuery } from "./languages";
import type { Language } from "./types";

const lang = (code: string, over: Partial<Language> = {}): Language => ({
  code,
  transcribe: true,
  translateFrom: true,
  translateTo: true,
  speak: { voices: 1, permissiveVoices: 1 },
  quality: { transcribe: null, translate: null, speak: null },
  ...over,
});

const LANGS = [
  lang("fr"),
  lang("de"),
  lang("km", { translateTo: false, speak: { voices: 0, permissiveVoices: 0 } }),
  lang("pt"),
];

beforeAll(() => {
  // jsdom lacks what the list and popover use.
  Element.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});
afterEach(cleanup);

describe("language options", () => {
  it("names languages from locale data, sorted by name, with the code as the key", () => {
    const options = languageOptions(LANGS, "source");
    expect(options.map((o) => o.name)).toEqual(["French", "German", "Khmer", "Portuguese"]);
    expect(options.map((o) => o.code)).toEqual(["fr", "de", "km", "pt"]);
  });

  it("matches part of a name or the start of a code, ignoring case and accents", () => {
    const [portuguese] = languageOptions([lang("pt")], "source");
    expect(matchesQuery(portuguese, "portu")).toBe(true);
    expect(matchesQuery(portuguese, "PT")).toBe(true);
    expect(matchesQuery(portuguese, "xyz")).toBe(false);
    expect(matchesQuery(portuguese, "  ")).toBe(true);
    const [french] = languageOptions([lang("fr")], "source");
    expect(matchesQuery(french, "frençh".replace("ç", "c"))).toBe(true);
  });

  it("says what is limited for the role", () => {
    const km = LANGS[2];
    expect(hintKey(km, "source")).toBeNull();
    expect(hintKey(km, "target")).toBe("languages.hint.noTranslator");
    expect(hintKey(lang("xx", { speak: { voices: 0, permissiveVoices: 0 } }), "target")).toBe(
      "languages.hint.noVoice",
    );
    expect(hintKey(lang("yy", { transcribe: false }), "source")).toBe(
      "languages.hint.noRecogniser",
    );
  });
});

describe("LanguagePicker", () => {
  const open = async (props: Partial<Parameters<typeof LanguagePicker>[0]> = {}) => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(
      <LanguagePicker
        id="p"
        languages={LANGS}
        value="fr"
        onChange={onChange}
        role="source"
        {...props}
      />,
    );
    await user.click(screen.getByRole("combobox"));
    return { onChange, user };
  };

  it("shows the selected language by name", () => {
    render(
      <LanguagePicker id="p" languages={LANGS} value="de" onChange={() => {}} role="source" />,
    );
    expect(screen.getByRole("combobox").textContent).toContain("German");
  });

  it("filters while typing and returns the code", async () => {
    const { onChange, user } = await open();
    await user.type(screen.getByPlaceholderText("Search languages…"), "port");
    const list = screen.getByRole("listbox");
    expect(within(list).getAllByRole("option")).toHaveLength(1);
    await user.click(within(list).getByText("Portuguese"));
    expect(onChange).toHaveBeenCalledWith("pt");
  });

  it("offers auto-detect for the source only", async () => {
    await open({ allowAuto: true });
    expect(screen.getByText("Detect automatically", { selector: "[role=option]" })).toBeTruthy();
    cleanup();
    await open({ allowAuto: false, role: "target" });
    expect(screen.queryByText("Detect automatically", { selector: "[role=option]" })).toBeNull();
  });

  it("shows a note on a language the role cannot fully use", async () => {
    await open({ role: "target" });
    const khmer = screen.getByText("Khmer").closest("[role=option]") as HTMLElement;
    expect(khmer.textContent).toContain("Cannot translate into it");
  });

  it("says so when nothing matches", async () => {
    const { user } = await open();
    await user.type(screen.getByPlaceholderText("Search languages…"), "zzzz");
    expect(screen.getByText("No language found.")).toBeTruthy();
  });
});

import { pairResolved } from "./languages";
import { NO_FALLBACKS, type PairCheck } from "./types";

describe("pairResolved", () => {
  const step = (s: "transcribe" | "translate" | "speak", supported: boolean) => ({
    step: s,
    supported,
    quality: null,
    reason: supported ? null : "x",
  });
  const check = (
    over: Partial<Record<"transcribe" | "translate" | "speak", boolean>>,
  ): PairCheck => ({
    ok: false,
    steps: [
      step("transcribe", over.transcribe ?? true),
      step("translate", over.translate ?? true),
      step("speak", over.speak ?? true),
    ],
    fallbacks: [],
  });

  it("blocks until a limited step has a chosen fix", () => {
    expect(pairResolved(null, NO_FALLBACKS)).toBe(false);
    expect(pairResolved(check({}), NO_FALLBACKS)).toBe(true);
    expect(pairResolved(check({ speak: false }), NO_FALLBACKS)).toBe(false);
    expect(pairResolved(check({ speak: false }), { ...NO_FALLBACKS, subtitlesOnly: true })).toBe(
      true,
    );
    expect(
      pairResolved(check({ translate: false }), { ...NO_FALLBACKS, subtitlesOnly: true }),
    ).toBe(false);
    expect(
      pairResolved(check({ translate: false }), { ...NO_FALLBACKS, pivotViaEnglish: true }),
    ).toBe(true);
    expect(pairResolved(check({ translate: false }), { ...NO_FALLBACKS, optIn: ["nllb"] })).toBe(
      true,
    );
  });

  it("cannot be fixed when speech cannot be recognised", () => {
    expect(
      pairResolved(check({ transcribe: false }), { ...NO_FALLBACKS, subtitlesOnly: true }),
    ).toBe(false);
  });
});

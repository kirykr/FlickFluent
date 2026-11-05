// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { OPEN_SETTINGS, PairCheck } from "./PairCheck";
import { pairResolved } from "./languages";
import { NO_FALLBACKS, type PairCheck as Check, type PairFallback } from "./types";

const speakMissing = (fallbacks: PairFallback[]): Check => ({
  ok: false,
  steps: [
    { step: "transcribe", supported: true, quality: null, reason: null },
    { step: "translate", supported: true, quality: null, reason: null },
    { step: "speak", supported: false, quality: null, reason: "languages.reason.noVoice" },
  ],
  fallbacks,
});

const subtitles: PairFallback = {
  kind: "subtitlesOnly",
  labelKey: "languages.fallback.subtitles",
  modelIds: [],
  noticeKeys: [],
};
const cloud: PairFallback = {
  kind: "cloudEngine",
  labelKey: "languages.fallback.cloudEngine",
  modelIds: [],
  noticeKeys: [],
  serviceIds: ["google-gemini"],
  step: "speak",
};
const setup: PairFallback = {
  kind: "setupCloudKey",
  labelKey: "languages.fallback.setupCloudKey",
  modelIds: [],
  noticeKeys: [],
  serviceIds: ["google-gemini"],
  step: "speak",
};

afterEach(cleanup);

describe("pair check with cloud engines", () => {
  it("offers the cloud service by name, next to the local fallbacks", () => {
    render(
      <PairCheck
        check={speakMissing([subtitles, cloud])}
        choices={NO_FALLBACKS}
        onChoices={() => {}}
        serviceNames={{ "google-gemini": "Google Gemini API" }}
      />,
    );
    expect(
      screen.getAllByLabelText(/Let a cloud service do this step \(Google Gemini API\)/).length,
    ).toBeGreaterThan(0);
    expect(screen.getAllByLabelText(/subtitles/i).length).toBeGreaterThan(0);
  });

  it("choosing it marks that step as cloud and resolves the pair", async () => {
    const onChoices = vi.fn();
    render(
      <PairCheck
        check={speakMissing([cloud])}
        choices={NO_FALLBACKS}
        onChoices={onChoices}
        serviceNames={{ "google-gemini": "Google Gemini API" }}
      />,
    );
    await userEvent.click(screen.getAllByRole("checkbox")[0]);
    expect(onChoices).toHaveBeenCalledWith(
      expect.objectContaining({ engines: { translate: "local", speak: "google-gemini" } }),
    );
    expect(pairResolved(speakMissing([cloud]), NO_FALLBACKS)).toBe(false);
    expect(
      pairResolved(speakMissing([cloud]), {
        ...NO_FALLBACKS,
        engines: { translate: "local", speak: "google-gemini" },
      }),
    ).toBe(true);
  });

  it("with no verified key it says to set one up and links to Settings", async () => {
    const opened = vi.fn();
    window.addEventListener(OPEN_SETTINGS, opened);
    render(<PairCheck check={speakMissing([subtitles, setup])} choices={NO_FALLBACKS} onChoices={() => {}} />);
    expect(screen.getByText(/Add a key for it in Settings first/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open Settings" }));
    expect(opened).toHaveBeenCalled();
    window.removeEventListener(OPEN_SETTINGS, opened);
  });
});

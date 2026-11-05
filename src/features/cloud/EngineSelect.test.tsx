// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const ipc = vi.hoisted(() => ({ cloudEnginesGet: vi.fn(), cloudEnginesSet: vi.fn() }));
vi.mock("../../app/ipc", () => ipc);

import { EngineSelect } from "./EngineSelect";
import type { CloudKey, CloudService } from "./types";

const gemini: CloudService = {
  id: "google-gemini",
  name: "Google Gemini API",
  steps: ["translate", "speak"],
  needsRegion: false,
  termsUrl: "https://example.com",
  languages: { translate: [], speak: [] },
};
const key = (state: CloudKey["state"]): CloudKey => ({
  id: "k1",
  serviceId: "google-gemini",
  label: "mine",
  region: null,
  last4: "1234",
  order: 0,
  state,
  verifiedAt: null,
  storage: "os",
});

beforeEach(() => {
  ipc.cloudEnginesGet.mockResolvedValue({ translate: "local", speak: "local" });
  ipc.cloudEnginesSet.mockImplementation(async (step: string, engine: string) => ({
    translate: step === "translate" ? engine : "local",
    speak: step === "speak" ? engine : "local",
  }));
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("EngineSelect", () => {
  it("shows Local for both steps by default", async () => {
    render(<EngineSelect services={[gemini]} keys={[]} />);
    await waitFor(() => expect(ipc.cloudEnginesGet).toHaveBeenCalled());
    expect(screen.getByLabelText("Translation engine").textContent).toContain("Local");
    expect(screen.getByLabelText("Speech engine").textContent).toContain("Local");
  });

  it("asks for a verified key first when a service has none", async () => {
    render(<EngineSelect services={[gemini]} keys={[key("untested")]} />);
    await userEvent.click(screen.getByLabelText("Translation engine"));
    await userEvent.click(await screen.findByRole("option", { name: "Google Gemini API" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Add and verify a key");
    expect(ipc.cloudEnginesSet).not.toHaveBeenCalled();
  });

  it("switches a step to a service that has a verified key", async () => {
    render(<EngineSelect services={[gemini]} keys={[key("verified")]} />);
    await userEvent.click(screen.getByLabelText("Translation engine"));
    await userEvent.click(await screen.findByRole("option", { name: "Google Gemini API" }));
    await waitFor(() => expect(ipc.cloudEnginesSet).toHaveBeenCalledWith("translate", "google-gemini"));
    await waitFor(() =>
      expect(screen.getByLabelText("Translation engine").textContent).toContain("Google Gemini API"),
    );
    expect(screen.getByLabelText("Speech engine").textContent).toContain("Local");
  });
});

// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const gemini = {
  id: "google-gemini",
  name: "Google Gemini API",
  steps: ["translate", "speak"],
  needsRegion: false,
  termsUrl: "https://example.com/terms",
  languages: { translate: ["fr"], speak: ["fr"] },
};
const azure = { ...gemini, id: "azure-ai", name: "Azure AI services", needsRegion: true };

const saved = {
  id: "k1",
  serviceId: "google-gemini",
  label: "personal",
  region: null,
  last4: "1234",
  order: 0,
  state: "untested",
  verifiedAt: null,
  storage: "os",
};

const ipc = vi.hoisted(() => ({
  cloudServices: vi.fn(),
  cloudKeysList: vi.fn(),
  cloudKeyAdd: vi.fn(),
  cloudKeyVerify: vi.fn(),
  cloudKeyRemove: vi.fn(),
  cloudKeysReorder: vi.fn(),
  cloudEnginesGet: vi.fn(),
  cloudEnginesSet: vi.fn(),
  cloudConsentList: vi.fn(),
  cloudConsentWithdraw: vi.fn(),
}));
vi.mock("../../app/ipc", () => ipc);

import { CloudCard } from "./CloudCard";

beforeEach(() => {
  ipc.cloudServices.mockResolvedValue([gemini]);
  ipc.cloudKeysList.mockResolvedValue([saved]);
  ipc.cloudKeyVerify.mockResolvedValue({ ok: true, languages: { translate: [], speak: [] } });
  ipc.cloudKeyRemove.mockResolvedValue(undefined);
  ipc.cloudKeyAdd.mockResolvedValue(saved);
  ipc.cloudEnginesGet.mockResolvedValue({ translate: "local", speak: "local" });
  ipc.cloudConsentList.mockResolvedValue([]);
  ipc.cloudConsentWithdraw.mockResolvedValue(undefined);
  ipc.cloudKeysReorder.mockResolvedValue([]);
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("CloudCard", () => {
  it("lists a key with its label, service and masked value only", async () => {
    render(<CloudCard />);
    expect(await screen.findByText("personal")).toBeTruthy();
    expect(screen.getByText("Google Gemini API")).toBeTruthy();
    expect(screen.getByText("Key ending in 1234")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/AIza/);
  });

  it("says the key works and names the service", async () => {
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Verify connection" }));
    expect(await screen.findByText("The key works with Google Gemini API.")).toBeTruthy();
    expect(ipc.cloudKeyVerify).toHaveBeenCalledWith("k1");
  });

  it.each([
    ["rejected", "refused this key"],
    ["noCredit", "no credit or quota"],
    ["offline", "Could not reach"],
    ["unreachable", "not answering"],
    ["unexpected", "did not expect"],
  ])("explains a %s failure in plain words", async (reason, text) => {
    ipc.cloudKeyVerify.mockResolvedValue({ ok: false, reason, message: "x" });
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Verify connection" }));
    const alert = await screen.findByRole("status");
    expect(alert.textContent).toContain(text);
  });

  it("asks before removing a key", async () => {
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(ipc.cloudKeyRemove).not.toHaveBeenCalled();
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Remove this key?")).toBeTruthy();
    await userEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(ipc.cloudKeyRemove).toHaveBeenCalledWith("k1"));
  });

  it("adds a key and clears the secret from the screen", async () => {
    ipc.cloudKeysList.mockResolvedValue([]);
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Add key" }));
    await userEvent.type(screen.getByLabelText("Name for this key"), "personal");
    await userEvent.type(screen.getByLabelText("Key"), "AIzaSECRET1234");
    await userEvent.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() =>
      expect(ipc.cloudKeyAdd).toHaveBeenCalledWith(
        expect.objectContaining({ serviceId: "google-gemini", label: "personal", secret: "AIzaSECRET1234" }),
      ),
    );
    await waitFor(() => expect(screen.queryByLabelText("Key")).toBeNull());
  });

  it("asks for a region when the service needs one", async () => {
    ipc.cloudServices.mockResolvedValue([azure]);
    ipc.cloudKeysList.mockResolvedValue([]);
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Add key" }));
    expect(screen.getByLabelText(/Region/)).toBeTruthy();
  });

  it("offers to use the key for this session only when secure storage fails", async () => {
    ipc.cloudKeysList.mockResolvedValue([]);
    ipc.cloudKeyAdd.mockRejectedValueOnce({ code: "KEY_STORE_UNAVAILABLE" });
    render(<CloudCard />);
    await userEvent.click(await screen.findByRole("button", { name: "Add key" }));
    await userEvent.type(screen.getByLabelText("Name for this key"), "temp");
    await userEvent.type(screen.getByLabelText("Key"), "AIzaSECRET9999");
    await userEvent.click(screen.getByRole("button", { name: "Save key" }));
    expect((await screen.findByRole("alert")).textContent).toContain("secure storage");
    await userEvent.click(screen.getByRole("button", { name: "Use for this session only" }));
    await waitFor(() =>
      expect(ipc.cloudKeyAdd).toHaveBeenLastCalledWith(expect.objectContaining({ sessionOnly: true })),
    );
  });

  it("puts keys of one service in the order the user chooses and sends that order", async () => {
    const second = { ...saved, id: "k2", label: "spare", last4: "9999", order: 1 };
    ipc.cloudKeysList.mockResolvedValue([saved, second]);
    render(<CloudCard />);
    await screen.findByText("spare");
    const down = screen.getAllByRole("button", { name: "Move down" });
    expect(down[1].hasAttribute("disabled")).toBe(true); // the last key cannot move down
    await userEvent.click(down[0]);
    await waitFor(() => expect(ipc.cloudKeysReorder).toHaveBeenCalledWith("google-gemini", ["k2", "k1"]));
  });

  it("lists what the user agreed to and takes the agreement back", async () => {
    ipc.cloudConsentList.mockResolvedValue([
      { serviceId: "google-gemini", step: "translate", agreedAt: "x", agreedChars: 12_340, noticeVersion: 1 },
    ]);
    render(<CloudCard />);
    expect(await screen.findByText(/Google Gemini API · Translation · agreed for about 12300 characters/)).toBeTruthy();
    ipc.cloudConsentList.mockResolvedValue([]);
    await userEvent.click(screen.getByRole("button", { name: "Withdraw" }));
    await waitFor(() => expect(ipc.cloudConsentWithdraw).toHaveBeenCalledWith("google-gemini", "translate"));
    await waitFor(() => expect(screen.getByText("You have not agreed to send anything yet.")).toBeTruthy());
  });
});

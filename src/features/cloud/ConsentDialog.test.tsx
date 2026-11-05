// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const ipc = vi.hoisted(() => ({ cloudNotice: vi.fn(), cloudConsentGrant: vi.fn() }));
vi.mock("../../app/ipc", () => ipc);

import { ConsentDialog } from "./ConsentDialog";
import type { CloudPreflight } from "./types";

const item: CloudPreflight = {
  step: "translate",
  serviceId: "google-gemini",
  serviceName: "Google Gemini API",
  estimatedChars: 12_340,
  needsConsent: true,
  reason: "first",
  hasKey: true,
};

beforeEach(() => {
  ipc.cloudNotice.mockResolvedValue({
    serviceName: "Google Gemini API",
    step: "translate",
    sends: ["cloud.notice.translate"],
    doesNotSend: ["cloud.notice.doesNotSend", "cloud.notice.bill"],
    termsUrl: "https://example.com/terms",
    estimatedChars: 12_340,
    noticeVersion: 1,
  });
  ipc.cloudConsentGrant.mockResolvedValue({});
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ConsentDialog", () => {
  it("names the service and step, what is sent and what is not, and links the terms", async () => {
    render(<ConsentDialog item={item} onAgree={() => {}} onDecline={() => {}} />);
    expect(await screen.findByText("Send text to Google Gemini API?")).toBeTruthy();
    expect(screen.getByText("The text of the lines to translate.")).toBeTruthy();
    expect(screen.getByTestId("consent-amount").textContent).toBe("About 12300 characters");
    expect(screen.getByText(/Not sent: your video, your original audio/)).toBeTruthy();
    expect(screen.getByText(/bills your own key/)).toBeTruthy();
    const link = screen.getByRole("link", { name: "Terms of Google Gemini API" });
    expect(link.getAttribute("href")).toBe("https://example.com/terms");
    expect(ipc.cloudNotice).toHaveBeenCalledWith("google-gemini", "translate", 12_340);
  });

  it("records the agreement with the amount shown, then tells the caller", async () => {
    const onAgree = vi.fn();
    render(<ConsentDialog item={item} onAgree={onAgree} onDecline={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: "Agree" }));
    await waitFor(() => expect(onAgree).toHaveBeenCalled());
    expect(ipc.cloudConsentGrant).toHaveBeenCalledWith("google-gemini", "translate", 12_340);
  });

  it("sends nothing and records nothing when the user says not now", async () => {
    const onDecline = vi.fn();
    render(<ConsentDialog item={item} onAgree={() => {}} onDecline={onDecline} />);
    await userEvent.click(await screen.findByRole("button", { name: "Not now" }));
    expect(onDecline).toHaveBeenCalled();
    expect(ipc.cloudConsentGrant).not.toHaveBeenCalled();
  });

  it("explains why it asks again for a much larger run", async () => {
    render(
      <ConsentDialog item={{ ...item, reason: "largerRun" }} onAgree={() => {}} onDecline={() => {}} />,
    );
    expect(await screen.findByText(/much larger than the one you agreed to/)).toBeTruthy();
  });

  it("shows nothing when no question is open", () => {
    render(<ConsentDialog item={null} onAgree={() => {}} onDecline={() => {}} />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

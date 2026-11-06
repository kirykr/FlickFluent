// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DiscardEditsDialog } from "./DiscardEditsDialog";

afterEach(cleanup);

describe("DiscardEditsDialog", () => {
  it("says how many edited lines a new dub replaces and waits for a choice", async () => {
    const onKeep = vi.fn();
    const onReplace = vi.fn();
    render(<DiscardEditsDialog count={3} open onKeep={onKeep} onReplace={onReplace} />);
    expect(screen.getByText(/replace 3 lines you edited/i)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Keep my edits" }));
    expect(onKeep).toHaveBeenCalled();
    expect(onReplace).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Start a new dub" }));
    expect(onReplace).toHaveBeenCalled();
  });

  it("stays closed when nothing was edited", () => {
    render(<DiscardEditsDialog count={0} open onKeep={() => {}} onReplace={() => {}} />);
    expect(screen.queryByTestId("discard-edits")).toBeNull();
  });
});

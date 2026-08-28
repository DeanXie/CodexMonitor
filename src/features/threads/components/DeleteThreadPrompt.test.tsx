// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DeleteThreadPrompt } from "./DeleteThreadPrompt";

afterEach(cleanup);

describe("DeleteThreadPrompt", () => {
  it("names the conversation and warns that spawned descendants are cascaded", () => {
    render(
      <DeleteThreadPrompt
        title="Scenario A"
        blocked={false}
        busy={false}
        error={null}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText(/Scenario A/)).toBeTruthy();
    expect(screen.getByText(/spawned Sub-Agent descendants/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete permanently" }).className).toContain("danger");
  });

  it("disables confirmation while the thread tree is active", () => {
    const onConfirm = vi.fn();
    render(
      <DeleteThreadPrompt
        title="Running task"
        blocked
        busy={false}
        error={null}
        onCancel={vi.fn()}
        onConfirm={onConfirm}
      />,
    );
    const button = screen.getByRole("button", { name: "Delete permanently" }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ComposerMetaBar } from "./ComposerMetaBar";

const CURRENT_ACCESS_DESCRIPTION =
  "Allows writes within the active workspace and enables network access. Approval remains on-request.";

afterEach(cleanup);

describe("ComposerMetaBar access mode", () => {
  it("describes the current value as the fixed workspace network preset", () => {
    render(
      <ComposerMetaBar
        disabled={false}
        collaborationModes={[]}
        selectedCollaborationModeId={null}
        onSelectCollaborationMode={vi.fn()}
        models={[]}
        selectedModelId={null}
        onSelectModel={vi.fn()}
        reasoningOptions={[]}
        selectedEffort={null}
        onSelectEffort={vi.fn()}
        selectedServiceTier={null}
        reasoningSupported={false}
        accessMode="current"
        onSelectAccessMode={vi.fn()}
      />,
    );

    const select = screen.getByLabelText("Agent access") as HTMLSelectElement;
    const options = within(select).getAllByRole("option");

    expect(select.value).toBe("current");
    expect(select.title).toBe(CURRENT_ACCESS_DESCRIPTION);
    expect(options.map((option) => [option.textContent, option.getAttribute("value")])).toEqual([
      ["Read only", "read-only"],
      ["Workspace access (network enabled)", "current"],
      ["Full access", "full-access"],
    ]);
  });
});

// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppSettings } from "@/types";
import { SettingsCodexSection } from "./SettingsCodexSection";

afterEach(cleanup);

describe("SettingsCodexSection access mode", () => {
  it("describes the current value as the fixed workspace network preset", () => {
    render(
      <SettingsCodexSection
        appSettings={
          {
            defaultAccessMode: "current",
            lastComposerModelId: null,
            lastComposerReasoningEffort: null,
            reviewDeliveryMode: "inline",
          } as AppSettings
        }
        onUpdateAppSettings={vi.fn().mockResolvedValue(undefined)}
        defaultModels={[]}
        defaultModelsLoading={false}
        defaultModelsError={null}
        defaultModelsConnectedWorkspaceCount={0}
        onRefreshDefaultModels={vi.fn()}
        codexPathDraft=""
        codexArgsDraft=""
        codexDirty={false}
        isSavingSettings={false}
        doctorState={{ status: "idle", result: null }}
        codexUpdateState={{ status: "idle", result: null }}
        globalAgentsMeta=""
        globalAgentsError={null}
        globalAgentsContent=""
        globalAgentsLoading={false}
        globalAgentsRefreshDisabled={false}
        globalAgentsSaveDisabled={false}
        globalAgentsSaveLabel="Save"
        globalConfigMeta=""
        globalConfigError={null}
        globalConfigContent=""
        globalConfigLoading={false}
        globalConfigRefreshDisabled={false}
        globalConfigSaveDisabled={false}
        globalConfigSaveLabel="Save"
        onSetCodexPathDraft={vi.fn()}
        onSetCodexArgsDraft={vi.fn()}
        onSetGlobalAgentsContent={vi.fn()}
        onSetGlobalConfigContent={vi.fn()}
        onBrowseCodex={vi.fn().mockResolvedValue(undefined)}
        onSaveCodexSettings={vi.fn().mockResolvedValue(undefined)}
        onRunDoctor={vi.fn().mockResolvedValue(undefined)}
        onRunCodexUpdate={vi.fn().mockResolvedValue(undefined)}
        onRefreshGlobalAgents={vi.fn()}
        onSaveGlobalAgents={vi.fn()}
        onRefreshGlobalConfig={vi.fn()}
        onSaveGlobalConfig={vi.fn()}
      />,
    );

    const select = screen.getByLabelText("Access mode") as HTMLSelectElement;
    const options = within(select).getAllByRole("option");

    expect(select.value).toBe("current");
    expect(select.title).toBe(
      "Allows writes within the active workspace and enables network access. Approval remains on-request.",
    );
    expect(options.map((option) => [option.textContent, option.getAttribute("value")])).toEqual([
      ["Read only", "read-only"],
      ["Workspace access (network enabled)", "current"],
      ["Full access", "full-access"],
    ]);
  });
});

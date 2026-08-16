// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { AgentMonitorPage } from "./AgentMonitorPage";

afterEach(cleanup);

describe("AgentMonitorPage", () => {
  it("renders the agent hierarchy and existing model usage proportions", () => {
    render(
      <AgentMonitorPage
        threadsByWorkspace={{
          workspace: [
            { id: "main", name: "Main Agent", updatedAt: 1, modelId: "gpt-5.4" },
            {
              id: "child",
              name: "Research Agent",
              updatedAt: 2,
              modelId: "gpt-5.3-mini",
              isSubagent: true,
              subagentRole: "explorer",
            },
          ],
        }}
        threadParentById={{ child: "main" }}
        threadStatusById={{
          main: { isProcessing: true, isReviewing: false, processingStartedAt: 1_000 },
        }}
        tokenUsageByThread={{}}
        localUsageSnapshot={{
          updatedAt: 1,
          days: [],
          totals: {
            last7DaysTokens: 0,
            last30DaysTokens: 0,
            averageDailyTokens: 0,
            cacheHitRatePercent: 0,
            peakDay: null,
            peakDayTokens: 0,
          },
          topModels: [{ model: "gpt-5.4", tokens: 400, sharePercent: 80 }],
        }}
        now={6_000}
      />,
    );

    expect(screen.getByRole("heading", { name: "Agent Monitor" })).toBeTruthy();
    expect(screen.getAllByText("Main Agent").length).toBeGreaterThan(0);
    expect(screen.getByText("Research Agent")).toBeTruthy();
    expect(screen.getAllByText("gpt-5.4").length).toBeGreaterThan(0);
    expect(screen.getByText("Running")).toBeTruthy();
    expect(screen.getByText("80%")).toBeTruthy();
  });

  it("filters the call tree to the selected workspace and session", () => {
    render(
      <AgentMonitorPage
        threadsByWorkspace={{
          codex: [
            { id: "main", name: "Fix build", updatedAt: 1, createdAt: 1, modelId: "gpt-5.6-terra" },
            { id: "child", name: "Investigate", updatedAt: 2, modelId: "gpt-5.6-sol", isSubagent: true },
          ],
          other: [{ id: "other", name: "Other project", updatedAt: 3, modelId: "gpt-5.6-luna" }],
        }}
        workspaceOptions={[
          { id: "codex", label: "CodexMonitor" },
          { id: "other", label: "Other" },
        ]}
        threadParentById={{ child: "main" }}
        threadStatusById={{}}
        tokenUsageByThread={{}}
        localUsageSnapshot={null}
        now={6_000}
      />,
    );

    expect(screen.getByLabelText("Workspace")).toBeTruthy();
    expect(screen.getByLabelText("Session")).toBeTruthy();
    expect(screen.getAllByText("Fix build").length).toBeGreaterThan(1);
    fireEvent.change(screen.getByLabelText("Workspace"), { target: { value: "codex" } });
    fireEvent.change(screen.getByLabelText("Session"), { target: { value: "main" } });
    expect(screen.getByText(/Created:/)).toBeTruthy();
    expect(screen.queryByText("Other project")).toBeNull();
  });
});

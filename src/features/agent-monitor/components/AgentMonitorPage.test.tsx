// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AgentMonitorPage } from "./AgentMonitorPage";

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
});

import { describe, expect, it } from "vitest";

import { buildAgentMonitorSummary } from "./agentMonitorMetrics";

describe("buildAgentMonitorSummary", () => {
  it("aggregates all visible agents and picks the highest-token model", () => {
    const summary = buildAgentMonitorSummary([
      {
        threadId: "main",
        name: "Main",
        modelId: "gpt-5.4",
        effort: null,
        role: null,
        isSubagent: false,
        status: "running",
        runtimeMs: 1_000,
        totalTokens: 100,
        children: [
          {
            threadId: "child",
            name: "Child",
            modelId: "gpt-5.3-mini",
            effort: null,
            role: "explorer",
            isSubagent: true,
            status: "reviewing",
            runtimeMs: 2_000,
            totalTokens: 150,
            children: [],
          },
        ],
      },
    ]);

    expect(summary).toEqual({
      totalAgents: 2,
      activeAgents: 2,
      totalTokens: 250,
      primaryModel: "gpt-5.3-mini",
    });
  });
});

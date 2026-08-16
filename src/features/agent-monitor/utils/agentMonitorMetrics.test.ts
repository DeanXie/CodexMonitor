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
        tokenUsage: { totalTokens: 100, inputTokens: 70, cachedInputTokens: 20, outputTokens: 30, reasoningOutputTokens: 10 },
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
            tokenUsage: { totalTokens: 150, inputTokens: 100, cachedInputTokens: 30, outputTokens: 50, reasoningOutputTokens: 20 },
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

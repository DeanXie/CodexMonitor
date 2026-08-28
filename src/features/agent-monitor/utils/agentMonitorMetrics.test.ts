import { describe, expect, it } from "vitest";

import { buildAgentMonitorSummary } from "./agentMonitorMetrics";

describe("buildAgentMonitorSummary", () => {
  it("counts agents without merging independent parent and child token totals", () => {
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
        source: { sourceKind: "monitor-app-server", temporalClass: "LIVE", freshnessState: "fresh", ageMs: 0, sourceTimestampMs: null, observedTimestampMs: null },
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
            source: { sourceKind: "monitor-app-server", temporalClass: "LIVE", freshnessState: "fresh", ageMs: 0, sourceTimestampMs: null, observedTimestampMs: null },
            children: [],
          },
        ],
      },
    ], "active-fresh");

    expect(summary).toEqual({
      totalAgents: 2,
      activityMetric: {
        label: "Active",
        value: 2,
        tooltip: null,
      },
      totalTokens: 100,
      primaryModel: null,
    });
  });

  it("excludes stale recorded lifecycle from Active Fresh but keeps reviewing when fresh", () => {
    const base = {
      name: "Agent",
      modelId: null,
      effort: null,
      role: null,
      isSubagent: false,
      runtimeMs: 1_000,
      totalTokens: null,
      tokenUsage: null,
      children: [],
    };
    const summary = buildAgentMonitorSummary([
      {
        ...base,
        threadId: "stale-running",
        status: "running",
        source: { sourceKind: "codex-cli-rollout", temporalClass: "NEAR_LIVE", freshnessState: "stale", ageMs: 90_000, sourceTimestampMs: 1, observedTimestampMs: 2 },
      },
      {
        ...base,
        threadId: "fresh-reviewing",
        status: "reviewing",
        source: { sourceKind: "monitor-app-server", temporalClass: "LIVE", freshnessState: "fresh", ageMs: 10, sourceTimestampMs: 3, observedTimestampMs: 4 },
      },
    ], "active-fresh");

    expect(summary.activityMetric).toEqual({
      label: "Active",
      value: 1,
      tooltip: null,
    });
  });

  it("labels all recorded active lifecycle and explains stale unresolved agents", () => {
    const summary = buildAgentMonitorSummary([{
      threadId: "stale-waiting",
      name: "Agent",
      modelId: null,
      effort: null,
      role: null,
      isSubagent: false,
      status: "waiting",
      runtimeMs: 1_000,
      totalTokens: null,
      tokenUsage: null,
      source: { sourceKind: "codex-cli-rollout", temporalClass: "NEAR_LIVE", freshnessState: "stale", ageMs: 90_000, sourceTimestampMs: 1, observedTimestampMs: 2 },
      children: [],
    }], "all");

    expect(summary.activityMetric).toEqual({
      label: "Recorded Active",
      value: 1,
      tooltip: "Includes stale unresolved agents whose last recorded lifecycle was Running or Waiting.",
    });
  });

  it("omits the activity metric in Settled mode", () => {
    const summary = buildAgentMonitorSummary([], "settled");

    expect(summary.activityMetric).toBeNull();
  });

  it("keeps root direct usage unavailable when more than one root is visible", () => {
    const root = {
      threadId: "root-a",
      name: "Main",
      modelId: null,
      effort: null,
      role: null,
      isSubagent: false,
      status: "completed" as const,
      runtimeMs: 1_000,
      totalTokens: 100,
      tokenUsage: null,
      source: { sourceKind: "codex-cli-rollout" as const, temporalClass: "NEAR_LIVE" as const, freshnessState: "settled" as const, ageMs: 5_000, sourceTimestampMs: 1, observedTimestampMs: 2 },
      children: [],
    };

    expect(buildAgentMonitorSummary([root, { ...root, threadId: "root-b", totalTokens: 200 }], "all").totalTokens)
      .toBeNull();
  });
});

import { describe, expect, it } from "vitest";

import { buildAgentMonitorForest } from "./agentMonitorTree";

describe("buildAgentMonitorForest", () => {
  it("builds a main-agent tree with current model, status, runtime, and tokens", () => {
    const forest = buildAgentMonitorForest({
      threads: [
        { id: "main", name: "Main Agent", updatedAt: 10, modelId: "gpt-5.4" },
        {
          id: "child",
          name: "Researcher",
          updatedAt: 20,
          modelId: "gpt-5.3-mini",
          isSubagent: true,
          subagentRole: "explorer",
        },
      ],
      threadParentById: { child: "main" },
      threadStatusById: {
        main: { isProcessing: true, isReviewing: false, processingStartedAt: 1_000 },
        child: { isProcessing: false, isReviewing: true, lastDurationMs: 4_000 },
      },
      tokenUsageByThread: {
        main: {
          total: {
            totalTokens: 120,
            inputTokens: 80,
            cachedInputTokens: 20,
            outputTokens: 40,
            reasoningOutputTokens: 10,
          },
          last: {
            totalTokens: 20,
            inputTokens: 10,
            cachedInputTokens: 0,
            outputTokens: 10,
            reasoningOutputTokens: 2,
          },
          modelContextWindow: 128_000,
        },
      },
      now: 6_000,
    });

    expect(forest).toHaveLength(1);
    expect(forest[0]).toMatchObject({
      threadId: "main",
      modelId: "gpt-5.4",
      status: "running",
      runtimeMs: 5_000,
      totalTokens: 120,
      children: [
        {
          threadId: "child",
          modelId: "gpt-5.3-mini",
          status: "reviewing",
          runtimeMs: 4_000,
          role: "explorer",
        },
      ],
    });
  });

  it("keeps unlinked subagents visible and rejects cyclic parent links", () => {
    const forest = buildAgentMonitorForest({
      threads: [
        { id: "main", name: "Main", updatedAt: 10 },
        { id: "orphan", name: "Orphan", updatedAt: 20, isSubagent: true },
        { id: "cycle-a", name: "Cycle A", updatedAt: 30 },
        { id: "cycle-b", name: "Cycle B", updatedAt: 40, isSubagent: true },
      ],
      threadParentById: { orphan: "missing", "cycle-a": "cycle-b", "cycle-b": "cycle-a" },
      threadStatusById: {},
      tokenUsageByThread: {},
      now: 1_000,
    });

    expect(forest.map((node) => node.threadId)).toEqual([
      "main",
      "orphan",
      "cycle-a",
      "cycle-b",
    ]);
    expect(forest.every((node) => node.children.length === 0)).toBe(true);
  });
});

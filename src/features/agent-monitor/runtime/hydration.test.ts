import { describe, expect, it } from "vitest";

import { selectAgentMonitorRuntimeView } from "../utils/agentRuntimeSelector";
import { buildAgentRuntimeHydrationRecords } from "./hydration";
import { applyRuntimeRecords, createRuntimeState } from "./runtimeState";

describe("agent runtime hydration", () => {
  it("catches up an already-running Thread from current app state with explicit provenance", () => {
    const records = buildAgentRuntimeHydrationRecords({
      threadsByWorkspace: {
        workspace: [{
          id: "thread-running-before-monitor",
          name: "Existing running task",
          createdAt: 1_700_000_000_000,
          updatedAt: 1_700_000_002_000,
          modelId: "historical-model-must-not-hydrate",
        }],
      },
      threadParentById: {},
      threadStatusById: {
        "thread-running-before-monitor": {
          isProcessing: true,
          processingStartedAt: 1_700_000_001_000,
        },
      },
      activeTurnIdByThread: {
        "thread-running-before-monitor": "turn-running-before-monitor",
      },
      capturedAtMs: 1_700_000_003_000,
    });
    const state = applyRuntimeRecords(createRuntimeState(), records, 1_700_000_003_000);
    const view = selectAgentMonitorRuntimeView(state, 1_700_000_004_000);

    expect(view.threads).toHaveLength(1);
    expect(view.roots[0]).toMatchObject({
      threadId: "thread-running-before-monitor",
      status: "running",
      runtimeMs: 3_000,
      modelId: null,
      tokenUsage: null,
      totalTokens: null,
    });
    expect(state.threads["thread-running-before-monitor"].status?.provenance).toMatchObject({
      recordSource: "HYDRATION",
      method: "app/runtime hydration",
      serverTimeMs: null,
    });
    expect(state.turns["turn-running-before-monitor"].startedAt?.provenance)
      .toMatchObject({ recordSource: "HYDRATION", serverTimeMs: null });
  });

  it("hydrates parent identity for a previously known subagent", () => {
    const records = buildAgentRuntimeHydrationRecords({
      threadsByWorkspace: {
        workspace: [
          { id: "main", name: "Main", updatedAt: 1 },
          { id: "child", name: "Child", updatedAt: 2, isSubagent: true },
        ],
      },
      threadParentById: { child: "main" },
      threadStatusById: { child: { isProcessing: true, processingStartedAt: 9_000 } },
      activeTurnIdByThread: { child: "child-turn" },
      capturedAtMs: 10_000,
    });
    const state = applyRuntimeRecords(createRuntimeState(), records, 10_000);

    expect(state.threads.child.parentThreadId).toBe("main");
    expect(state.threads.main.childThreadIds).toEqual(["child"]);
  });

  it("does not turn idle historical thread summaries into Live sessions", () => {
    const records = buildAgentRuntimeHydrationRecords({
      threadsByWorkspace: {
        workspace: [
          { id: "historical-idle", name: "Historical", updatedAt: 1, modelId: "old-model" },
          { id: "currently-running", name: "Current", updatedAt: 2 },
        ],
      },
      threadParentById: {},
      threadStatusById: {
        "historical-idle": { isProcessing: false },
        "currently-running": { isProcessing: true, processingStartedAt: 1_000 },
      },
      activeTurnIdByThread: { "currently-running": "current-turn" },
      capturedAtMs: 2_000,
    });

    expect(records.map((record) => record.payload.threadId)).toEqual(["currently-running"]);
  });
});

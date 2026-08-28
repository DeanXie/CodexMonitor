import { describe, expect, it } from "vitest";
import phase24C from "../../../../docs/evidence/phase-2-4-c.json";

import {
  applyRuntimeRecords,
  createRuntimeState,
  type RuntimeProtocolRecord,
} from "../runtime";
import type {
  GlobalSourceProvenance,
  GlobalSourceSnapshot,
  GlobalSourceThread,
} from "../global-source/types";
import { selectUnifiedAgentMonitorView } from "./globalSourceSelector";

const nearLive: GlobalSourceProvenance = {
  sourceKind: "codex-cli-rollout",
  temporalClass: "NEAR_LIVE",
  sourceInstanceId: "rollout-tail:home-1",
  sourceGeneration: "file:1",
  sourceTimestampMs: 9_000,
  observedTimestampMs: 9_400,
  freshness: {
    state: "fresh",
    lastCompleteRecordObservedAtMs: 9_400,
    reason: "complete rollout record observed",
  },
};

const freshLive: GlobalSourceProvenance = {
  ...nearLive,
  sourceKind: "monitor-app-server",
  temporalClass: "LIVE",
  sourceInstanceId: "app-server:workspace",
};

const staleNearLive: GlobalSourceProvenance = {
  ...nearLive,
  observedTimestampMs: 8_000,
  freshness: {
    state: "stale",
    lastCompleteRecordObservedAtMs: 8_000,
    reason: "no recent complete record",
  },
};

const settledNearLive: GlobalSourceProvenance = {
  ...nearLive,
  observedTimestampMs: 7_000,
  freshness: {
    state: "settled",
    lastCompleteRecordObservedAtMs: 7_000,
    reason: "source settled",
  },
};

function sourceThread(
  threadId: string,
  overrides: Partial<GlobalSourceThread> = {},
): GlobalSourceThread {
  return {
    key: { codexHomeIdentity: "home-1", threadId },
    parentThreadKey: null,
    agentPath: null,
    currentTurn: {
      key: {
        threadKey: { codexHomeIdentity: "home-1", threadId },
        turnId: `turn-${threadId}`,
      },
      lifecycle: { value: "running", provenance: nearLive },
      startedAt: { ...nearLive, sourceTimestampMs: 8_000 },
      completedAt: null,
    },
    lifecycle: { value: "running", provenance: nearLive },
    observedModel: { value: "gpt-cli", provenance: nearLive },
    tokenSnapshot: {
      value: {
        inputTokens: 120,
        cachedInputTokens: 80,
        cacheWriteInputTokens: 0,
        outputTokens: 5,
        reasoningOutputTokens: 2,
        totalTokens: 125,
      },
      provenance: nearLive,
    },
    authorityProvenance: nearLive,
    liveLaneCount: 0,
    nearLiveLaneCount: 1,
    historicalLaneCount: 0,
    ...overrides,
  };
}

function snapshot(threads: GlobalSourceThread[]): GlobalSourceSnapshot {
  return {
    revision: 1,
    generatedAtMs: 9_500,
    workspaceCodexHomeIdentities: { workspace: "home-1" },
    threads,
  };
}

function liveRuntime(threadId = "paired") {
  const records: RuntimeProtocolRecord[] = [{
    source: "EVENT",
    capturedAt: "2026-08-26T00:00:00Z",
    label: "thread/started",
    payload: {
      workspace_id: "workspace",
      message: {
        method: "thread/started",
        params: { thread: { id: threadId, status: { type: "active" } } },
      },
    },
  }, {
    source: "EVENT",
    capturedAt: "2026-08-26T00:00:01Z",
    label: "thread/tokenUsage/updated",
    payload: {
      workspace_id: "workspace",
      message: {
        method: "thread/tokenUsage/updated",
        params: {
          threadId,
          turnId: `turn-${threadId}`,
          tokenUsage: {
            last: { inputTokens: 190, cachedInputTokens: 100, outputTokens: 10, totalTokens: 200 },
            total: { inputTokens: 190, cachedInputTokens: 100, outputTokens: 10, totalTokens: 200 },
          },
        },
      },
    },
  }];
  return applyRuntimeRecords(createRuntimeState(), records, 9_000);
}

describe("selectUnifiedAgentMonitorView", () => {
  it("projects external CLI main and confirmed sub-agent as NEAR LIVE", () => {
    const main = sourceThread("cli-main");
    const child = sourceThread("cli-child", {
      parentThreadKey: {
        value: main.key,
        provenance: nearLive,
      },
      agentPath: { value: "/root/reader", provenance: nearLive },
    });

    const view = selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([main, child]),
      10_000,
    );

    expect(view.threads).toHaveLength(2);
    expect(view.roots[0]).toMatchObject({
      threadId: "cli-main",
      source: {
        temporalClass: "NEAR_LIVE",
        freshnessState: "fresh",
        ageMs: 1_000,
        observedAgeMs: 600,
      },
      modelId: "gpt-cli",
      status: "running",
      runtimeMs: 2_000,
      totalTokens: 125,
    });
    expect(view.roots[0]?.children[0]).toMatchObject({
      threadId: "cli-child",
      name: "reader",
      role: "/root/reader",
    });
  });

  it("deduplicates a paired Thread by canonical key and keeps Phase 1 LIVE tokens", () => {
    const rolloutPair = sourceThread("paired", {
      liveLaneCount: 1,
      authorityProvenance: freshLive,
      tokenSnapshot: {
        value: {
          inputTokens: 290,
          cachedInputTokens: 200,
          cacheWriteInputTokens: 0,
          outputTokens: 10,
          reasoningOutputTokens: 0,
          totalTokens: 300,
        },
        provenance: freshLive,
      },
    });

    const view = selectUnifiedAgentMonitorView(
      liveRuntime(),
      snapshot([rolloutPair]),
      10_000,
    );

    expect(view.threads).toHaveLength(1);
    expect(view.threads[0]).toMatchObject({
      threadId: "paired",
      totalTokens: 200,
      source: { temporalClass: "LIVE" },
    });
  });

  it("uses the canonical rollout fallback without duplicating or regressing a stale LIVE Thread", () => {
    const fallback = sourceThread("paired", {
      liveLaneCount: 1,
      nearLiveLaneCount: 1,
      authorityProvenance: nearLive,
      tokenSnapshot: {
        value: {
          inputTokens: 290,
          cachedInputTokens: 200,
          cacheWriteInputTokens: 0,
          outputTokens: 10,
          reasoningOutputTokens: 0,
          totalTokens: 300,
        },
        provenance: nearLive,
      },
    });

    const view = selectUnifiedAgentMonitorView(
      liveRuntime(),
      snapshot([fallback]),
      10_000,
    );

    expect(view.threads).toHaveLength(1);
    expect(view.threads[0]).toMatchObject({
      threadId: "paired",
      workspaceId: "workspace",
      isCurrentEligible: true,
      totalTokens: 300,
      source: { temporalClass: "NEAR_LIVE" },
    });
  });

  it("retains a confirmed rollout model when fresh LIVE remains authoritative after completion", () => {
    const threadId = phase24C.fullThreadId;
    const observedModel = phase24C.observedModels[0].model;
    const completedPair = sourceThread(threadId, {
      lifecycle: { value: "completed", provenance: freshLive },
      observedModel: { value: observedModel, provenance: nearLive },
      authorityProvenance: freshLive,
      liveLaneCount: 1,
      nearLiveLaneCount: 1,
    });

    const view = selectUnifiedAgentMonitorView(
      liveRuntime(threadId),
      snapshot([completedPair]),
      10_000,
    );

    expect(view.threads).toHaveLength(1);
    expect(view.threads[0]).toMatchObject({
      threadId,
      modelId: observedModel,
      source: { temporalClass: "LIVE" },
      modelSource: {
        sourceKind: "codex-cli-rollout",
        temporalClass: "NEAR_LIVE",
      },
    });
  });

  it("filters whole sessions by activity and sorts running, waiting, fresh, stale, then settled", () => {
    const runningStale = sourceThread("running-stale", {
      lifecycle: { value: "running", provenance: staleNearLive },
      authorityProvenance: staleNearLive,
    });
    const waitingFresh = sourceThread("waiting-fresh", {
      lifecycle: { value: "waiting", provenance: nearLive },
    });
    const completedFresh = sourceThread("completed-fresh", {
      lifecycle: { value: "completed", provenance: nearLive },
    });
    const completedSettled = sourceThread("completed-settled", {
      lifecycle: { value: "completed", provenance: settledNearLive },
      authorityProvenance: settledNearLive,
    });
    const sources = snapshot([
      completedSettled,
      completedFresh,
      waitingFresh,
      runningStale,
    ]);

    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000, { activityFilter: "active-fresh" },
    ).roots.map((thread) => thread.threadId)).toEqual([
      "waiting-fresh",
      "completed-fresh",
    ]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000, { activityFilter: "all" },
    ).roots.map((thread) => thread.threadId)).toEqual([
      "running-stale",
      "waiting-fresh",
      "completed-fresh",
      "completed-settled",
    ]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000, { activityFilter: "settled" },
    ).roots.map((thread) => thread.threadId)).toEqual(["completed-settled"]);
  });

  it("keeps distinct full Thread IDs even when their visible CLI names match", () => {
    const view = selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([sourceThread("cli-one"), sourceThread("cli-two")]),
      10_000,
      { activityFilter: "all" },
    );

    expect(view.roots.map((thread) => thread.threadId)).toEqual(["cli-two", "cli-one"]);
    expect(view.roots.map((thread) => thread.name)).toEqual([
      "CLI — Main Agent",
      "CLI — Main Agent",
    ]);
  });

  it("filters source lanes and never projects historical-only records as active agents", () => {
    const history = sourceThread("history", {
      lifecycle: null,
      authorityProvenance: { ...nearLive, sourceKind: "historical-rollout-scan", temporalClass: "HISTORICAL" },
      liveLaneCount: 0,
      nearLiveLaneCount: 0,
      historicalLaneCount: 1,
    });
    const all = snapshot([sourceThread("cli"), history]);

    expect(selectUnifiedAgentMonitorView(
      liveRuntime(), all, 10_000, { sourceFilter: "monitor-live" },
    ).threads.map((thread) => thread.threadId)).toEqual(["paired"]);
    expect(selectUnifiedAgentMonitorView(
      liveRuntime(), all, 10_000, { sourceFilter: "cli-near-live" },
    ).threads.map((thread) => thread.threadId)).toEqual(["cli"]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), snapshot([history]), 10_000,
    ).threads).toEqual([]);
  });
});

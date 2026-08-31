import { describe, expect, it } from "vitest";
import phase24C from "../../../../docs/evidence/phase-2-4-c.json";

import {
  applyRuntimeRecords,
  createRuntimeState,
  type RuntimeProtocolRecord,
} from "../runtime";
import type {
  GlobalSourceEvidenceConfidence,
  GlobalSourceProducerSurface,
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

const historical: GlobalSourceProvenance = {
  ...nearLive,
  sourceKind: "historical-rollout-scan",
  temporalClass: "HISTORICAL",
  sourceInstanceId: "historical-scan:home-1",
  freshness: {
    state: "settled",
    lastCompleteRecordObservedAtMs: 9_400,
    reason: "historical scan",
  },
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

function producerSurface(
  surface: GlobalSourceProducerSurface,
  confidence: GlobalSourceEvidenceConfidence = "confirmed",
) {
  return {
    surface,
    confidence,
    evidence: [`fixture:${surface}`],
    provenance: ["fixture"],
  };
}

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
    producerSurface: producerSurface("CLI"),
    workspaceAssignment: null,
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
  it("projects Desktop Main/Sub-Agent hierarchy with producer, workspace, model, lifecycle, Runtime, Token, and latest activity", () => {
    const main = sourceThread("desktop-main", {
      producerSurface: producerSurface("DESKTOP"),
      workspaceAssignment: {
        state: "ASSIGNED",
        workspaceId: "desktop-workspace",
        provenance: "desktop-project-assignment",
        matchedPath: "f:/ai/codexmonitor",
        candidateWorkspaceIds: ["desktop-workspace"],
      },
    });
    const child = sourceThread("desktop-child", {
      parentThreadKey: { value: main.key, provenance: nearLive },
      agentPath: { value: "/root/desktop_reader", provenance: nearLive },
      producerSurface: producerSurface("DESKTOP", "inferred"),
      workspaceAssignment: {
        state: "ASSIGNED",
        workspaceId: "desktop-workspace",
        provenance: "confirmed-parent-edge",
        matchedPath: null,
        candidateWorkspaceIds: ["desktop-workspace"],
      },
    });

    const view = selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([main, child]),
      10_000,
      { workspaceId: "desktop-workspace" },
    );

    expect(view.threads).toHaveLength(2);
    expect(view.threads[0]).toMatchObject({
      threadId: "desktop-main",
      workspaceId: "desktop-workspace",
      isCurrentEligible: false,
      name: "Desktop — Main Agent",
      producer: { surface: "DESKTOP", confidence: "confirmed" },
      modelId: "gpt-cli",
      status: "running",
      runtimeMs: 2_000,
      totalTokens: 125,
      source: { temporalClass: "NEAR_LIVE", sourceTimestampMs: 9_000 },
    });
    expect(view.roots[0]?.children[0]).toMatchObject({
      threadId: "desktop-child",
      name: "desktop_reader",
      role: "/root/desktop_reader",
      producer: { surface: "DESKTOP", confidence: "inferred" },
    });
  });

  it("keeps Producer filtering orthogonal to Source and Activity when Desktop and CLI coexist", () => {
    const desktop = sourceThread("desktop", {
      producerSurface: producerSurface("DESKTOP"),
    });
    const cli = sourceThread("cli", {
      producerSurface: producerSurface("CLI"),
    });
    const sources = snapshot([desktop, cli]);

    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000,
      { sourceFilter: "cli-near-live", producerFilter: "desktop", activityFilter: "active-fresh" },
    ).threads.map((thread) => thread.threadId)).toEqual(["desktop"]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000,
      { sourceFilter: "cli-near-live", producerFilter: "cli", activityFilter: "active-fresh" },
    ).threads.map((thread) => thread.threadId)).toEqual(["cli"]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), sources, 10_000,
      { sourceFilter: "monitor-live", producerFilter: "desktop", activityFilter: "all" },
    ).threads).toEqual([]);

    const settledDesktop = sourceThread("settled-desktop", {
      producerSurface: producerSurface("DESKTOP"),
      lifecycle: { value: "completed", provenance: settledNearLive },
      authorityProvenance: settledNearLive,
    });
    const activeCliChild = sourceThread("active-cli-child", {
      parentThreadKey: { value: settledDesktop.key, provenance: nearLive },
      producerSurface: producerSurface("CLI"),
    });
    const mixedSession = snapshot([settledDesktop, activeCliChild]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), mixedSession, 10_000,
      { producerFilter: "desktop", activityFilter: "active-fresh" },
    ).threads).toEqual([]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(), mixedSession, 10_000,
      { producerFilter: "desktop", activityFilter: "all" },
    ).threads.map((thread) => thread.threadId)).toEqual(["settled-desktop"]);
  });

  it("keeps canonical AMBIGUOUS and UNKNOWN producers visible without disguising them, but excludes zero-lane stale orphans", () => {
    const ambiguous = sourceThread("ambiguous", {
      producerSurface: producerSurface("AMBIGUOUS", "inferred"),
    });
    const unknown = sourceThread("unknown", {
      producerSurface: producerSurface("UNKNOWN", "unknown"),
    });
    const staleOrphan = sourceThread("stale-orphan", {
      producerSurface: producerSurface("DESKTOP"),
      liveLaneCount: 0,
      nearLiveLaneCount: 0,
      historicalLaneCount: 0,
    });

    const view = selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([ambiguous, unknown, staleOrphan]),
      10_000,
      { activityFilter: "all" },
    );

    expect(view.threads.map((thread) => thread.threadId)).toEqual(["unknown", "ambiguous"]);
    expect(view.threads.map((thread) => ({
      name: thread.name,
      producer: thread.producer.surface,
    }))).toEqual([
      { name: "Unknown Producer — Main Agent", producer: "UNKNOWN" },
      { name: "Ambiguous Producer — Main Agent", producer: "AMBIGUOUS" },
    ]);
    expect(selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([ambiguous, unknown]),
      10_000,
      { producerFilter: "desktop", activityFilter: "all" },
    ).threads).toEqual([]);
  });

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

  it("does not project a historical model into a paired LIVE Runtime thread", () => {
    const completedPair = sourceThread("paired", {
      lifecycle: { value: "completed", provenance: freshLive },
      observedModel: { value: "gpt-history", provenance: historical },
      authorityProvenance: freshLive,
      liveLaneCount: 1,
      nearLiveLaneCount: 0,
      historicalLaneCount: 1,
    });

    const view = selectUnifiedAgentMonitorView(
      liveRuntime(),
      snapshot([completedPair]),
      10_000,
    );

    expect(view.threads).toHaveLength(1);
    expect(view.threads[0]).toMatchObject({
      threadId: "paired",
      modelId: null,
      modelSource: null,
      source: { temporalClass: "LIVE" },
    });
  });

  it("does not project a historical model into a NEAR LIVE source thread", () => {
    const nearLiveThread = sourceThread("cli-history", {
      observedModel: { value: "gpt-history", provenance: historical },
      historicalLaneCount: 1,
    });

    const view = selectUnifiedAgentMonitorView(
      createRuntimeState(),
      snapshot([nearLiveThread]),
      10_000,
    );

    expect(view.threads[0]).toMatchObject({
      threadId: "cli-history",
      modelId: null,
      modelSource: null,
      source: { temporalClass: "NEAR_LIVE" },
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

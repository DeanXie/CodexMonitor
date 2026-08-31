import { describe, expect, it } from "vitest";

import multiStartFixture from "../../../../docs/fixtures/app-server/multi-agent-start.events.json";
import { applyRuntimeRecords, createRuntimeState, type RuntimeProtocolRecord } from "../runtime";
import {
  selectAgentMonitorRuntimeView,
  selectAgentMonitorSessionOptions,
} from "./agentRuntimeSelector";
import type { AgentMonitorRuntimeThread } from "../types";

describe("selectAgentMonitorRuntimeView", () => {
  it("deduplicates root sessions, puts Current first, and formats title plus short id", () => {
    const base = {
      codexHomeIdentity: null,
      workspaceId: "workspace",
      parentThreadId: null,
      createdAtMs: null,
      isCurrentEligible: true,
      name: "Main Agent",
      producer: { surface: "MONITOR" as const, confidence: "confirmed" as const, evidence: [], provenance: [] },
      modelId: null,
      effort: null,
      role: null,
      isSubagent: false,
      status: "idle" as const,
      runtimeMs: null,
      totalTokens: null,
      tokenUsage: null,
      source: {
        sourceKind: "monitor-app-server" as const,
        temporalClass: "LIVE" as const,
        freshnessState: "fresh" as const,
        ageMs: 0,
        sourceTimestampMs: null,
        observedTimestampMs: null,
      },
    };
    const currentId = "01a02fb4-1111-2222-3333-444444444444";
    const otherId = "01a02eee-1111-2222-3333-444444444444";
    const threads: AgentMonitorRuntimeThread[] = [
      { ...base, threadId: otherId },
      { ...base, threadId: currentId },
      { ...base, threadId: currentId },
      { ...base, threadId: "child", parentThreadId: currentId, isSubagent: true },
    ];

    expect(selectAgentMonitorSessionOptions(threads, {
      currentThreadId: currentId,
      workspaceId: null,
      titlesByThreadId: {
        [currentId]: "场景 A 实时验证任务",
        [otherId]: "旧任务",
      },
    })).toEqual({
      currentObserved: true,
      options: [
        {
          threadId: currentId,
          label: "● Current — 场景 A 实时验证任务 — 01a02fb4",
          isCurrent: true,
        },
        {
          threadId: otherId,
          label: "旧任务 — 01a02eee",
          isCurrent: false,
        },
      ],
    });
  });

  it("reports an unobserved current thread without selecting another session", () => {
    expect(selectAgentMonitorSessionOptions([], {
      currentThreadId: "not-observed",
      workspaceId: null,
      titlesByThreadId: {},
    })).toEqual({ currentObserved: false, options: [] });
  });

  it("projects the real Main plus three Sub-Agent fixture without historical inputs", () => {
    const runtimeState = applyRuntimeRecords(
      createRuntimeState(),
      multiStartFixture.records as RuntimeProtocolRecord[],
      30_000,
    );
    const view = selectAgentMonitorRuntimeView(runtimeState, 1_787_440_200_000);
    const main = view.roots.find((node) => node.threadId === "thread-main");

    expect(view.threads).toHaveLength(4);
    expect(main?.children.map((child) => child.threadId)).toEqual([
      "thread-early-child-a",
      "thread-early-child-b",
      "thread-early-reviewer-c",
    ]);
    expect(main?.modelId).toBeNull();
    expect(main?.totalTokens).toBe(391_060);
    expect(main?.children[0]).toMatchObject({
      role: "/root/early_a",
      modelId: null,
      totalTokens: 21_813,
    });
  });

  it("exposes unavailable live model and token when Runtime has no evidence", () => {
    const runtimeState = applyRuntimeRecords(createRuntimeState(), [{
      source: "EVENT",
      capturedAt: "2026-08-23T08:00:00.000Z",
      label: "thread/started",
      payload: {
        workspace_id: "workspace",
        message: {
          method: "thread/started",
          params: { thread: { id: "thread-runtime-only", status: { type: "idle" } } },
        },
      },
    }], 90_000);

    const view = selectAgentMonitorRuntimeView(runtimeState, 100_000);

    expect(view.roots[0]).toMatchObject({
      modelId: null,
      totalTokens: null,
      tokenUsage: null,
      status: "idle",
    });
  });

  it("keeps completed lifecycle and confirmed model while observation freshness ages independently", () => {
    const runtimeState = applyRuntimeRecords(createRuntimeState(), [
      {
        source: "EVENT",
        capturedAt: "2026-08-26T00:00:00Z",
        label: "thread/started",
        payload: {
          workspace_id: "workspace",
          message: {
            method: "thread/started",
            params: { thread: { id: "completed", status: { type: "active" } } },
          },
        },
      },
      {
        source: "EVENT",
        capturedAt: "2026-08-26T00:00:01Z",
        label: "thread/settings/updated",
        payload: {
          workspace_id: "workspace",
          message: {
            method: "thread/settings/updated",
            params: { threadId: "completed", threadSettings: { model: "gpt-confirmed" } },
          },
        },
      },
      {
        source: "EVENT",
        capturedAt: "2026-08-26T00:00:02Z",
        label: "turn/started",
        payload: {
          workspace_id: "workspace",
          message: {
            method: "turn/started",
            params: {
              threadId: "completed",
              turn: { id: "turn-completed" },
            },
          },
        },
      },
      {
        source: "EVENT",
        capturedAt: "2026-08-26T00:00:03Z",
        label: "turn/completed",
        payload: {
          workspace_id: "workspace",
          message: {
            method: "turn/completed",
            params: {
              threadId: "completed",
              turn: { id: "turn-completed", status: "completed", completedAt: 3, durationMs: 1_000, error: null },
            },
          },
        },
      },
    ], 1_000);

    expect(selectAgentMonitorRuntimeView(runtimeState, 7_000).roots[0]).toMatchObject({
      status: "completed",
      modelId: "gpt-confirmed",
      source: { temporalClass: "LIVE", freshnessState: "stale" },
    });
  });

  it("prunes a deleted main thread and its descendants without mutating Runtime", () => {
    const runtimeState = applyRuntimeRecords(
      createRuntimeState(),
      multiStartFixture.records as RuntimeProtocolRecord[],
      30_000,
    );
    const runtimeThreadIdsBefore = Object.keys(runtimeState.threads);

    const view = selectAgentMonitorRuntimeView(runtimeState, 1_787_440_200_000, {
      excludedThreadIds: new Set(["thread-main"]),
    });

    expect(view.threads).toEqual([]);
    expect(Object.keys(runtimeState.threads)).toEqual(runtimeThreadIdsBefore);
  });
});

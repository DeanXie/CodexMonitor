import { describe, expect, it } from "vitest";

import multiCompletionFixture from "../../../../docs/fixtures/app-server/multi-agent-completion.events.json";
import multiStartFixture from "../../../../docs/fixtures/app-server/multi-agent-start.events.json";
import singleFixture from "../../../../docs/fixtures/app-server/single-agent.events.json";
import { normalizeRuntimeRecord } from "./eventNormalizer";
import {
  applyRuntimeEvent,
  applyRuntimeRecords,
  clearLiveRuntimeState,
  createRuntimeState,
  getLiveRuntimeClearState,
} from "./runtimeState";
import type { RuntimeProtocolRecord } from "./types";

const singleRecords = singleFixture.records as RuntimeProtocolRecord[];
const multiStartRecords = multiStartFixture.records as RuntimeProtocolRecord[];
const multiCompletionRecords = multiCompletionFixture.records as RuntimeProtocolRecord[];

describe("runtime state", () => {
  it("clears all idle Live Runtime layers", () => {
    const idle = applyRuntimeRecords(createRuntimeState(), singleRecords, 10_000);
    idle.assignments["idle-assignment"] = {
      assignmentId: "idle-assignment",
      parentThreadId: "thread-single",
      childThreadId: "thread-child",
      agentThreadId: "thread-child",
      agentPath: "/root/idle_child",
      spawnedAt: {
        valueMs: 10_000,
        provenance: {
          eventKey: "idle-assignment-event",
          method: "item/started",
          recordSource: "EVENT",
          serverTimeMs: null,
          observedAtMs: 10_000,
        },
      },
      provenance: {
        eventKey: "idle-assignment-event",
        method: "item/started",
        recordSource: "EVENT",
        serverTimeMs: null,
        observedAtMs: 10_000,
      },
    };

    expect(Object.keys(idle.threads).length).toBeGreaterThan(0);
    expect(Object.keys(idle.turns).length).toBeGreaterThan(0);
    expect(Object.keys(idle.assignments)).toHaveLength(1);

    expect(clearLiveRuntimeState(idle)).toEqual(createRuntimeState());
  });

  it.each(["running", "waiting"] as const)(
    "refuses to clear a %s turn",
    (status) => {
      const active = createRuntimeState();
      active.turns["active-turn"] = {
        turnId: "active-turn",
        threadId: "active-thread",
        status: {
          value: status,
          provenance: {
            eventKey: `active-${status}`,
            method: `test/${status}`,
            recordSource: "EVENT",
            serverTimeMs: null,
            observedAtMs: 1,
          },
        },
        requestedModel: null,
        startedAt: null,
        completedAt: null,
        durationMs: null,
        tokenDelta: {
          cacheWriteInputTokens: 0,
          cachedInputTokens: 0,
          inputTokens: 0,
          outputTokens: 0,
          reasoningOutputTokens: 0,
          totalTokens: 0,
        },
        lastActivityAt: null,
      };

      expect(getLiveRuntimeClearState(active)).toEqual({
        canClear: false,
        activeTurnIds: ["active-turn"],
      });
      expect(clearLiveRuntimeState(active)).toBe(active);
    },
  );

  it("builds separate thread and turn state from the real single-agent fixture", () => {
    const state = applyRuntimeRecords(createRuntimeState(), singleRecords, 10_000);
    const thread = state.threads["thread-single"];
    const turn = state.turns["turn-single-1"];

    expect(thread).toMatchObject({
      threadId: "thread-single",
      status: { value: "idle" },
      observedModel: {
        value: "gpt-5.6-sol",
        source: "threadSettingsUpdated",
      },
      tokenUsage: {
        total: {
          inputTokens: 23_760,
          cachedInputTokens: 6_912,
          outputTokens: 18,
          reasoningOutputTokens: 0,
          totalTokens: 23_778,
        },
      },
    });
    expect(thread.createdAt).toMatchObject({
      valueMs: 1_787_440_105_000,
      provenance: { serverTimeMs: 1_787_440_105_000 },
    });
    expect(turn).toMatchObject({
      turnId: "turn-single-1",
      threadId: "thread-single",
      status: { value: "completed" },
      requestedModel: { value: "gpt-5.6-sol" },
      durationMs: { value: 5_321 },
      tokenDelta: {
        inputTokens: 23_760,
        cachedInputTokens: 6_912,
        outputTokens: 18,
        reasoningOutputTokens: 0,
        totalTokens: 23_778,
      },
    });
    expect(thread.status?.value).not.toBe("completed");
  });

  it("builds three independent assignments and child token totals from real fixtures", () => {
    const withCompletionBaseline = applyRuntimeRecords(
      createRuntimeState(),
      multiCompletionRecords,
      20_000,
    );
    const state = applyRuntimeRecords(withCompletionBaseline, multiStartRecords, 30_000);

    expect(Object.values(state.assignments)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          parentThreadId: "thread-main",
          childThreadId: "thread-early-child-a",
          agentPath: "/root/early_a",
        }),
        expect.objectContaining({
          parentThreadId: "thread-main",
          childThreadId: "thread-early-child-b",
          agentPath: "/root/early_b",
        }),
        expect.objectContaining({
          parentThreadId: "thread-main",
          childThreadId: "thread-early-reviewer-c",
          agentPath: "/root/early_reviewer_c",
        }),
      ]),
    );
    expect(state.threads["thread-early-child-a"].parentThreadId).toBe("thread-main");
    expect(state.threads["thread-main"].childThreadIds).toEqual([
      "thread-early-child-a",
      "thread-early-child-b",
      "thread-early-reviewer-c",
    ]);
    expect(state.threads["thread-early-child-a"].tokenUsage?.total.totalTokens).toBe(21_813);
    expect(state.threads["thread-early-child-b"].tokenUsage?.total.totalTokens).toBe(23_584);
    expect(state.threads["thread-early-reviewer-c"].tokenUsage?.total.totalTokens).toBe(23_588);
    expect(state.threads["thread-early-child-a"].observedModel).toBeNull();
  });

  it("is idempotent when every captured event is replayed", () => {
    const once = applyRuntimeRecords(createRuntimeState(), multiStartRecords, 40_000);
    const twice = applyRuntimeRecords(once, multiStartRecords, 40_000);

    expect(twice).toEqual(once);
    expect(Object.keys(twice.assignments)).toHaveLength(3);
    expect(twice.turns["turn-main-2"].tokenDelta.totalTokens).toBe(174_035);
  });

  it("uses real collab wait items to enter and leave Waiting without ending the turn", () => {
    const waitStartedIndex = multiStartRecords.findIndex((record) => {
      const message = record.payload.message as Record<string, unknown> | undefined;
      const params = message?.params as Record<string, unknown> | undefined;
      const item = params?.item as Record<string, unknown> | undefined;
      return record.label === "item/started" && item?.type === "collabAgentToolCall";
    });
    const waitCompletedIndex = multiStartRecords.findIndex((record, index) => {
      const message = record.payload.message as Record<string, unknown> | undefined;
      const params = message?.params as Record<string, unknown> | undefined;
      const item = params?.item as Record<string, unknown> | undefined;
      return (
        index > waitStartedIndex &&
        record.label === "item/completed" &&
        item?.type === "collabAgentToolCall"
      );
    });

    const waiting = applyRuntimeRecords(
      createRuntimeState(),
      multiStartRecords.slice(0, waitStartedIndex + 1),
      45_000,
    );
    const resumed = applyRuntimeRecords(
      waiting,
      [multiStartRecords[waitCompletedIndex]],
      46_000,
    );

    expect(waiting.turns["turn-main-2"].status?.value).toBe("waiting");
    expect(resumed.turns["turn-main-2"].status?.value).toBe("running");
  });

  it("preserves newer model and terminal turn evidence when real records arrive out of order", () => {
    const olderStartResponse = structuredClone(
      singleRecords.find((record) => record.label === "thread/start response")!,
    );
    const olderResult = olderStartResponse.payload.result as Record<string, unknown>;
    olderResult.model = "older-requested-model";
    const labels = [
      "thread/started",
      "thread/settings/updated",
      "turn/completed",
      "thread/tokenUsage/updated",
      "turn/started",
      "thread/status/changed",
    ];
    const reordered = [
      ...labels.flatMap((label) => singleRecords.filter((record) => record.label === label)),
      olderStartResponse,
    ];
    const state = applyRuntimeRecords(createRuntimeState(), reordered, 50_000);

    expect(state.threads["thread-single"].observedModel).toMatchObject({
      value: "gpt-5.6-sol",
      source: "threadSettingsUpdated",
    });
    expect(state.turns["turn-single-1"].status?.value).toBe("completed");
    expect(state.turns["turn-single-1"].tokenDelta.totalTokens).toBeGreaterThanOrEqual(0);
    expect(state.threads["thread-single"].tokenUsage?.total.totalTokens).toBe(23_778);

    const originalSettings = singleRecords.find(
      (record) => record.label === "thread/settings/updated",
    )!;
    const newerSettings = structuredClone(originalSettings);
    const newerMessage = newerSettings.payload.message as Record<string, unknown>;
    newerMessage.emittedAtMs = 1_787_440_106_934;
    const newerParams = newerMessage.params as Record<string, unknown>;
    const newerThreadSettings = newerParams.threadSettings as Record<string, unknown>;
    newerThreadSettings.model = "newer-confirmed-model";
    const modelState = applyRuntimeRecords(
      createRuntimeState(),
      [newerSettings, originalSettings],
      51_000,
    );

    expect(modelState.threads["thread-single"].observedModel?.value).toBe(
      "newer-confirmed-model",
    );
  });

  it("keeps the newest thread snapshot while attributing real token updates in reverse order", () => {
    const baseline = applyRuntimeRecords(
      createRuntimeState(),
      multiCompletionRecords,
      55_000,
    );
    const reversedTokenRecords = multiStartRecords
      .filter((record) => {
        const message = record.payload.message as Record<string, unknown> | undefined;
        const params = message?.params as Record<string, unknown> | undefined;
        return record.label === "thread/tokenUsage/updated" && params?.threadId === "thread-main";
      })
      .reverse();
    const state = applyRuntimeRecords(baseline, reversedTokenRecords, 56_000);

    expect(state.threads["thread-main"].tokenUsage?.total.totalTokens).toBe(391_060);
    expect(state.turns["turn-main-2"].tokenDelta.totalTokens).toBe(174_035);
  });

  it("does not create negative turn deltas when a newer cumulative snapshot resets", () => {
    const tokenRecord = singleRecords.find(
      (record) => record.label === "thread/tokenUsage/updated",
    );
    expect(tokenRecord).toBeDefined();
    const firstEvents = normalizeRuntimeRecord(tokenRecord!, 60_000);
    const first = firstEvents.reduce(applyRuntimeEvent, createRuntimeState());
    const resetRecord = structuredClone(tokenRecord!);
    const message = resetRecord.payload.message as Record<string, unknown>;
    message.emittedAtMs = 1_787_440_112_000;
    const params = message.params as Record<string, unknown>;
    const usage = params.tokenUsage as Record<string, unknown>;
    usage.total = {
      cacheWriteInputTokens: 0,
      cachedInputTokens: 10,
      inputTokens: 100,
      outputTokens: 5,
      reasoningOutputTokens: 0,
      totalTokens: 105,
    };
    usage.last = {
      cacheWriteInputTokens: 0,
      cachedInputTokens: 10,
      inputTokens: 100,
      outputTokens: 5,
      reasoningOutputTokens: 0,
      totalTokens: 105,
    };
    const state = normalizeRuntimeRecord(resetRecord, 60_001).reduce(applyRuntimeEvent, first);

    expect(state.threads["thread-single"].tokenUsage?.total.totalTokens).toBe(105);
    expect(state.turns["turn-single-1"].tokenDelta.totalTokens).toBeGreaterThanOrEqual(0);
    expect(state.turns["turn-single-1"].tokenDelta.inputTokens).toBeGreaterThanOrEqual(0);
  });

  it("records server timestamps separately from fallback observation time", () => {
    const state = applyRuntimeRecords(createRuntimeState(), singleRecords, 70_000);
    const model = state.threads["thread-single"].observedModel;
    const requested = state.turns["turn-single-1"].requestedModel;

    expect(model?.provenance).toMatchObject({
      method: "thread/settings/updated",
      serverTimeMs: 1_787_440_105_934,
    });
    expect(requested?.provenance).toMatchObject({
      method: "turn/start",
      serverTimeMs: null,
      observedAtMs: expect.any(Number),
    });

    const fallbackRecord: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "00:00:00",
      label: "item/started",
      payload: {
        workspace_id: "workspace",
        message: {
          method: "item/started",
          params: {
            threadId: "parent",
            turnId: "turn",
            item: {
              id: "assignment-without-server-time",
              type: "subAgentActivity",
              kind: "started",
              agentThreadId: "child",
              agentPath: "/root/child",
            },
          },
        },
      },
    };
    const fallbackState = applyRuntimeRecords(createRuntimeState(), [fallbackRecord], 90_000);

    expect(fallbackState.assignments["assignment-without-server-time"].spawnedAt).toMatchObject({
      valueMs: 90_000,
      provenance: { serverTimeMs: null, observedAtMs: 90_000 },
    });
  });
});

import { describe, expect, it } from "vitest";

import multiStartFixture from "../../../../docs/fixtures/app-server/multi-agent-start.events.json";
import singleFixture from "../../../../docs/fixtures/app-server/single-agent.events.json";
import { normalizeRuntimeRecord } from "./eventNormalizer";
import type { RuntimeProtocolRecord } from "./types";

const singleRecords = singleFixture.records as RuntimeProtocolRecord[];
const multiStartRecords = multiStartFixture.records as RuntimeProtocolRecord[];

describe("normalizeRuntimeRecord", () => {
  it("keeps requested and confirmed observed models as separate evidence", () => {
    const events = singleRecords.flatMap((record, index) =>
      normalizeRuntimeRecord(record, 1_000 + index),
    );

    expect(events.filter((event) => event.type === "turnRequested")).toEqual([
      expect.objectContaining({
        threadId: "thread-single",
        requestedModel: "gpt-5.6-sol",
        provenance: expect.objectContaining({
          method: "turn/start",
          recordSource: "CLIENT",
          serverTimeMs: null,
        }),
      }),
    ]);
    expect(events.filter((event) => event.type === "observedModelConfirmed")).toEqual([
      expect.objectContaining({
        threadId: "thread-single",
        model: "gpt-5.6-sol",
        source: "threadStartResponse",
      }),
      expect.objectContaining({
        threadId: "thread-single",
        model: "gpt-5.6-sol",
        source: "threadSettingsUpdated",
      }),
    ]);
  });

  it("normalizes the three real subAgentActivity parent-child edges", () => {
    const assignments = multiStartRecords
      .flatMap((record, index) => normalizeRuntimeRecord(record, 2_000 + index))
      .filter((event) => event.type === "assignmentStarted");

    expect(assignments).toEqual([
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
    ]);
    expect(assignments.every((event) => event.provenance.serverTimeMs !== null)).toBe(true);
  });

  it("does not invent model/rerouted support without a captured schema", () => {
    const record: RuntimeProtocolRecord = {
      source: "EVENT",
      capturedAt: "00:00:00",
      label: "model/rerouted",
      payload: {
        workspace_id: "workspace",
        message: {
          emittedAtMs: 123,
          method: "model/rerouted",
          params: { model: "unverified" },
        },
      },
    };

    expect(normalizeRuntimeRecord(record, 456)).toEqual([]);
  });

  it("hydrates thread identity and an active turn without accepting model or token history", () => {
    const record: RuntimeProtocolRecord = {
      source: "HYDRATION",
      capturedAt: "2026-08-23T09:00:00.000Z",
      label: "app/runtime hydration",
      payload: {
        workspaceId: "workspace-catch-up",
        threadId: "thread-catch-up",
        parentThreadId: null,
        createdAtMs: 1_700_000_000_000,
        threadStatus: "active",
        activeTurn: {
          turnId: "turn-catch-up",
          status: "running",
          startedAtMs: 1_700_000_001_000,
        },
        modelId: "historical-model-must-be-ignored",
        tokenUsage: { totalTokens: 999_999 },
      },
    };

    const events = normalizeRuntimeRecord(record, 1_700_000_002_000);

    expect(events).toEqual(expect.arrayContaining([
      expect.objectContaining({
        type: "threadHydrated",
        threadId: "thread-catch-up",
        status: "active",
      }),
      expect.objectContaining({
        type: "turnHydrated",
        threadId: "thread-catch-up",
        turnId: "turn-catch-up",
        status: "running",
      }),
    ]));
    expect(events.some((event) => event.type === "observedModelConfirmed")).toBe(false);
    expect(events.some((event) => event.type === "threadTokensUpdated")).toBe(false);
    expect(events.every((event) => event.provenance.recordSource === "HYDRATION")).toBe(true);
    expect(events.every((event) => event.provenance.serverTimeMs === null)).toBe(true);
  });
});

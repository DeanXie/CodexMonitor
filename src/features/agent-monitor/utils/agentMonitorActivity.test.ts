import { describe, expect, it } from "vitest";

import type { AgentMonitorRuntimeThread } from "../types";
import { describeSessionActivity } from "./agentMonitorActivity";

function runtimeThread(
  threadId: string,
  createdAtMs: number,
  sourceTimestampMs: number,
): AgentMonitorRuntimeThread {
  return {
    threadId,
    name: threadId,
    producer: {
      surface: "CLI",
      confidence: "confirmed",
      evidence: [],
      provenance: [],
    },
    modelId: null,
    effort: null,
    role: null,
    isSubagent: false,
    status: "completed",
    runtimeMs: null,
    totalTokens: null,
    tokenUsage: null,
    source: {
      sourceKind: "codex-cli-rollout",
      temporalClass: "NEAR_LIVE",
      freshnessState: "settled",
      ageMs: null,
      sourceTimestampMs,
      observedTimestampMs: sourceTimestampMs + 1,
    },
    codexHomeIdentity: "codex-home:phase-3-4-3",
    workspaceId: null,
    parentThreadId: null,
    createdAtMs,
    isCurrentEligible: false,
  };
}

describe("agentMonitorActivity", () => {
  it("long_lived_thread_uses_latest_activity_not_creation_time", () => {
    const createdAtMs = 1_700_000_000_000;
    const latestActivityAtMs = 1_800_000_000_000;

    const activity = describeSessionActivity(
      [runtimeThread("long-lived-thread", createdAtMs, latestActivityAtMs)],
      null,
    );

    expect(activity.lastActivityAtMs).toBe(latestActivityAtMs);
    expect(activity.lastActivityAtMs).not.toBe(createdAtMs);
  });
});

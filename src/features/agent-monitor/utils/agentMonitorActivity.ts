import type { AgentMonitorRuntimeThread } from "../types";

export type AgentMonitorActivityFilter = "active-fresh" | "all" | "settled";

type SessionActivity = {
  current: boolean;
  running: boolean;
  waiting: boolean;
  freshLive: boolean;
  freshNearLive: boolean;
  stale: boolean;
  settled: boolean;
  activeFresh: boolean;
  lastActivityAtMs: number;
  rootThreadId: string;
};

const ACTIVE_STATUSES = new Set(["active", "running", "waiting", "reviewing"]);

function activityTimestamp(thread: AgentMonitorRuntimeThread) {
  return thread.source.sourceTimestampMs
    ?? thread.source.observedTimestampMs
    ?? thread.createdAtMs
    ?? -1;
}

export function describeSessionActivity(
  threads: readonly AgentMonitorRuntimeThread[],
  currentThreadId: string | null,
): SessionActivity {
  const root = threads.find((thread) => !thread.parentThreadId) ?? threads[0];
  const current = Boolean(
    root && root.isCurrentEligible && currentThreadId === root.threadId,
  );
  const running = threads.some((thread) => thread.status === "running");
  const waiting = threads.some((thread) => thread.status === "waiting");
  const freshLive = threads.some(
    (thread) => thread.source.temporalClass === "LIVE" && thread.source.freshnessState === "fresh",
  );
  const freshNearLive = threads.some(
    (thread) => thread.source.temporalClass === "NEAR_LIVE" && thread.source.freshnessState === "fresh",
  );
  const stale = threads.some((thread) => thread.source.freshnessState === "stale");
  const hasSettledEvidence = threads.some(
    (thread) => thread.source.freshnessState === "settled" || thread.status === "completed",
  );
  const noActiveLifecycle = threads.every((thread) => !ACTIVE_STATUSES.has(thread.status));
  const activeFresh = current
    || freshLive
    || freshNearLive
    || threads.some(
      (thread) => ACTIVE_STATUSES.has(thread.status)
        && thread.source.freshnessState === "fresh",
    );
  return {
    current,
    running,
    waiting,
    freshLive,
    freshNearLive,
    stale,
    settled: !activeFresh && noActiveLifecycle && hasSettledEvidence,
    activeFresh,
    lastActivityAtMs: threads.reduce(
      (latest, thread) => Math.max(latest, activityTimestamp(thread)),
      -1,
    ),
    rootThreadId: root?.threadId ?? "",
  };
}

export function matchesActivityFilter(
  threads: readonly AgentMonitorRuntimeThread[],
  filter: AgentMonitorActivityFilter,
  currentThreadId: string | null,
) {
  if (filter === "all") return true;
  const activity = describeSessionActivity(threads, currentThreadId);
  return filter === "active-fresh" ? activity.activeFresh : activity.settled;
}

export function compareSessionActivity(
  leftThreads: readonly AgentMonitorRuntimeThread[],
  rightThreads: readonly AgentMonitorRuntimeThread[],
  currentThreadId: string | null,
) {
  const left = describeSessionActivity(leftThreads, currentThreadId);
  const right = describeSessionActivity(rightThreads, currentThreadId);
  const leftRank = [
    left.current,
    left.running,
    left.waiting,
    left.freshLive,
    left.freshNearLive,
    !left.stale,
    !left.settled,
  ];
  const rightRank = [
    right.current,
    right.running,
    right.waiting,
    right.freshLive,
    right.freshNearLive,
    !right.stale,
    !right.settled,
  ];
  for (let index = 0; index < leftRank.length; index += 1) {
    const difference = Number(rightRank[index]) - Number(leftRank[index]);
    if (difference !== 0) return difference;
  }
  if (right.lastActivityAtMs !== left.lastActivityAtMs) {
    return right.lastActivityAtMs - left.lastActivityAtMs;
  }
  return right.rootThreadId.localeCompare(left.rootThreadId);
}

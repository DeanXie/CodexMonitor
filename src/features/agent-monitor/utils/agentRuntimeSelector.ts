import type {
  AgentMonitorNode,
  AgentMonitorRuntimeThread,
  AgentMonitorRuntimeView,
  AgentMonitorSessionOption,
  AgentMonitorStatus,
} from "../types";
import type {
  AgentAssignment,
  AgentRuntimeStore,
  RuntimeProvenance,
  TurnRuntimeState,
} from "../runtime";
import { compareSessionActivity } from "./agentMonitorActivity";

type RuntimeViewFilter = {
  workspaceId?: string | null;
  sessionId?: string | null;
  excludedThreadIds?: ReadonlySet<string>;
};

function evidenceTime(provenance: RuntimeProvenance | null | undefined) {
  return provenance ? provenance.serverTimeMs ?? provenance.observedAtMs : -1;
}

function latestTurnForThread(
  runtimeState: AgentRuntimeStore,
  threadId: string,
): TurnRuntimeState | null {
  return Object.values(runtimeState.turns)
    .filter((turn) => turn.threadId === threadId)
    .sort(
      (left, right) =>
        evidenceTime(right.lastActivityAt?.provenance) -
        evidenceTime(left.lastActivityAt?.provenance),
    )[0] ?? null;
}

function assignmentByChild(runtimeState: AgentRuntimeStore) {
  const assignments = new Map<string, AgentAssignment>();
  Object.values(runtimeState.assignments).forEach((assignment) => {
    const existing = assignments.get(assignment.childThreadId);
    if (!existing || evidenceTime(assignment.provenance) >= evidenceTime(existing.provenance)) {
      assignments.set(assignment.childThreadId, assignment);
    }
  });
  return assignments;
}

function shortId(threadId: string) {
  return threadId.length <= 12 ? threadId : threadId.slice(0, 8);
}

const LIVE_FRESH_WINDOW_MS = 5_000;

function liveFreshness(observedTimestampMs: number | null, now: number) {
  if (observedTimestampMs === null) {
    return { state: "unknown" as const, ageMs: null };
  }
  const ageMs = Math.max(0, now - observedTimestampMs);
  return {
    state: ageMs <= LIVE_FRESH_WINDOW_MS ? "fresh" as const : "stale" as const,
    ageMs,
  };
}

export function selectAgentMonitorSessionOptions(
  threads: AgentMonitorRuntimeThread[],
  {
    currentThreadId,
    workspaceId,
    titlesByThreadId,
  }: {
    currentThreadId: string | null;
    workspaceId: string | null;
    titlesByThreadId: Record<string, string>;
  },
): { currentObserved: boolean; options: AgentMonitorSessionOption[] } {
  const rootThreads = threads.filter((thread) => !thread.parentThreadId);
  const currentObserved = Boolean(
    currentThreadId && rootThreads.some(
      (thread) => thread.isCurrentEligible && thread.threadId === currentThreadId,
    ),
  );
  const rootsById = new Map<string, AgentMonitorRuntimeThread>();
  rootThreads.forEach((thread) => {
    if (workspaceId && thread.workspaceId !== workspaceId) return;
    if (!rootsById.has(thread.threadId)) rootsById.set(thread.threadId, thread);
  });
  const options = Array.from(rootsById.values()).map((thread) => {
    const isCurrent = currentObserved && thread.threadId === currentThreadId;
    const title = titlesByThreadId[thread.threadId]?.trim()
      || (thread.source.temporalClass === "NEAR_LIVE" ? thread.name : "Main Agent");
    return {
      threadId: thread.threadId,
      label: `${isCurrent ? "● Current — " : ""}${title} — ${shortId(thread.threadId)}`,
      isCurrent,
    };
  });
  options.sort((left, right) => {
    const currentDifference = Number(right.isCurrent) - Number(left.isCurrent);
    if (currentDifference !== 0) return currentDifference;
    return compareSessionActivity(
      [rootsById.get(left.threadId)!],
      [rootsById.get(right.threadId)!],
      currentThreadId,
    );
  });
  return { currentObserved, options };
}

function displayName(
  threadId: string,
  assignment: AgentAssignment | undefined,
  isSubagent: boolean,
) {
  if (assignment?.agentPath) {
    const parts = assignment.agentPath.split("/").filter(Boolean);
    const leaf = parts[parts.length - 1];
    if (leaf) return leaf;
  }
  return `${isSubagent ? "Sub Agent" : "Main Agent"} · ${shortId(threadId)}`;
}

function selectStatus(
  turn: TurnRuntimeState | null,
  threadStatus: AgentMonitorStatus | null,
): AgentMonitorStatus {
  return turn?.status?.value ?? threadStatus ?? "unavailable";
}

function selectRuntimeMs(turn: TurnRuntimeState | null, now: number) {
  if (!turn) return null;
  if (
    (turn.status?.value === "running" || turn.status?.value === "waiting") &&
    turn.startedAt
  ) {
    return Math.max(0, now - turn.startedAt.valueMs);
  }
  return turn.durationMs?.value ?? null;
}

export function selectAgentMonitorRuntimeView(
  runtimeState: AgentRuntimeStore,
  now: number,
  filter: RuntimeViewFilter = {},
): AgentMonitorRuntimeView {
  const assignments = assignmentByChild(runtimeState);
  const parentByChild = new Map<string, string>();
  const assignmentParents = new Set<string>();
  assignments.forEach((assignment, childThreadId) => {
    parentByChild.set(childThreadId, assignment.parentThreadId);
    assignmentParents.add(assignment.parentThreadId);
  });

  const sessionDescendants = new Set<string>();
  if (filter.sessionId) {
    const pending = [filter.sessionId];
    while (pending.length) {
      const parentId = pending.pop()!;
      if (sessionDescendants.has(parentId)) continue;
      sessionDescendants.add(parentId);
      Object.values(runtimeState.threads).forEach((thread) => {
        const parentThreadId = parentByChild.get(thread.threadId) ?? thread.parentThreadId;
        if (parentThreadId === parentId) pending.push(thread.threadId);
      });
    }
  }

  const turnThreadIds = new Set(
    Object.values(runtimeState.turns)
      .filter((turn) => Boolean(turn.status || turn.startedAt))
      .map((turn) => turn.threadId),
  );
  const selectedThreads = Object.values(runtimeState.threads).filter((thread) => {
    const hasAgentIdentityEvidence =
      assignments.has(thread.threadId) ||
      assignmentParents.has(thread.threadId) ||
      Boolean(thread.identityProvenance) ||
      Boolean(
        thread.createdAt ||
        thread.observedModel ||
        (thread.status && thread.status.value !== "notLoaded"),
      ) ||
      turnThreadIds.has(thread.threadId);
    if (!hasAgentIdentityEvidence) return false;
    if (filter.excludedThreadIds?.has(thread.threadId)) return false;
    let ancestorId = parentByChild.get(thread.threadId) ?? thread.parentThreadId;
    const visited = new Set<string>();
    while (ancestorId && !visited.has(ancestorId)) {
      if (filter.excludedThreadIds?.has(ancestorId)) return false;
      visited.add(ancestorId);
      const ancestor = runtimeState.threads[ancestorId];
      ancestorId = parentByChild.get(ancestorId) ?? ancestor?.parentThreadId ?? null;
    }
    if (filter.workspaceId && thread.workspaceId !== filter.workspaceId) return false;
    return !filter.sessionId || sessionDescendants.has(thread.threadId);
  });
  const selectedIds = new Set(selectedThreads.map((thread) => thread.threadId));

  const threads: AgentMonitorRuntimeThread[] = selectedThreads.map((thread) => {
    const assignment = assignments.get(thread.threadId);
    const parentThreadId = parentByChild.get(thread.threadId) ?? thread.parentThreadId;
    const isSubagent = Boolean(parentThreadId || assignment);
    const turn = latestTurnForThread(runtimeState, thread.threadId);
    const tokenUsage = thread.tokenUsage?.total ?? null;
    const provenance = [
      turn?.lastActivityAt?.provenance,
      thread.lastActivityAt?.provenance,
      thread.tokenUsage?.provenance,
      thread.observedModel?.provenance,
      thread.identityProvenance,
    ]
      .filter((value): value is RuntimeProvenance => Boolean(value))
      .sort((left, right) => evidenceTime(right) - evidenceTime(left))[0] ?? null;
    const observedTimestampMs = provenance?.observedAtMs ?? null;
    const sourceFreshness = liveFreshness(observedTimestampMs, now);
    const modelFreshness = liveFreshness(
      thread.observedModel?.provenance.observedAtMs ?? null,
      now,
    );
    return {
      threadId: thread.threadId,
      codexHomeIdentity: null,
      workspaceId: thread.workspaceId,
      parentThreadId,
      createdAtMs: thread.createdAt?.valueMs ?? null,
      isCurrentEligible: true,
      name: displayName(thread.threadId, assignment, isSubagent),
      producer: {
        surface: "MONITOR",
        confidence: "confirmed",
        evidence: ["Monitor app-server Runtime observation"],
        provenance: ["monitor-app-server"],
      },
      modelId: thread.observedModel?.value ?? null,
      effort: null,
      role: assignment?.agentPath ?? null,
      isSubagent,
      status: selectStatus(turn, thread.status?.value ?? null),
      runtimeMs: selectRuntimeMs(turn, now),
      totalTokens: tokenUsage?.totalTokens ?? null,
      tokenUsage: tokenUsage
        ? {
            totalTokens: tokenUsage.totalTokens,
            inputTokens: tokenUsage.inputTokens,
            cachedInputTokens: tokenUsage.cachedInputTokens,
            outputTokens: tokenUsage.outputTokens,
            reasoningOutputTokens: tokenUsage.reasoningOutputTokens,
        }
        : null,
      modelSource: thread.observedModel
        ? {
            sourceKind: "monitor-app-server",
            temporalClass: "LIVE",
            freshnessState: modelFreshness.state,
            ageMs: modelFreshness.ageMs,
            sourceTimestampMs: thread.observedModel.provenance.serverTimeMs,
            observedTimestampMs: thread.observedModel.provenance.observedAtMs,
            sourceInstanceId: null,
            sourceGeneration: null,
            freshnessReason: thread.observedModel.source,
          }
        : null,
      source: {
        sourceKind: "monitor-app-server",
        temporalClass: "LIVE",
        freshnessState: sourceFreshness.state,
        ageMs: sourceFreshness.ageMs,
        sourceTimestampMs: provenance?.serverTimeMs ?? null,
        observedTimestampMs,
        sourceInstanceId: null,
        sourceGeneration: null,
        freshnessReason: provenance ? provenance.method : null,
      },
    };
  });

  const nodesById = new Map<string, AgentMonitorNode>();
  threads.forEach((thread) => {
    const {
      codexHomeIdentity: _codexHomeIdentity,
      workspaceId: _workspaceId,
      parentThreadId: _parentThreadId,
      createdAtMs: _createdAtMs,
      isCurrentEligible: _isCurrentEligible,
      ...node
    } = thread;
    nodesById.set(thread.threadId, { ...node, children: [] });
  });
  const roots: AgentMonitorNode[] = [];
  threads.forEach((thread) => {
    const node = nodesById.get(thread.threadId)!;
    const parent = thread.parentThreadId && selectedIds.has(thread.parentThreadId)
      ? nodesById.get(thread.parentThreadId)
      : null;
    if (parent && parent.threadId !== thread.threadId) parent.children.push(node);
    else roots.push(node);
  });

  return { roots, threads };
}

import type {
  AgentMonitorNode,
  AgentMonitorRuntimeThread,
  AgentMonitorRuntimeView,
  AgentMonitorSourceInfo,
  AgentMonitorStatus,
} from "../types";
import type { AgentRuntimeStore } from "../runtime";
import type {
  GlobalSourceProducerClassification,
  GlobalSourceProducerSurface,
  GlobalSourceProvenance,
  GlobalSourceSnapshot,
  GlobalSourceThread,
} from "../global-source/types";
import { selectAgentMonitorRuntimeView } from "./agentRuntimeSelector";
import {
  compareSessionActivity,
  matchesActivityFilter,
  type AgentMonitorActivityFilter,
} from "./agentMonitorActivity";

export type AgentMonitorSourceFilter = "all" | "monitor-live" | "cli-near-live";
export type AgentMonitorProducerFilter = "all" | "monitor" | "desktop" | "cli" | "ide";

type UnifiedViewFilter = {
  workspaceId?: string | null;
  sessionId?: string | null;
  excludedThreadIds?: ReadonlySet<string>;
  sourceFilter?: AgentMonitorSourceFilter;
  producerFilter?: AgentMonitorProducerFilter;
  activityFilter?: AgentMonitorActivityFilter;
  currentThreadId?: string | null;
};

function canonicalKey(codexHomeIdentity: string, threadId: string) {
  return `${codexHomeIdentity}\u001f${threadId}`;
}

function shortId(threadId: string) {
  return threadId.length <= 12 ? threadId : threadId.slice(0, 8);
}

function evidenceTime(provenance: GlobalSourceProvenance | null | undefined) {
  if (!provenance) return null;
  return provenance.sourceTimestampMs ?? provenance.observedTimestampMs;
}

function provenanceSourceInfo(
  provenance: GlobalSourceProvenance | null | undefined,
  now: number,
): AgentMonitorSourceInfo {
  const temporalClass = provenance?.temporalClass
    ?? "HISTORICAL";
  const sourceKind = provenance?.sourceKind
    ?? (temporalClass === "LIVE"
      ? "monitor-app-server"
      : temporalClass === "NEAR_LIVE"
        ? "codex-cli-rollout"
        : "historical-rollout-scan");
  const sourceTimestampMs = provenance?.sourceTimestampMs ?? null;
  const observedTimestampMs = provenance?.observedTimestampMs ?? null;
  return {
    sourceKind,
    temporalClass,
    freshnessState: provenance?.freshness.state ?? "unknown",
    ageMs: sourceTimestampMs === null ? null : Math.max(0, now - sourceTimestampMs),
    observedAgeMs: observedTimestampMs === null ? null : Math.max(0, now - observedTimestampMs),
    sourceTimestampMs,
    observedTimestampMs,
    sourceInstanceId: provenance?.sourceInstanceId ?? null,
    sourceGeneration: provenance?.sourceGeneration ?? null,
    freshnessReason: provenance?.freshness.reason ?? null,
  };
}

function sourceInfo(thread: GlobalSourceThread, now: number) {
  const provenance = thread.authorityProvenance
    ?? thread.lifecycle?.provenance
    ?? thread.tokenSnapshot?.provenance
    ?? thread.observedModel?.provenance
    ?? thread.parentThreadKey?.provenance
    ?? thread.agentPath?.provenance
    ?? null;
  if (provenance) return provenanceSourceInfo(provenance, now);
  const temporalClass = thread.liveLaneCount > 0
    ? "LIVE"
    : thread.nearLiveLaneCount > 0
      ? "NEAR_LIVE"
      : "HISTORICAL";
  return {
    ...provenanceSourceInfo(null, now),
    temporalClass,
    sourceKind: temporalClass === "LIVE"
      ? "monitor-app-server"
      : temporalClass === "NEAR_LIVE"
        ? "codex-cli-rollout"
        : "historical-rollout-scan",
  } as AgentMonitorSourceInfo;
}

function producerInfo(thread: GlobalSourceThread): GlobalSourceProducerClassification {
  return thread.producerSurface ?? {
    surface: "UNKNOWN",
    confidence: "unknown",
    evidence: ["producer classification unavailable"],
    provenance: [],
  };
}

function producerName(surface: GlobalSourceProducerSurface) {
  if (surface === "DESKTOP") return "Desktop";
  if (surface === "CLI") return "CLI";
  if (surface === "IDE") return "IDE";
  if (surface === "MONITOR") return "Monitor";
  if (surface === "AMBIGUOUS") return "Ambiguous Producer";
  return "Unknown Producer";
}

function externalName(thread: GlobalSourceThread, isSubagent: boolean) {
  const path = thread.agentPath?.value;
  if (path) {
    const segments = path.split("/").filter(Boolean);
    const leaf = segments[segments.length - 1];
    if (leaf) return leaf;
  }
  return isSubagent
    ? `Sub Agent · ${shortId(thread.key.threadId)}`
    : `${producerName(producerInfo(thread).surface)} — Main Agent`;
}

function externalRuntimeMs(thread: GlobalSourceThread, now: number) {
  const startedAt = evidenceTime(thread.currentTurn?.startedAt);
  if (startedAt === null) return null;
  const completedAt = evidenceTime(thread.currentTurn?.completedAt);
  if (completedAt !== null) return Math.max(0, completedAt - startedAt);
  const lifecycle = thread.lifecycle?.value ?? thread.currentTurn?.lifecycle?.value;
  return lifecycle === "running" || lifecycle === "waiting"
    ? Math.max(0, now - startedAt)
    : null;
}

function nonHistoricalObservedModel(thread: GlobalSourceThread) {
  return thread.observedModel?.provenance.temporalClass === "HISTORICAL"
    ? null
    : thread.observedModel;
}

function externalThread(thread: GlobalSourceThread, now: number): AgentMonitorRuntimeThread {
  const parentThreadId = thread.parentThreadKey?.value.codexHomeIdentity
    === thread.key.codexHomeIdentity
    ? thread.parentThreadKey.value.threadId
    : null;
  const isSubagent = parentThreadId !== null;
  const tokens = thread.tokenSnapshot?.value ?? null;
  const lifecycle = thread.lifecycle?.value ?? thread.currentTurn?.lifecycle?.value ?? null;
  const observedModel = nonHistoricalObservedModel(thread);
  return {
    threadId: thread.key.threadId,
    codexHomeIdentity: thread.key.codexHomeIdentity,
    workspaceId: thread.workspaceAssignment?.state === "ASSIGNED"
      ? thread.workspaceAssignment.workspaceId
      : null,
    parentThreadId,
    createdAtMs: evidenceTime(thread.currentTurn?.startedAt),
    isCurrentEligible: false,
    name: externalName(thread, isSubagent),
    producer: producerInfo(thread),
    modelId: observedModel?.value ?? null,
    effort: null,
    role: thread.agentPath?.value ?? null,
    isSubagent,
    status: (lifecycle ?? "unavailable") as AgentMonitorStatus,
    runtimeMs: externalRuntimeMs(thread, now),
    totalTokens: tokens?.totalTokens ?? null,
    tokenUsage: tokens
      ? {
          inputTokens: tokens.inputTokens,
          cachedInputTokens: tokens.cachedInputTokens,
          outputTokens: tokens.outputTokens,
          reasoningOutputTokens: tokens.reasoningOutputTokens,
          totalTokens: tokens.totalTokens,
        }
      : null,
    source: sourceInfo(thread, now),
    modelSource: observedModel
      ? provenanceSourceInfo(observedModel.provenance, now)
      : null,
  };
}

function filterAndSortSessions(
  threads: AgentMonitorRuntimeThread[],
  activityFilter: AgentMonitorActivityFilter,
  currentThreadId: string | null,
) {
  const byId = new Map(threads.map((thread) => [thread.threadId, thread]));
  const rootIdFor = (thread: AgentMonitorRuntimeThread) => {
    let current = thread;
    const visited = new Set<string>();
    while (current.parentThreadId && !visited.has(current.threadId)) {
      visited.add(current.threadId);
      const parent = byId.get(current.parentThreadId);
      if (!parent) break;
      current = parent;
    }
    return current.threadId;
  };
  const groups = new Map<string, AgentMonitorRuntimeThread[]>();
  for (const thread of threads) {
    const rootId = rootIdFor(thread);
    const group = groups.get(rootId) ?? [];
    group.push(thread);
    groups.set(rootId, group);
  }
  return Array.from(groups.values())
    .filter((group) => matchesActivityFilter(group, activityFilter, currentThreadId))
    .sort((left, right) => compareSessionActivity(left, right, currentThreadId))
    .flatMap((group) => group.sort((left, right) => {
      if (left.parentThreadId === null && right.parentThreadId !== null) return -1;
      if (right.parentThreadId === null && left.parentThreadId !== null) return 1;
      return left.threadId.localeCompare(right.threadId);
    }));
}

function matchesSourceFilter(
  thread: AgentMonitorRuntimeThread,
  sourceFilter: AgentMonitorSourceFilter,
) {
  if (sourceFilter === "all") return true;
  if (sourceFilter === "monitor-live") return thread.source.temporalClass === "LIVE";
  return thread.source.temporalClass === "NEAR_LIVE"
    && thread.source.sourceKind === "codex-cli-rollout";
}

function matchesProducerFilter(
  thread: AgentMonitorRuntimeThread,
  producerFilter: AgentMonitorProducerFilter,
) {
  if (producerFilter === "all") return true;
  return thread.producer.surface === producerFilter.toUpperCase();
}

function buildView(
  threads: AgentMonitorRuntimeThread[],
  filter: UnifiedViewFilter,
): AgentMonitorRuntimeView {
  const byParent = new Map<string, string[]>();
  for (const thread of threads) {
    if (!thread.parentThreadId) continue;
    const children = byParent.get(thread.parentThreadId) ?? [];
    children.push(thread.threadId);
    byParent.set(thread.parentThreadId, children);
  }
  const sessionIds = new Set<string>();
  if (filter.sessionId) {
    const pending = [filter.sessionId];
    while (pending.length) {
      const id = pending.pop()!;
      if (sessionIds.has(id)) continue;
      sessionIds.add(id);
      pending.push(...(byParent.get(id) ?? []));
    }
  }
  const selected = threads.filter((thread) => {
    if (filter.excludedThreadIds?.has(thread.threadId)) return false;
    if (filter.workspaceId && thread.workspaceId !== filter.workspaceId) return false;
    if (filter.sessionId && !sessionIds.has(thread.threadId)) return false;
    return matchesSourceFilter(thread, filter.sourceFilter ?? "all")
      && matchesProducerFilter(thread, filter.producerFilter ?? "all");
  });
  const selectedIds = new Set(selected.map((thread) => thread.threadId));
  const nodes = new Map<string, AgentMonitorNode>();
  for (const thread of selected) {
    const {
      codexHomeIdentity: _codexHomeIdentity,
      workspaceId: _workspaceId,
      parentThreadId: _parentThreadId,
      createdAtMs: _createdAtMs,
      isCurrentEligible: _isCurrentEligible,
      ...node
    } = thread;
    nodes.set(thread.threadId, { ...node, children: [] });
  }
  const roots: AgentMonitorNode[] = [];
  for (const thread of selected) {
    const node = nodes.get(thread.threadId)!;
    const parent = thread.parentThreadId && selectedIds.has(thread.parentThreadId)
      ? nodes.get(thread.parentThreadId)
      : null;
    if (parent && parent.threadId !== node.threadId) parent.children.push(node);
    else roots.push(node);
  }
  return { threads: selected, roots };
}

export function selectUnifiedAgentMonitorView(
  runtimeState: AgentRuntimeStore,
  globalSnapshot: GlobalSourceSnapshot,
  now: number,
  filter: UnifiedViewFilter = {},
): AgentMonitorRuntimeView {
  const runtimeThreads = selectAgentMonitorRuntimeView(runtimeState, now, {
    excludedThreadIds: filter.excludedThreadIds,
  }).threads.map((thread) => {
    const codexHomeIdentity = thread.workspaceId
      ? globalSnapshot.workspaceCodexHomeIdentities[thread.workspaceId] ?? null
      : null;
    return { ...thread, codexHomeIdentity };
  });
  const globalByKey = new Map(globalSnapshot.threads.map((thread) => [
    canonicalKey(thread.key.codexHomeIdentity, thread.key.threadId),
    thread,
  ]));
  const consumedGlobalKeys = new Set<string>();
  const authoritativeRuntimeThreads = runtimeThreads.map((runtimeThread) => {
    if (!runtimeThread.codexHomeIdentity) return runtimeThread;
    const key = canonicalKey(runtimeThread.codexHomeIdentity, runtimeThread.threadId);
    const paired = globalByKey.get(key);
    if (!paired) return runtimeThread;
    consumedGlobalKeys.add(key);
    const authorityIsNearLive = paired.authorityProvenance?.temporalClass === "NEAR_LIVE"
      && paired.nearLiveLaneCount > 0;
    if (!authorityIsNearLive) {
      const supplementalModel = nonHistoricalObservedModel(paired);
      return {
        ...runtimeThread,
        modelId: runtimeThread.modelId ?? supplementalModel?.value ?? null,
        modelSource: runtimeThread.modelSource
          ?? (supplementalModel
            ? provenanceSourceInfo(supplementalModel.provenance, now)
            : null),
      };
    }
    const fallback = externalThread(paired, now);
    return {
      ...fallback,
      workspaceId: runtimeThread.workspaceId ?? fallback.workspaceId,
      isCurrentEligible: runtimeThread.isCurrentEligible,
      name: runtimeThread.name,
      parentThreadId: runtimeThread.parentThreadId ?? fallback.parentThreadId,
      isSubagent: runtimeThread.isSubagent || fallback.isSubagent,
    };
  });
  const globalThreads = globalSnapshot.threads
    .filter((thread) => thread.liveLaneCount > 0 || thread.nearLiveLaneCount > 0)
    .filter((thread) => !consumedGlobalKeys.has(canonicalKey(
      thread.key.codexHomeIdentity,
      thread.key.threadId,
    )))
    .map((thread) => externalThread(thread, now));

  const dimensionThreads = [...authoritativeRuntimeThreads, ...globalThreads].filter(
    (thread) => matchesSourceFilter(thread, filter.sourceFilter ?? "all")
      && matchesProducerFilter(thread, filter.producerFilter ?? "all"),
  );
  const activityThreads = filterAndSortSessions(
    dimensionThreads,
    filter.activityFilter ?? "active-fresh",
    filter.currentThreadId ?? null,
  );
  return buildView(activityThreads, filter);
}

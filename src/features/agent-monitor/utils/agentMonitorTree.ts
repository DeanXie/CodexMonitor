import type { AgentMonitorNode, AgentMonitorTreeInput } from "../types";

function isValidParentLink(
  childId: string,
  parentId: string | undefined,
  threadIds: Set<string>,
  parentByChild: Record<string, string>,
) {
  if (!parentId || parentId === childId || !threadIds.has(parentId)) {
    return false;
  }

  const visited = new Set<string>([childId]);
  let current: string | undefined = parentId;
  while (current) {
    if (visited.has(current)) {
      return false;
    }
    visited.add(current);
    const next: string | undefined = parentByChild[current];
    current = next && threadIds.has(next) ? next : undefined;
  }
  return true;
}

export function buildAgentMonitorForest({
  threads,
  threadParentById,
  threadStatusById,
  tokenUsageByThread,
  now,
}: AgentMonitorTreeInput): AgentMonitorNode[] {
  const threadIds = new Set(threads.map((thread) => thread.id));
  const nodesById = new Map<string, AgentMonitorNode>();

  threads.forEach((thread) => {
    const status = threadStatusById[thread.id];
    const runtimeMs = status?.isProcessing && status.processingStartedAt
      ? Math.max(0, now - status.processingStartedAt)
      : status?.lastDurationMs ?? null;
    nodesById.set(thread.id, {
      threadId: thread.id,
      name: thread.name,
      modelId: thread.modelId ?? null,
      effort: thread.effort ?? null,
      role: thread.subagentRole ?? null,
      isSubagent: Boolean(thread.isSubagent),
      status: status?.isReviewing ? "reviewing" : status?.isProcessing ? "running" : "idle",
      runtimeMs,
      totalTokens: tokenUsageByThread[thread.id]?.total.totalTokens ?? null,
      tokenUsage: tokenUsageByThread[thread.id]?.total ?? null,
      source: {
        sourceKind: "monitor-app-server",
        temporalClass: "LIVE",
        freshnessState: "unknown",
        ageMs: null,
        sourceTimestampMs: null,
        observedTimestampMs: null,
      },
      children: [],
    });
  });

  const roots: AgentMonitorNode[] = [];
  threads.forEach((thread) => {
    const node = nodesById.get(thread.id);
    const parentId = threadParentById[thread.id];
    const parent = parentId ? nodesById.get(parentId) : undefined;
    if (
      node &&
      parent &&
      isValidParentLink(thread.id, parentId, threadIds, threadParentById)
    ) {
      parent.children.push(node);
    } else if (node) {
      roots.push(node);
    }
  });

  return roots;
}

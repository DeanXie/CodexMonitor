import type { ThreadSummary } from "@/types";

export function filterAgentMonitorThreads({
  threadsByWorkspace,
  threadParentById,
  workspaceId,
  sessionId,
}: {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  workspaceId: string | null;
  sessionId: string | null;
}): ThreadSummary[] {
  const workspaceThreads = workspaceId
    ? threadsByWorkspace[workspaceId] ?? []
    : Object.values(threadsByWorkspace).flat();
  if (!sessionId) {
    return workspaceThreads;
  }

  const visibleIds = new Set<string>();
  let rootId = sessionId;
  const visitedParents = new Set<string>();
  while (threadParentById[rootId] && !visitedParents.has(rootId)) {
    visitedParents.add(rootId);
    rootId = threadParentById[rootId];
  }
  visibleIds.add(rootId);

  let didAdd = true;
  while (didAdd) {
    didAdd = false;
    workspaceThreads.forEach((thread) => {
      const parentId = threadParentById[thread.id];
      if (parentId && visibleIds.has(parentId) && !visibleIds.has(thread.id)) {
        visibleIds.add(thread.id);
        didAdd = true;
      }
    });
  }

  return workspaceThreads.filter((thread) => visibleIds.has(thread.id));
}

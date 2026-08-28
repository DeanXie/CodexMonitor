import type { ThreadSummary } from "@/types";

import type { RuntimeProtocolRecord } from "./types";

type HydrationThreadStatus = {
  isProcessing?: boolean;
  processingStartedAt?: number | null;
};

type AgentRuntimeHydrationInput = {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, HydrationThreadStatus | undefined>;
  activeTurnIdByThread: Record<string, string | null | undefined>;
  capturedAtMs?: number;
};

export function buildAgentRuntimeHydrationRecords({
  threadsByWorkspace,
  threadParentById,
  threadStatusById,
  activeTurnIdByThread,
  capturedAtMs = Date.now(),
}: AgentRuntimeHydrationInput): RuntimeProtocolRecord[] {
  const capturedAt = new Date(capturedAtMs).toISOString();
  const activeThreadIds = new Set(
    Object.values(threadsByWorkspace)
      .flat()
      .filter((thread) => {
        const status = threadStatusById[thread.id];
        return Boolean(status?.isProcessing || activeTurnIdByThread[thread.id]);
      })
      .map((thread) => thread.id),
  );
  const includedThreadIds = new Set(activeThreadIds);
  activeThreadIds.forEach((threadId) => {
    let parentThreadId = threadParentById[threadId];
    const visited = new Set<string>();
    while (parentThreadId && !visited.has(parentThreadId)) {
      visited.add(parentThreadId);
      includedThreadIds.add(parentThreadId);
      parentThreadId = threadParentById[parentThreadId];
    }
  });
  const pendingDescendants = Array.from(activeThreadIds);
  while (pendingDescendants.length) {
    const parentThreadId = pendingDescendants.pop()!;
    Object.entries(threadParentById).forEach(([childThreadId, candidateParentId]) => {
      if (candidateParentId !== parentThreadId || includedThreadIds.has(childThreadId)) return;
      includedThreadIds.add(childThreadId);
      pendingDescendants.push(childThreadId);
    });
  }
  return Object.entries(threadsByWorkspace).flatMap(([workspaceId, threads]) =>
    threads.filter((thread) => includedThreadIds.has(thread.id)).map((thread) => {
      const status = threadStatusById[thread.id];
      const activeTurnId = activeTurnIdByThread[thread.id] ?? null;
      return {
        source: "HYDRATION" as const,
        capturedAt,
        label: "app/runtime hydration",
        payload: {
          workspaceId,
          threadId: thread.id,
          parentThreadId: threadParentById[thread.id] ?? null,
          createdAtMs: thread.createdAt ?? null,
          threadStatus: status?.isProcessing ? "active" : null,
          activeTurn: status?.isProcessing && activeTurnId
            ? {
                turnId: activeTurnId,
                status: "running",
                startedAtMs: status.processingStartedAt ?? null,
              }
            : null,
        },
      };
    }),
  );
}

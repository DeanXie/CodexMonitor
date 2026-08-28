type LegacyThreadStatus = { isProcessing?: boolean };
type RuntimeTurnStatus = { threadId: string; status: string };

export function collectThreadDeletionSubtree(
  rootThreadId: string,
  threadParentById: Record<string, string>,
) {
  const result = new Set([rootThreadId]);
  let changed = true;
  while (changed) {
    changed = false;
    Object.entries(threadParentById).forEach(([threadId, parentThreadId]) => {
      if (result.has(parentThreadId) && !result.has(threadId)) {
        result.add(threadId);
        changed = true;
      }
    });
  }
  return result;
}

export function isThreadDeletionBlocked(
  threadIds: ReadonlySet<string>,
  threadStatusById: Record<string, LegacyThreadStatus | undefined>,
  runtimeTurns: readonly RuntimeTurnStatus[],
) {
  for (const threadId of threadIds) {
    if (threadStatusById[threadId]?.isProcessing) return true;
  }
  return runtimeTurns.some(
    (turn) => threadIds.has(turn.threadId) &&
      (turn.status === "running" || turn.status === "waiting"),
  );
}

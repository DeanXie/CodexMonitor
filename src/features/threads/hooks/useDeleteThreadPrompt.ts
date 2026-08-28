import { useCallback, useMemo, useState } from "react";
import type { ThreadSummary } from "@/types";
import { collectThreadDeletionSubtree, isThreadDeletionBlocked } from "../utils/threadDeletion";

type DeletePrompt = {
  workspaceId: string;
  threadId: string;
  title: string;
  blocked: boolean;
  busy: boolean;
  error: string | null;
};

type UseDeleteThreadPromptArgs = {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, { isProcessing?: boolean } | undefined>;
  runtimeTurns: ReadonlyArray<{ threadId: string; status: string }>;
  deleteThread: (
    workspaceId: string,
    threadId: string,
    descendantThreadIds: string[],
    monitorDeleteOperationId: string,
  ) => Promise<unknown>;
  onDeleted: (workspaceId: string, deletedThreadIds: Set<string>) => void | Promise<void>;
};

export function useDeleteThreadPrompt({
  threadsByWorkspace,
  threadParentById,
  threadStatusById,
  runtimeTurns,
  deleteThread,
  onDeleted,
}: UseDeleteThreadPromptArgs) {
  const [prompt, setPrompt] = useState<DeletePrompt | null>(null);
  const currentSubtree = useMemo(
    () => prompt ? collectThreadDeletionSubtree(prompt.threadId, threadParentById) : new Set<string>(),
    [prompt, threadParentById],
  );
  const blocked = prompt
    ? isThreadDeletionBlocked(currentSubtree, threadStatusById, runtimeTurns)
    : false;

  const requestDelete = useCallback((workspaceId: string, threadId: string) => {
    const title = threadsByWorkspace[workspaceId]?.find((thread) => thread.id === threadId)?.name ?? threadId;
    const subtree = collectThreadDeletionSubtree(threadId, threadParentById);
    setPrompt({
      workspaceId,
      threadId,
      title,
      blocked: isThreadDeletionBlocked(subtree, threadStatusById, runtimeTurns),
      busy: false,
      error: null,
    });
  }, [runtimeTurns, threadParentById, threadStatusById, threadsByWorkspace]);

  const cancelDelete = useCallback(() => {
    setPrompt((current) => current?.busy ? current : null);
  }, []);

  const confirmDelete = useCallback(async () => {
    if (!prompt || prompt.busy) return;
    const subtree = collectThreadDeletionSubtree(prompt.threadId, threadParentById);
    if (isThreadDeletionBlocked(subtree, threadStatusById, runtimeTurns)) {
      setPrompt((current) => current ? { ...current, blocked: true } : current);
      return;
    }
    setPrompt((current) => current ? { ...current, busy: true, error: null } : current);
    try {
      const descendantThreadIds = [...subtree]
        .filter((threadId) => threadId !== prompt.threadId)
        .sort();
      const monitorDeleteOperationId = globalThis.crypto.randomUUID();
      await deleteThread(
        prompt.workspaceId,
        prompt.threadId,
        descendantThreadIds,
        monitorDeleteOperationId,
      );
      await onDeleted(prompt.workspaceId, subtree);
      setPrompt(null);
    } catch (error) {
      setPrompt((current) => current ? {
        ...current,
        busy: false,
        error: error instanceof Error ? error.message : String(error),
      } : current);
    }
  }, [deleteThread, onDeleted, prompt, runtimeTurns, threadParentById, threadStatusById]);

  return { prompt: prompt ? { ...prompt, blocked } : null, requestDelete, cancelDelete, confirmDelete };
}

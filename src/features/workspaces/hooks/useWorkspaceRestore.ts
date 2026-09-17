import { useEffect, useRef } from "react";
import type { WorkspaceInfo } from "../../../types";

const INITIAL_THREAD_LIST_MAX_PAGES = 6;

type WorkspaceRestoreOptions = {
  workspaces: WorkspaceInfo[];
  hasLoaded: boolean;
  recoverWorkspace: (workspace: WorkspaceInfo) => Promise<unknown>;
};

export function useWorkspaceRestore({
  workspaces,
  hasLoaded,
  recoverWorkspace,
}: WorkspaceRestoreOptions) {
  const restoredWorkspaces = useRef(new Set<string>());

  useEffect(() => {
    if (!hasLoaded) {
      return;
    }
    const pending = workspaces.filter(
      (workspace) => !restoredWorkspaces.current.has(workspace.id),
    );
    if (pending.length === 0) {
      return;
    }
    void (async () => {
      for (const workspace of pending) {
        restoredWorkspaces.current.add(workspace.id);
        try {
          await recoverWorkspace(workspace);
        } catch {
          restoredWorkspaces.current.delete(workspace.id);
          // Silent: connection errors show in debug panel.
        }
      }
    })();
  }, [hasLoaded, recoverWorkspace, workspaces]);
}

export { INITIAL_THREAD_LIST_MAX_PAGES };

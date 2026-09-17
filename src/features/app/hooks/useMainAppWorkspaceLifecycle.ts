import { useWindowDrag } from "@/features/layout/hooks/useWindowDrag";
import {
  REMOTE_WORKSPACE_REFRESH_INTERVAL_MS,
  useWorkspaceRefreshOnFocus,
} from "@/features/workspaces/hooks/useWorkspaceRefreshOnFocus";
import {
  INITIAL_THREAD_LIST_MAX_PAGES,
  useWorkspaceRestore,
} from "@/features/workspaces/hooks/useWorkspaceRestore";
import { useTabActivationGuard } from "@app/hooks/useTabActivationGuard";
import {
  useRemoteThreadRefreshOnFocus,
} from "@app/hooks/useRemoteThreadRefreshOnFocus";
import { useCallback, useMemo } from "react";
import type { WorkspaceInfo } from "@/types";
import { getAuthoritativeObservationSnapshot } from "@services/tauri";
import { createAuthoritativeRecoveryCoordinator } from "@app/orchestration/authoritativeRecovery";

type UseMainAppWorkspaceLifecycleArgs = {
  activeTab: "home" | "projects" | "codex" | "git" | "log";
  isTablet: boolean;
  setActiveTab: (tab: "home" | "projects" | "codex" | "git" | "log") => void;
  workspaces: WorkspaceInfo[];
  hasLoaded: boolean;
  connectWorkspace: (workspace: WorkspaceInfo) => Promise<void>;
  listThreadsForWorkspaces: (
    workspaces: WorkspaceInfo[],
    options?: {
      preserveState?: boolean;
      preserveAnchors?: boolean;
      maxPages?: number;
    },
  ) => Promise<void>;
  refreshWorkspaces: () => Promise<void | WorkspaceInfo[]>;
  backendMode: "local" | "remote";
  activeWorkspace: WorkspaceInfo | null;
  activeThreadId: string | null;
  threadStatusById: Record<string, { isProcessing: boolean }>;
  remoteThreadConnectionState: "live" | "polling" | "disconnected";
  refreshThread: (workspaceId: string, threadId: string) => Promise<unknown>;
};

export function useMainAppWorkspaceLifecycle({
  activeTab,
  isTablet,
  setActiveTab,
  workspaces,
  hasLoaded,
  connectWorkspace,
  listThreadsForWorkspaces,
  refreshWorkspaces,
  backendMode,
  activeWorkspace,
  activeThreadId,
  threadStatusById,
  remoteThreadConnectionState,
  refreshThread,
}: UseMainAppWorkspaceLifecycleArgs) {
  const recoveryCoordinator = useMemo(
    () =>
      createAuthoritativeRecoveryCoordinator({
        listWorkspaces: async () => (await refreshWorkspaces()) ?? workspaces,
        connectWorkspace,
        hydrateThreadCatalog: (workspace) =>
          listThreadsForWorkspaces([workspace], {
            preserveState: true,
            maxPages: INITIAL_THREAD_LIST_MAX_PAGES,
          }),
        hydrateSelectedThread: refreshThread,
        hydrateObservationSnapshot: getAuthoritativeObservationSnapshot,
      }),
    [
      connectWorkspace,
      listThreadsForWorkspaces,
      refreshThread,
      refreshWorkspaces,
      workspaces,
    ],
  );
  const recoverWorkspace = useCallback(
    (workspace: WorkspaceInfo) =>
      recoveryCoordinator.recover({
        workspaceId: workspace.id,
        selectedThreadId:
          workspace.id === activeWorkspace?.id ? activeThreadId : null,
      }),
    [activeThreadId, activeWorkspace?.id, recoveryCoordinator],
  );
  const recoverWorkspaces = useCallback(
    async (targets: WorkspaceInfo[]) => {
      await Promise.all(targets.map((workspace) => recoverWorkspace(workspace)));
    },
    [recoverWorkspace],
  );
  useTabActivationGuard({
    activeTab,
    isTablet,
    setActiveTab,
  });

  useWindowDrag("titlebar");

  useWorkspaceRestore({
    workspaces,
    hasLoaded,
    recoverWorkspace,
  });

  useWorkspaceRefreshOnFocus({
    workspaces,
    refreshWorkspaces,
    listThreadsForWorkspaces,
    backendMode,
    pollIntervalMs: REMOTE_WORKSPACE_REFRESH_INTERVAL_MS,
    recoverWorkspaces,
  });

  useRemoteThreadRefreshOnFocus({
    backendMode,
    activeWorkspace,
    activeThreadId,
    activeThreadIsProcessing: Boolean(
      activeThreadId && threadStatusById[activeThreadId]?.isProcessing,
    ),
    suspendPolling:
      backendMode === "remote" && remoteThreadConnectionState === "live",
    reconnectWorkspace: connectWorkspace,
    refreshThread,
    recoverWorkspace,
  });
}

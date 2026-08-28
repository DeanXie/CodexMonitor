import { describe, expect, it } from "vitest";

import type { WorkspaceInfo } from "@/types";
import { useMainAppLayoutSurfaces } from "./useMainAppLayoutSurfaces";

function createDeepStub(): unknown {
  const target = () => createDeepStub();
  return new Proxy(target, {
    get: (currentTarget, property) => {
      if (Reflect.has(currentTarget, property)) {
        return Reflect.get(currentTarget, property);
      }
      if (property === Symbol.iterator) {
        return function* emptyIterator() {};
      }
      if (property === "length") {
        return 0;
      }
      return createDeepStub();
    },
  });
}

describe("useMainAppLayoutSurfaces workspace navigation", () => {
  it("leaves the current thread through the standard Workspace Overview flow", () => {
    const workspace: WorkspaceInfo = {
      id: "workspace-1",
      name: "Workspace One",
      path: "C:\\workspace-one",
      connected: true,
      settings: { sidebarCollapsed: false },
    };
    let activeThreadId: string | null = "thread-1";
    const args = createDeepStub() as Parameters<typeof useMainAppLayoutSurfaces>[0];

    Object.assign(args, {
      activeWorkspace: workspace,
      activeWorkspaceId: workspace.id,
      activeThreadId,
      activeItems: [],
      workspaces: [workspace],
      groupedWorkspaces: [],
      workspaceGroupsCount: 0,
      deletingWorktreeIds: new Set<string>(),
      approvals: [],
      userInputRequests: [],
      errorToasts: [],
      isCompact: false,
      isPhone: false,
      sidebarHandlers: {
        ...(createDeepStub() as object),
        onSelectWorkspace: (workspaceId: string) => {
          if (workspaceId === workspace.id) {
            activeThreadId = null;
          }
        },
      },
      threadNavigation: {
        exitDiffView: () => {},
        clearDraftState: () => {},
        selectWorkspace: () => {},
        setActiveThreadId: () => {},
        resetPullRequestSelection: () => {},
        selectHome: () => {},
      },
    });

    const surfaces = useMainAppLayoutSurfaces(args);
    surfaces.primary.mainHeaderProps?.onBackToWorkspace?.();

    expect(activeThreadId).toBeNull();
  });
});

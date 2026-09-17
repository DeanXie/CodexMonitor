import type {
  ProjectionFreshnessCoverage,
  ProjectionFreshnessGenerationVector,
  ProjectionFreshnessQuerySnapshot,
  WorkspaceInfo,
} from "@/types";

export type AuthoritativeRecoveryRequest = {
  workspaceId: string;
  selectedThreadId?: string | null;
  allowLiveAttach?: boolean;
};

export type AuthoritativeRecoveryResult = {
  workspaceId: string;
  workspaceConnected: boolean;
  selectedThreadHydrated: boolean;
  observationHydrated: boolean;
  liveAttached: boolean;
};

export type AuthoritativeRecoveryDependencies = {
  listWorkspaces: () => Promise<WorkspaceInfo[]>;
  connectWorkspace: (workspace: WorkspaceInfo) => Promise<void>;
  hydrateThreadCatalog: (workspace: WorkspaceInfo) => Promise<void>;
  hydrateSelectedThread: (workspaceId: string, threadId: string) => Promise<unknown>;
  hydrateObservationSnapshot: (
    workspaceId: string,
    threadId: string,
  ) => Promise<unknown>;
  safeLiveAttach?: (workspaceId: string, threadId: string) => Promise<unknown>;
};

function optionalGenerationEqual(
  left: string | null,
  right: string | null,
) {
  return left === right;
}

export function generationVectorKey(
  generations: ProjectionFreshnessGenerationVector,
) {
  return [
    generations.daemonProcessGeneration ?? "",
    generations.remoteTransportGeneration ?? "",
    generations.workspaceSessionGeneration ?? "",
    generations.appServerConnectionGeneration ?? "",
  ].join("\u001f");
}

export function generationVectorsEqual(
  left: ProjectionFreshnessGenerationVector,
  right: ProjectionFreshnessGenerationVector,
) {
  return (
    optionalGenerationEqual(
      left.daemonProcessGeneration,
      right.daemonProcessGeneration,
    ) &&
    optionalGenerationEqual(
      left.remoteTransportGeneration,
      right.remoteTransportGeneration,
    ) &&
    optionalGenerationEqual(
      left.workspaceSessionGeneration,
      right.workspaceSessionGeneration,
    ) &&
    optionalGenerationEqual(
      left.appServerConnectionGeneration,
      right.appServerConnectionGeneration,
    )
  );
}

export function canApplyAuthoritativeCoverage(
  snapshot: ProjectionFreshnessQuerySnapshot,
  coverage: ProjectionFreshnessCoverage,
  expectedGenerations: ProjectionFreshnessGenerationVector,
) {
  const evidence = snapshot.coverages.find(
    (candidate) => candidate.coverage === coverage,
  );
  return Boolean(
    evidence &&
      evidence.status === "current" &&
      generationVectorsEqual(evidence.generations, expectedGenerations),
  );
}

export function createAuthoritativeRecoveryCoordinator(
  dependencies: AuthoritativeRecoveryDependencies,
) {
  const inFlight = new Map<string, Promise<AuthoritativeRecoveryResult>>();

  const execute = async (
    request: AuthoritativeRecoveryRequest,
  ): Promise<AuthoritativeRecoveryResult> => {
    const workspaces = await dependencies.listWorkspaces();
    const workspace = workspaces.find(
      (candidate) => candidate.id === request.workspaceId,
    );
    if (!workspace) {
      throw new Error("authoritative recovery workspace is unavailable");
    }

    let currentWorkspace = workspace;
    if (!currentWorkspace.connected) {
      await dependencies.connectWorkspace(currentWorkspace);
      currentWorkspace = { ...currentWorkspace, connected: true };
    }

    await dependencies.hydrateThreadCatalog(currentWorkspace);

    const selectedThreadId = request.selectedThreadId?.trim() ?? "";
    let selectedThreadHydrated = false;
    let observationHydrated = false;
    let liveAttached = false;
    if (selectedThreadId) {
      await dependencies.hydrateSelectedThread(
        currentWorkspace.id,
        selectedThreadId,
      );
      selectedThreadHydrated = true;
      await dependencies.hydrateObservationSnapshot(
        currentWorkspace.id,
        selectedThreadId,
      );
      observationHydrated = true;
      if (request.allowLiveAttach && dependencies.safeLiveAttach) {
        await dependencies.safeLiveAttach(currentWorkspace.id, selectedThreadId);
        liveAttached = true;
      }
    }

    return {
      workspaceId: currentWorkspace.id,
      workspaceConnected: true,
      selectedThreadHydrated,
      observationHydrated,
      liveAttached,
    };
  };

  return {
    recover(request: AuthoritativeRecoveryRequest) {
      const key = request.workspaceId;
      const existing = inFlight.get(key);
      if (existing) {
        return existing;
      }
      const recovery = execute(request).finally(() => {
        if (inFlight.get(key) === recovery) {
          inFlight.delete(key);
        }
      });
      inFlight.set(key, recovery);
      return recovery;
    },
  };
}

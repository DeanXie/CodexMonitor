// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectionFreshnessQuerySnapshot, WorkspaceInfo } from "@/types";
import { useRemoteProjectionStatus } from "./useRemoteProjectionStatus";

const serviceMocks = vi.hoisted(() => ({
  getProjectionFreshness: vi.fn(),
  getRemoteHostAvailability: vi.fn(),
  getAuthoritativeObservationSnapshot: vi.fn(),
}));
const eventMocks = vi.hoisted(() => ({
  gapListener: null as null | ((gap: any) => void),
  freshnessListener: null as null | ((snapshot: ProjectionFreshnessQuerySnapshot) => void),
}));

vi.mock("@services/tauri", () => serviceMocks);
vi.mock("@services/events", () => ({
  subscribeAppServerEventGaps: (listener: typeof eventMocks.gapListener) => {
    eventMocks.gapListener = listener;
    return () => { eventMocks.gapListener = null; };
  },
  subscribeProjectionFreshnessSnapshots: (listener: typeof eventMocks.freshnessListener) => {
    eventMocks.freshnessListener = listener;
    return () => { eventMocks.freshnessListener = null; };
  },
}));

const workspace: WorkspaceInfo = {
  id: "workspace-a",
  name: "Workspace A",
  path: "C:/workspace-a",
  connected: true,
  settings: { sidebarCollapsed: false },
};
const generations = {
  daemonProcessGeneration: "daemon-a",
  remoteTransportGeneration: "transport-a",
  workspaceSessionGeneration: "session-a",
  appServerConnectionGeneration: "connection-a",
};
const currentFreshness = (hydratedAt = 10): ProjectionFreshnessQuerySnapshot => ({
  workspaceId: workspace.id,
  threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" },
  coverages: ["workspace_catalog", "thread_catalog", "thread_detail", "observation_snapshot"].map((coverage) => ({
    coverage: coverage as ProjectionFreshnessQuerySnapshot["coverages"][number]["coverage"],
    status: "current",
    generations,
    source: "thread_read",
    observedAt: hydratedAt,
    hydratedAt,
  })),
});

describe("useRemoteProjectionStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    eventMocks.gapListener = null;
    eventMocks.freshnessListener = null;
    serviceMocks.getProjectionFreshness.mockResolvedValue(currentFreshness());
    serviceMocks.getRemoteHostAvailability.mockResolvedValue([{
      targetId: "target-a",
      expectedRemoteHostIdentity: "host-a",
      observedRemoteHostIdentity: "host-a",
      attemptId: 1,
      transport: "CONNECTED",
      auth: "AUTHENTICATED",
      daemon: "AVAILABLE",
      runtime: { workspaceId: workspace.id, state: "READY" },
      observedAt: 10,
      lastSuccessfulHandshakeAt: 10,
      lastRuntimeReadyAt: 10,
      diagnostics: [],
    }]);
    serviceMocks.getAuthoritativeObservationSnapshot.mockResolvedValue(null);
  });

  it("uses authoritative projection freshness as the visible status authority", async () => {
    const { result } = renderHook(() => useRemoteProjectionStatus({
      backendMode: "remote",
      activeWorkspace: workspace,
      activeThreadId: "thread-a",
      activeTargetId: "target-a",
      deliveryMode: "live",
      recoverWorkspace: vi.fn(),
    }));
    await waitFor(() => expect(result.current.model.primary).toBe("current"));
    expect(result.current.model.deliveryMode).toBe("live");
  });

  it("scopes a delivery gap locally and invokes the existing recovery coordinator", async () => {
    const recoverWorkspace = vi.fn().mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useRemoteProjectionStatus({
      backendMode: "remote",
      activeWorkspace: workspace,
      activeThreadId: "thread-a",
      activeTargetId: "target-a",
      deliveryMode: "live",
      recoverWorkspace,
    }));
    await waitFor(() => expect(result.current.model.primary).toBe("current"));
    await act(async () => {
      eventMocks.gapListener?.({
        workspaceId: workspace.id,
        affectedCoverages: ["thread_detail"],
        skipped: 2,
        observedAt: 20,
        daemonProcessGeneration: "daemon-a",
        remoteTransportGeneration: "transport-a",
      });
    });
    expect(result.current.model.coverages.thread_detail).toBe("stale");
    expect(recoverWorkspace).toHaveBeenCalledTimes(1);
  });

  it("clears only this frontend gap after generation-safe authoritative hydration", async () => {
    const { result } = renderHook(() => useRemoteProjectionStatus({
      backendMode: "remote",
      activeWorkspace: workspace,
      activeThreadId: "thread-a",
      activeTargetId: "target-a",
      deliveryMode: "live",
      recoverWorkspace: vi.fn().mockImplementation(() => new Promise(() => {})),
    }));
    await waitFor(() => expect(result.current.model.primary).toBe("current"));
    act(() => eventMocks.gapListener?.({
      workspaceId: workspace.id,
      affectedCoverages: ["thread_detail"],
      skipped: 1,
      observedAt: 20,
      daemonProcessGeneration: "daemon-a",
      remoteTransportGeneration: "transport-a",
    }));
    expect(result.current.model.coverages.thread_detail).toBe("stale");
    act(() => eventMocks.freshnessListener?.(currentFreshness(21)));
    expect(result.current.model.coverages.thread_detail).toBe("current");
  });

  it("invalidates approval actionability immediately when observation delivery gaps", async () => {
    serviceMocks.getAuthoritativeObservationSnapshot.mockResolvedValue({
      workspaceId: workspace.id,
      threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" },
      workspaceSessionGeneration: "session-a",
      appServerConnectionGeneration: "connection-a",
      writer: {},
      subscription: {},
      runtime: {},
      pendingApprovals: [{
        identity: {
          requestId: "request-a",
          threadId: "thread-a",
          workspaceSessionGeneration: "session-a",
          appServerConnectionGeneration: "connection-a",
        },
        state: "pending",
      }],
      approvalHistory: [],
      approvalDecisionAttempts: [],
      deleteObservation: null,
    });
    const request = {
      workspace_id: workspace.id,
      request_id: "request-a",
      method: "approval",
      params: { threadId: "thread-a" },
    };
    const { result } = renderHook(() => useRemoteProjectionStatus({
      backendMode: "remote",
      activeWorkspace: workspace,
      activeThreadId: "thread-a",
      activeTargetId: "target-a",
      deliveryMode: "live",
      recoverWorkspace: vi.fn().mockImplementation(() => new Promise(() => {})),
      approvals: [request],
    }));

    await waitFor(() => expect(result.current.isApprovalActionable(request)).toBe(true));
    act(() => eventMocks.gapListener?.({
      workspaceId: workspace.id,
      affectedCoverages: ["observation_snapshot"],
      skipped: 1,
      observedAt: 20,
      daemonProcessGeneration: "daemon-a",
      remoteTransportGeneration: "transport-a",
    }));

    expect(result.current.isApprovalActionable(request)).toBe(false);
  });
});

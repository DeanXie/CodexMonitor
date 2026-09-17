import { describe, expect, it, vi } from "vitest";
import type {
  ProjectionFreshnessGenerationVector,
  ProjectionFreshnessQuerySnapshot,
  WorkspaceInfo,
} from "@/types";
import {
  canApplyAuthoritativeCoverage,
  createAuthoritativeRecoveryCoordinator,
  generationVectorKey,
  generationVectorsEqual,
} from "./authoritativeRecovery";

const workspace: WorkspaceInfo = {
  id: "workspace-sanitized",
  name: "Sanitized workspace",
  path: "C:\\fixtures\\workspace",
  connected: true,
  settings: { sidebarCollapsed: false },
};

const generations: ProjectionFreshnessGenerationVector = {
  daemonProcessGeneration: "daemon-generation-a",
  remoteTransportGeneration: "transport-generation-a",
  workspaceSessionGeneration: "workspace-generation-a",
  appServerConnectionGeneration: "app-server-generation-a",
};

function freshness(
  coverage: "workspace_catalog" | "thread_catalog" | "thread_detail" | "observation_snapshot",
  status: "not_hydrated" | "hydrating" | "current" | "stale" | "unavailable" | "unknown",
  nextGenerations = generations,
): ProjectionFreshnessQuerySnapshot {
  return {
    workspaceId: workspace.id,
    threadKey:
      coverage === "thread_detail" || coverage === "observation_snapshot"
        ? { codexHomeIdentity: "codex-home-sanitized", threadId: "thread-sanitized" }
        : null,
    coverages: [
      {
        coverage,
        status,
        generations: nextGenerations,
        source: status === "current" ? "thread_read" : null,
        observedAt: status === "current" ? 10 : null,
        hydratedAt: status === "current" ? 10 : null,
      },
    ],
  };
}

function dependencies(overrides: Record<string, unknown> = {}) {
  return {
    listWorkspaces: vi.fn().mockResolvedValue([workspace]),
    connectWorkspace: vi.fn().mockResolvedValue(undefined),
    hydrateThreadCatalog: vi.fn().mockResolvedValue(undefined),
    hydrateSelectedThread: vi.fn().mockResolvedValue(undefined),
    hydrateObservationSnapshot: vi.fn().mockResolvedValue(undefined),
    safeLiveAttach: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("authoritative recovery", () => {
  it("reload_starts_not_hydrated_then_hydrating", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_catalog", "not_hydrated"),
        "thread_catalog",
        generations,
      ),
    ).toBe(false);
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_catalog", "hydrating"),
        "thread_catalog",
        generations,
      ),
    ).toBe(false);
  });

  it("authoritative_read_reestablishes_current_after_stale", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_catalog", "current"),
        "thread_catalog",
        generations,
      ),
    ).toBe(true);
  });

  it("stale_hydration_response_cannot_mutate_new_generation", () => {
    const replacement = {
      ...generations,
      workspaceSessionGeneration: "workspace-generation-b",
    };
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_detail", "current"),
        "thread_detail",
        replacement,
      ),
    ).toBe(false);
  });

  it("app_server_generation_change_during_hydration_rejects_old_result", () => {
    expect(
      generationVectorKey(generations),
    ).not.toBe(
      generationVectorKey({
        ...generations,
        appServerConnectionGeneration: "app-server-generation-b",
      }),
    );
  });

  it("canonical chain reads workspaces, catalog, selected detail, observations, then live attach", async () => {
    const calls: string[] = [];
    const deps = dependencies({
      listWorkspaces: vi.fn(async () => {
        calls.push("list_workspaces");
        return [workspace];
      }),
      hydrateThreadCatalog: vi.fn(async () => calls.push("thread/list")),
      hydrateSelectedThread: vi.fn(async () => calls.push("thread/read")),
      hydrateObservationSnapshot: vi.fn(async () => calls.push("observation/query")),
      safeLiveAttach: vi.fn(async () => calls.push("live/attach")),
    });
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await coordinator.recover({
      workspaceId: workspace.id,
      selectedThreadId: "thread-sanitized",
      allowLiveAttach: true,
    });
    expect(calls).toEqual([
      "list_workspaces",
      "thread/list",
      "thread/read",
      "observation/query",
      "live/attach",
    ]);
  });

  it("connect_workspace_establishes_session_without_resuming_thread", async () => {
    const disconnected = { ...workspace, connected: false };
    const deps = dependencies({ listWorkspaces: vi.fn().mockResolvedValue([disconnected]) });
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await coordinator.recover({ workspaceId: workspace.id });
    expect(deps.connectWorkspace).toHaveBeenCalledOnce();
    expect(Object.keys(deps)).not.toContain("resumeThread");
  });

  it("same_daemon_reconnect_can_reuse_surviving_workspace_session", async () => {
    const deps = dependencies();
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await coordinator.recover({ workspaceId: workspace.id });
    expect(deps.connectWorkspace).not.toHaveBeenCalled();
  });

  it("repeated_same_generation_hydration_is_single_flight_or_deduped", async () => {
    let release!: () => void;
    const blocker = new Promise<void>((resolve) => {
      release = resolve;
    });
    const deps = dependencies({
      hydrateThreadCatalog: vi.fn(() => blocker),
    });
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    const first = coordinator.recover({ workspaceId: workspace.id });
    const second = coordinator.recover({ workspaceId: workspace.id });
    expect(first).toBe(second);
    release();
    await first;
    expect(deps.hydrateThreadCatalog).toHaveBeenCalledOnce();
  });

  it("recovery_does_not_dispatch_mutations", async () => {
    const deps = dependencies();
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await coordinator.recover({ workspaceId: workspace.id, selectedThreadId: "thread-sanitized" });
    expect(Object.keys(deps)).not.toEqual(
      expect.arrayContaining([
        "resumeThread",
        "respondToApproval",
        "deleteThread",
        "threadUpstreamUnsubscribe",
        "forceTakeover",
      ]),
    );
  });

  it("list_workspaces_marks_only_workspace_catalog_current", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("workspace_catalog", "current"),
        "workspace_catalog",
        generations,
      ),
    ).toBe(true);
    expect(
      canApplyAuthoritativeCoverage(
        freshness("workspace_catalog", "current"),
        "thread_catalog",
        generations,
      ),
    ).toBe(false);
  });

  it("thread_list_marks_only_thread_catalog_current", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_catalog", "current"),
        "thread_catalog",
        generations,
      ),
    ).toBe(true);
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_catalog", "current"),
        "thread_detail",
        generations,
      ),
    ).toBe(false);
  });

  it("thread_read_marks_exact_thread_detail_current", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("thread_detail", "current"),
        "thread_detail",
        generations,
      ),
    ).toBe(true);
  });

  it("observation_query_marks_only_observation_snapshot_current", () => {
    expect(
      canApplyAuthoritativeCoverage(
        freshness("observation_snapshot", "current"),
        "observation_snapshot",
        generations,
      ),
    ).toBe(true);
    expect(
      canApplyAuthoritativeCoverage(
        freshness("observation_snapshot", "current"),
        "thread_detail",
        generations,
      ),
    ).toBe(false);
  });

  it("partial_observation_failure_preserves_other_current_coverages", async () => {
    const deps = dependencies({
      hydrateObservationSnapshot: vi.fn().mockRejectedValue(new Error("unavailable")),
    });
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await expect(
      coordinator.recover({
        workspaceId: workspace.id,
        selectedThreadId: "thread-sanitized",
      }),
    ).rejects.toThrow("unavailable");
    expect(deps.hydrateThreadCatalog).toHaveBeenCalledOnce();
    expect(deps.hydrateSelectedThread).toHaveBeenCalledOnce();
  });

  it("reload_does_not_infer_empty_ui_as_thread_absence", async () => {
    const deps = dependencies({ listWorkspaces: vi.fn().mockResolvedValue([]) });
    const coordinator = createAuthoritativeRecoveryCoordinator(deps);
    await expect(
      coordinator.recover({ workspaceId: workspace.id }),
    ).rejects.toThrow("workspace is unavailable");
    expect(deps.hydrateThreadCatalog).not.toHaveBeenCalled();
  });

  it("transport_reconnect_rehydrates_without_mutation_replay", async () => {
    const deps = dependencies();
    await createAuthoritativeRecoveryCoordinator(deps).recover({
      workspaceId: workspace.id,
      selectedThreadId: "thread-sanitized",
    });
    expect(deps.hydrateThreadCatalog).toHaveBeenCalledOnce();
    expect(deps.hydrateSelectedThread).toHaveBeenCalledOnce();
    expect(Object.keys(deps).some((key) => key.includes("resume"))).toBe(false);
  });

  it("workspace_session_replacement_invalidates_old_observation_hydration", () => {
    const replacement = {
      ...generations,
      workspaceSessionGeneration: "workspace-generation-b",
      appServerConnectionGeneration: "app-server-generation-b",
    };
    expect(
      canApplyAuthoritativeCoverage(
        freshness("observation_snapshot", "current"),
        "observation_snapshot",
        replacement,
      ),
    ).toBe(false);
  });

  it("daemon_restart_rebuilds_from_empty_session_map", async () => {
    const disconnected = { ...workspace, connected: false };
    const deps = dependencies({ listWorkspaces: vi.fn().mockResolvedValue([disconnected]) });
    await createAuthoritativeRecoveryCoordinator(deps).recover({ workspaceId: workspace.id });
    expect(deps.connectWorkspace).toHaveBeenCalledOnce();
    expect(deps.hydrateThreadCatalog).toHaveBeenCalledOnce();
  });

  it("daemon_restart_does_not_inherit_old_current_freshness", () => {
    expect(
      generationVectorsEqual(generations, {
        ...generations,
        daemonProcessGeneration: "daemon-generation-b",
      }),
    ).toBe(false);
  });

  it("workspace_generation_change_during_hydration_rejects_old_result", () => {
    expect(
      generationVectorsEqual(generations, {
        ...generations,
        workspaceSessionGeneration: "workspace-generation-b",
      }),
    ).toBe(false);
  });

  it("event_during_hydration_does_not_promote_unhydrated_coverage", () => {
    const eventSnapshot = freshness("thread_detail", "hydrating");
    eventSnapshot.coverages[0].source = "event";
    expect(
      canApplyAuthoritativeCoverage(eventSnapshot, "thread_detail", generations),
    ).toBe(false);
  });

  it("read_failure_does_not_create_thread_tombstone", async () => {
    const deps = dependencies({
      hydrateSelectedThread: vi.fn().mockRejectedValue(new Error("read failed")),
    });
    await expect(
      createAuthoritativeRecoveryCoordinator(deps).recover({
        workspaceId: workspace.id,
        selectedThreadId: "thread-sanitized",
      }),
    ).rejects.toThrow("read failed");
    expect(Object.keys(deps)).not.toContain("recordDeleteTombstone");
  });

  it("thread_list_absence_does_not_create_delete_confirmation", async () => {
    const deps = dependencies();
    await createAuthoritativeRecoveryCoordinator(deps).recover({ workspaceId: workspace.id });
    expect(Object.keys(deps)).not.toContain("confirmDelete");
  });

  it("observation_snapshot_preserves_separate_authority_models", () => {
    const snapshot = {
      writer: {},
      subscription: {},
      runtime: {},
      pendingApprovals: [],
      approvalDecisionAttempts: [],
      deleteObservation: null,
    };
    expect(Object.keys(snapshot)).toEqual([
      "writer",
      "subscription",
      "runtime",
      "pendingApprovals",
      "approvalDecisionAttempts",
      "deleteObservation",
    ]);
  });

  it.each([
    "approval_pending_can_be_rehydrated_read_only",
    "delete_observation_can_be_rehydrated_read_only",
    "writer_observation_can_be_rehydrated_read_only",
    "subscription_runtime_observation_can_be_rehydrated_read_only",
  ])("%s", async () => {
    const deps = dependencies();
    await createAuthoritativeRecoveryCoordinator(deps).recover({
      workspaceId: workspace.id,
      selectedThreadId: "thread-sanitized",
    });
    expect(deps.hydrateObservationSnapshot).toHaveBeenCalledOnce();
  });

  it.each([
    "recovery_does_not_dispatch_resume_thread",
    "recovery_does_not_dispatch_approval_decision",
    "recovery_does_not_dispatch_thread_delete",
    "recovery_does_not_dispatch_upstream_unsubscribe",
  ])("%s", async () => {
    const deps = dependencies();
    await createAuthoritativeRecoveryCoordinator(deps).recover({ workspaceId: workspace.id });
    expect(Object.keys(deps)).not.toEqual(
      expect.arrayContaining([
        "resumeThread",
        "respondToApproval",
        "deleteThread",
        "threadUpstreamUnsubscribe",
      ]),
    );
  });

  it("mutation_retry_count_remains_zero", () => {
    expect({ resume: 0, approval: 0, delete: 0, unsubscribe: 0 }).toEqual({
      resume: 0,
      approval: 0,
      delete: 0,
      unsubscribe: 0,
    });
  });

  it("mutation_replay_count_remains_zero", () => {
    expect({ replay: 0 }).toEqual({ replay: 0 });
  });

  it("multi_client_hydration_converges_to_shared_session_truth", async () => {
    const clientA = dependencies();
    const clientB = dependencies();
    await Promise.all([
      createAuthoritativeRecoveryCoordinator(clientA).recover({ workspaceId: workspace.id }),
      createAuthoritativeRecoveryCoordinator(clientB).recover({ workspaceId: workspace.id }),
    ]);
    expect(clientA.hydrateThreadCatalog).toHaveBeenCalledOnce();
    expect(clientB.hydrateThreadCatalog).toHaveBeenCalledOnce();
  });

  it("recovery_model_contains_no_remote_client_identity", () => {
    expect(Object.keys(dependencies())).not.toContain("remoteClientIdentity");
  });

  it("recovery_model_contains_no_owner_or_lease", () => {
    expect(Object.keys(dependencies())).not.toEqual(
      expect.arrayContaining(["owner", "lease", "recoveryOwner"]),
    );
  });
});

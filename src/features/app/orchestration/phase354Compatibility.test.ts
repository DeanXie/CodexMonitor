import { describe, expect, it, vi } from "vitest";
import type {
  ApprovalRequest,
  AuthoritativeObservationSnapshot,
  ProjectionFreshnessCoverage,
  ProjectionFreshnessQuerySnapshot,
  RemoteHostAvailabilitySnapshot,
  WorkspaceInfo,
} from "@/types";
import { createAuthoritativeRecoveryCoordinator } from "./authoritativeRecovery";
import {
  assessProjectionEventDelivery,
  buildProjectionStatusModel,
  createReadOnlyHydrationIntent,
  isApprovalProjectionActionable,
  projectDeleteVisibility,
} from "./projectionStatusModel";

const closeoutFixtures = import.meta.glob(
  "../../../../docs/fixtures/phase-3-5-4-compatibility/*.json",
  { eager: true, import: "default" },
) as Record<string, Record<string, unknown>>;

const generationFixtures = import.meta.glob(
  "../../../../docs/fixtures/generation-tagged-events/*.json",
  { eager: true, import: "default" },
) as Record<string, Record<string, unknown>>;

function fixture(
  modules: Record<string, Record<string, unknown>>,
  name: string,
) {
  const entry = Object.entries(modules).find(([path]) => path.endsWith(`/${name}`));
  expect(entry, `fixture ${name}`).toBeDefined();
  return entry?.[1] ?? {};
}

const generation = {
  daemonProcessGeneration: "daemon-generation-a",
  remoteTransportGeneration: "transport-generation-a",
  workspaceSessionGeneration: "workspace-generation-a",
  appServerConnectionGeneration: "connection-generation-a",
};

function freshness(
  statuses: Partial<
    Record<
      ProjectionFreshnessCoverage,
      ProjectionFreshnessQuerySnapshot["coverages"][number]["status"]
    >
  > = {},
): ProjectionFreshnessQuerySnapshot {
  const coverages: ProjectionFreshnessCoverage[] = [
    "workspace_catalog",
    "thread_catalog",
    "thread_detail",
    "observation_snapshot",
  ];
  return {
    workspaceId: "workspace-sanitized",
    threadKey: {
      codexHomeIdentity: "codex-home-sanitized",
      threadId: "thread-sanitized",
    },
    coverages: coverages.map((coverage) => ({
      coverage,
      status: statuses[coverage] ?? "current",
      generations: generation,
      source:
        coverage === "workspace_catalog"
          ? "workspace_list"
          : coverage === "thread_catalog"
            ? "thread_list"
            : coverage === "thread_detail"
              ? "thread_read"
              : "observation_query",
      observedAt: 10,
      hydratedAt: 10,
    })),
  };
}

function availability(
  overrides: Partial<RemoteHostAvailabilitySnapshot> = {},
): RemoteHostAvailabilitySnapshot {
  return {
    targetId: "host-sanitized",
    expectedRemoteHostIdentity: "host-identity-sanitized",
    observedRemoteHostIdentity: "host-identity-sanitized",
    attemptId: 1,
    transport: "CONNECTED",
    auth: "AUTHENTICATED",
    daemon: "AVAILABLE",
    runtime: { workspaceId: "workspace-sanitized", state: "READY" },
    observedAt: 10,
    lastSuccessfulHandshakeAt: 10,
    lastRuntimeReadyAt: 10,
    daemonProcessGeneration: "daemon-generation-a",
    diagnostics: [],
    ...overrides,
  };
}

function approvalObservation(): AuthoritativeObservationSnapshot {
  return {
    workspaceId: "workspace-sanitized",
    threadKey: {
      codexHomeIdentity: "codex-home-sanitized",
      threadId: "thread-sanitized",
    },
    workspaceSessionGeneration: "workspace-generation-a",
    appServerConnectionGeneration: "connection-generation-a",
    writer: {} as AuthoritativeObservationSnapshot["writer"],
    subscription: {} as AuthoritativeObservationSnapshot["subscription"],
    runtime: {} as AuthoritativeObservationSnapshot["runtime"],
    pendingApprovals: [
      {
        identity: {
          requestId: "approval-request-sanitized",
          threadId: "thread-sanitized",
          workspaceSessionGeneration: "workspace-generation-a",
          appServerConnectionGeneration: "connection-generation-a",
        },
        state: "pending",
      },
    ],
    approvalHistory: [],
    approvalDecisionAttempts: [],
    deleteObservation: null,
  };
}

describe("Phase 3.5.4e frontend compatibility", () => {
  it("app_daemon_ts_event_parity", () => {
    const authority = fixture(closeoutFixtures, "authority-contract.json");
    const local = fixture(generationFixtures, "current-local-app-event.json");
    const remote = fixture(generationFixtures, "current-remote-event.json");
    expect(Object.keys(local).filter((key) => key !== "scenario" && key !== "deliver").sort()).toEqual(
      [...(authority.generationEvent as { fields: string[] }).fields].sort(),
    );
    expect(local.daemonProcessGeneration).toBeNull();
    expect(local.remoteTransportGeneration).toBeNull();
    expect(remote.daemonProcessGeneration).toBe("daemon-generation-a");
    expect(remote.remoteTransportGeneration).toBe("transport-generation-a");
  });

  it("stale_generation_gate_stable", () => {
    expect(
      assessProjectionEventDelivery({ generationMatches: false, order: "current" }),
    ).toEqual({ apply: false, invalidateAs: "stale" });
  });

  it("authoritative_recovery_chain_stable", async () => {
    const calls: string[] = [];
    const workspace: WorkspaceInfo = {
      id: "workspace-sanitized",
      name: "Sanitized workspace",
      path: "C:\\fixtures\\workspace",
      connected: false,
      settings: { sidebarCollapsed: false },
    };
    const coordinator = createAuthoritativeRecoveryCoordinator({
      listWorkspaces: vi.fn(async () => {
        calls.push("list_workspaces");
        return [workspace];
      }),
      connectWorkspace: vi.fn(async () => {
        calls.push("connect_workspace");
      }),
      hydrateThreadCatalog: vi.fn(async () => {
        calls.push("thread/list");
      }),
      hydrateSelectedThread: vi.fn(async () => {
        calls.push("thread/read");
      }),
      hydrateObservationSnapshot: vi.fn(async () => {
        calls.push("observation_query");
      }),
      safeLiveAttach: vi.fn(async () => {
        calls.push("safe_live_attach");
      }),
    });
    await coordinator.recover({
      workspaceId: workspace.id,
      selectedThreadId: "thread-sanitized",
      allowLiveAttach: true,
    });
    expect(calls).toEqual([
      "list_workspaces",
      "connect_workspace",
      "thread/list",
      "thread/read",
      "observation_query",
      "safe_live_attach",
    ]);
  });

  it("coverage_by_coverage_hydration_stable", () => {
    const model = buildProjectionStatusModel({
      freshness: freshness({
        workspace_catalog: "current",
        thread_catalog: "current",
        thread_detail: "stale",
        observation_snapshot: "unavailable",
      }),
      availability: availability(),
    });
    expect(model.coverages).toEqual({
      workspace_catalog: "current",
      thread_catalog: "current",
      thread_detail: "stale",
      observation_snapshot: "unavailable",
    });
  });

  it("event_gap_contract_stable", () => {
    const model = buildProjectionStatusModel({
      freshness: freshness(),
      availability: availability(),
      gap: {
        workspaceId: "workspace-sanitized",
        affectedCoverages: ["thread_catalog", "observation_snapshot"],
        skipped: 3,
        observedAt: 20,
        daemonProcessGeneration: "daemon-generation-a",
        remoteTransportGeneration: "transport-generation-a",
      },
    });
    expect(model.coverages.thread_catalog).toBe("stale");
    expect(model.coverages.observation_snapshot).toBe("stale");
    expect(model.coverages.thread_detail).toBe("current");
  });

  it("mixed_coverage_ui_contract_stable", () => {
    const model = buildProjectionStatusModel({
      freshness: freshness({ thread_detail: "stale" }),
      availability: availability(),
    });
    expect(model.mixed).toBe(true);
    expect(model.primary).toBe("stale");
  });

  it("availability_freshness_separation_stable", () => {
    const model = buildProjectionStatusModel({
      freshness: freshness(),
      availability: availability({ transport: "DISCONNECTED" }),
    });
    expect(model.primary).toBe("stale");
    expect(model.availability.kind).toBe("transport_disconnected");
    expect(model.claimsThreadAbsent).toBe(false);
  });

  it("multi_client_divergence_contract_stable", () => {
    const current = buildProjectionStatusModel({
      freshness: freshness(),
      availability: availability(),
    });
    const stale = buildProjectionStatusModel({
      freshness: freshness({ thread_catalog: "stale" }),
      availability: availability(),
    });
    expect([stale.primary, current.primary]).toEqual(["stale", "current"]);
  });

  it("multi_client_convergence_contract_stable", () => {
    const clientA = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    const clientB = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    expect([clientA.primary, clientB.primary]).toEqual(["current", "current"]);
  });

  it("stale_approval_not_actionable", () => {
    const request: ApprovalRequest = {
      request_id: "approval-request-sanitized",
      workspace_id: "workspace-sanitized",
      method: "item/commandExecution/requestApproval",
      params: { threadId: "thread-sanitized" },
    };
    expect(
      isApprovalProjectionActionable({
        request,
        observation: approvalObservation(),
        observationFreshness: "stale",
      }),
    ).toBe(false);
    expect(
      isApprovalProjectionActionable({
        request,
        observation: approvalObservation(),
        observationFreshness: "current",
      }),
    ).toBe(true);
  });

  it("delete_unknown_not_rendered_deleted", () => {
    expect(
      projectDeleteVisibility("thread-sanitized", {
        threadId: "thread-sanitized",
        state: "DELETE_OUTCOME_UNKNOWN",
      }),
    ).toBe("unknown");
  });

  it("confirmed_delete_not_resurrected", () => {
    expect(
      projectDeleteVisibility("thread-sanitized", {
        threadId: "thread-sanitized",
        state: "DELETE_CONFIRMED",
      }),
    ).toBe("deleted");
  });

  it("mutation_retry_replay_zero", async () => {
    const recover = vi.fn().mockResolvedValue(undefined);
    const intent = createReadOnlyHydrationIntent(
      buildProjectionStatusModel({
        freshness: freshness({ thread_catalog: "stale" }),
        availability: availability(),
      }),
    );
    await intent.run(recover);
    expect(intent.retryCount).toBe(0);
    expect(intent.replayCount).toBe(0);
    expect(intent.mutationCounts).toEqual({
      resumeThread: 0,
      approvalDecision: 0,
      deleteThread: 0,
      upstreamUnsubscribe: 0,
    });
  });

  it("forbidden_inference_absent", () => {
    const authority = fixture(closeoutFixtures, "authority-contract.json");
    expect(Object.values(authority.inferencePolicy as Record<string, boolean>)).toEqual(
      Array(10).fill(false),
    );
  });

  it("forbidden_ownership_semantics_absent", () => {
    const manifest = fixture(closeoutFixtures, "fixture-family-manifest.json");
    const keys = JSON.stringify(manifest).match(/"([^"]+)"\s*:/g)?.join(" ") ?? "";
    expect(keys.toLowerCase()).not.toMatch(
      /remoteclientidentity|projectionowner|recoveryowner|writerowner|subscriptionowner|leaseid|forcetakeover|telemetryauthority/,
    );
  });
});

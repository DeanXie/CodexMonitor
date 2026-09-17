import { describe, expect, it, vi } from "vitest";
import type {
  AuthoritativeObservationSnapshot,
  ProjectionFreshnessCoverage,
  ProjectionFreshnessQuerySnapshot,
  RemoteHostAvailabilitySnapshot,
} from "@/types";
import {
  assessProjectionEventDelivery,
  buildProjectionStatusModel,
  createReadOnlyHydrationIntent,
  isApprovalProjectionActionable,
  projectDeleteVisibility,
  upsertProjectionEntity,
} from "./projectionStatusModel";

const generations = {
  daemonProcessGeneration: "daemon-1",
  remoteTransportGeneration: "transport-1",
  workspaceSessionGeneration: "workspace-1",
  appServerConnectionGeneration: "app-server-1",
};

function freshness(
  statuses: Partial<Record<ProjectionFreshnessCoverage, ProjectionFreshnessQuerySnapshot["coverages"][number]["status"]>> = {},
): ProjectionFreshnessQuerySnapshot {
  const coverages: ProjectionFreshnessCoverage[] = [
    "workspace_catalog",
    "thread_catalog",
    "thread_detail",
    "observation_snapshot",
  ];
  return {
    workspaceId: "workspace-a",
    threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" },
    coverages: coverages.map((coverage) => ({
      coverage,
      status: statuses[coverage] ?? "current",
      generations,
      source: "thread_read",
      observedAt: 10,
      hydratedAt: 10,
    })),
  };
}

function availability(
  overrides: Partial<RemoteHostAvailabilitySnapshot> = {},
): RemoteHostAvailabilitySnapshot {
  return {
    targetId: "host-a",
    expectedRemoteHostIdentity: "host-identity-a",
    observedRemoteHostIdentity: "host-identity-a",
    attemptId: 1,
    transport: "CONNECTED",
    auth: "AUTHENTICATED",
    daemon: "AVAILABLE",
    runtime: { workspaceId: "workspace-a", state: "READY" },
    observedAt: 10,
    lastSuccessfulHandshakeAt: 10,
    lastRuntimeReadyAt: 10,
    daemonProcessGeneration: "daemon-1",
    diagnostics: [],
    ...overrides,
  };
}

function observation(): AuthoritativeObservationSnapshot {
  return {
    workspaceId: "workspace-a",
    threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" },
    workspaceSessionGeneration: "workspace-1",
    appServerConnectionGeneration: "app-server-1",
    writer: {} as AuthoritativeObservationSnapshot["writer"],
    subscription: {} as AuthoritativeObservationSnapshot["subscription"],
    runtime: {} as AuthoritativeObservationSnapshot["runtime"],
    pendingApprovals: [
      {
        identity: {
          requestId: "request-a",
          threadId: "thread-a",
          workspaceSessionGeneration: "workspace-1",
          appServerConnectionGeneration: "app-server-1",
        },
        state: "pending",
      },
    ],
    approvalHistory: [],
    approvalDecisionAttempts: [],
    deleteObservation: null,
  };
}

describe("Phase 3.5.4d projection status model", () => {
  it("current_projection_renders_current", () => {
    expect(buildProjectionStatusModel({ freshness: freshness(), availability: availability() }).primary).toBe("current");
  });

  it("hydrating_projection_renders_hydrating", () => {
    expect(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "hydrating" }), availability: availability() }).primary).toBe("hydrating");
  });

  it("stale_projection_renders_stale_without_claiming_absence", () => {
    const model = buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() });
    expect(model.primary).toBe("stale");
    expect(model.claimsThreadAbsent).toBe(false);
  });

  it("unavailable_projection_renders_unavailable_without_claiming_absence", () => {
    const model = buildProjectionStatusModel({ freshness: freshness({ thread_detail: "unavailable" }), availability: availability() });
    expect(model.primary).toBe("unavailable");
    expect(model.claimsThreadAbsent).toBe(false);
  });

  it("unknown_projection_remains_unknown", () => {
    expect(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "unknown" }), availability: availability() }).primary).toBe("unknown");
  });

  it("thread_catalog_current_does_not_hide_stale_thread_detail", () => {
    const model = buildProjectionStatusModel({ freshness: freshness({ thread_catalog: "current", thread_detail: "stale" }), availability: availability() });
    expect(model.coverages.thread_catalog).toBe("current");
    expect(model.coverages.thread_detail).toBe("stale");
  });

  it("partial_hydration_can_render_mixed_freshness", () => {
    const model = buildProjectionStatusModel({ freshness: freshness({ thread_catalog: "current", thread_detail: "hydrating", observation_snapshot: "unavailable" }), availability: availability() });
    expect(model.mixed).toBe(true);
    expect(model.coverageDetails).toContain("Thread catalog: Current");
    expect(model.coverageDetails).toContain("Observation snapshot: Unavailable");
  });

  it("transport_disconnect_marks_projection_stale_or_unavailable_not_absent", () => {
    const model = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ transport: "DISCONNECTED" }) });
    expect(["stale", "unavailable"]).toContain(model.primary);
    expect(model.claimsThreadAbsent).toBe(false);
  });

  it("workspace_unavailable_is_distinct_from_transport_disconnected", () => {
    const disconnected = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ transport: "DISCONNECTED" }) });
    const workspaceUnavailable = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ runtime: { workspaceId: "workspace-a", state: "UNAVAILABLE" } }) });
    expect(disconnected.availability.kind).toBe("transport_disconnected");
    expect(workspaceUnavailable.availability.kind).toBe("workspace_unavailable");
  });

  it("daemon_restart_invalidates_old_current_ui_projection", () => {
    const model = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ daemonProcessGeneration: "daemon-2" }) });
    expect(model.primary).toBe("stale");
  });

  it("same_host_identity_does_not_restore_current_ui", () => {
    const model = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ daemonProcessGeneration: "daemon-2", observedRemoteHostIdentity: "host-identity-a" }) });
    expect(model.primary).toBe("stale");
  });

  it("reconnect_preserves_cache_as_stale_until_hydrated", () => {
    const model = buildProjectionStatusModel({ freshness: freshness(), availability: availability({ transport: "CONNECTING", auth: "AUTHENTICATING" }) });
    expect(model.primary).toBe("stale");
    expect(model.retainsCachedProjection).toBe(true);
  });

  it("detected_event_gap_marks_affected_coverage_stale", () => {
    const model = buildProjectionStatusModel({ freshness: freshness(), availability: availability(), gap: { workspaceId: "workspace-a", affectedCoverages: ["thread_catalog"], skipped: 2, observedAt: 12, daemonProcessGeneration: "daemon-1", remoteTransportGeneration: "transport-1" } });
    expect(model.coverages.thread_catalog).toBe("stale");
  });

  it("event_gap_does_not_mutate_shared_authority", () => {
    const snapshot = freshness();
    buildProjectionStatusModel({ freshness: snapshot, availability: availability(), gap: { workspaceId: "workspace-a", affectedCoverages: ["thread_catalog"], skipped: 2, observedAt: 12, daemonProcessGeneration: "daemon-1", remoteTransportGeneration: "transport-1" } });
    expect(snapshot.coverages.find((entry) => entry.coverage === "thread_catalog")?.status).toBe("current");
  });

  it("client_a_gap_does_not_mark_client_b_stale", () => {
    const clientA = buildProjectionStatusModel({ freshness: freshness(), availability: availability(), gap: { workspaceId: "workspace-a", affectedCoverages: ["thread_detail"], skipped: 1, observedAt: 12, daemonProcessGeneration: "daemon-1", remoteTransportGeneration: "transport-1" } });
    const clientB = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    expect(clientA.coverages.thread_detail).toBe("stale");
    expect(clientB.coverages.thread_detail).toBe("current");
  });

  it("multi_client_projections_can_temporarily_diverge", () => {
    const clientA = buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() });
    const clientB = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    expect(clientA.primary).not.toBe(clientB.primary);
  });

  it("multi_client_authoritative_hydration_converges", () => {
    const clientA = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    const clientB = buildProjectionStatusModel({ freshness: freshness(), availability: availability() });
    expect(clientA.coverages).toEqual(clientB.coverages);
  });

  it("duplicate_event_does_not_create_duplicate_projection_entity", () => {
    const once = upsertProjectionEntity([], { id: "thread-a", title: "A" });
    const twice = upsertProjectionEntity(once, { id: "thread-a", title: "A" });
    expect(twice).toHaveLength(1);
  });

  it("out_of_order_unresolved_event_marks_stale_or_unknown", () => {
    expect(assessProjectionEventDelivery({ generationMatches: true, order: "unresolved_out_of_order" }).invalidateAs).toBe("stale");
  });

  it("event_timestamp_does_not_override_generation_authority", () => {
    const result = assessProjectionEventDelivery({ generationMatches: false, order: "current", sourceTimestamp: 999_999 });
    expect(result.apply).toBe(false);
    expect(result.invalidateAs).toBe("stale");
  });

  it("stale_approval_observation_is_not_actionable", () => {
    expect(isApprovalProjectionActionable({ request: { workspace_id: "workspace-a", request_id: "request-a", method: "approval", params: { threadId: "thread-a" } }, observation: observation(), observationFreshness: "stale" })).toBe(false);
  });

  it("current_pending_approval_observation_is_actionable", () => {
    expect(isApprovalProjectionActionable({ request: { workspace_id: "workspace-a", request_id: "request-a", method: "approval", params: { threadId: "thread-a" } }, observation: observation(), observationFreshness: "current" })).toBe(true);
  });

  it("delete_outcome_unknown_does_not_render_deleted", () => {
    expect(projectDeleteVisibility("thread-a", { state: "delete_outcome_unknown", threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" } })).toBe("unknown");
  });

  it("confirmed_delete_cannot_be_resurrected_by_stale_projection", () => {
    expect(projectDeleteVisibility("thread-a", { state: "delete_confirmed", threadKey: { codexHomeIdentity: "home-a", threadId: "thread-a" } })).toBe("deleted");
  });

  it("stale_writer_observation_is_labeled_non_current", () => {
    const model = buildProjectionStatusModel({ freshness: freshness({ observation_snapshot: "stale" }), availability: availability() });
    expect(model.observationIsCurrent).toBe(false);
  });

  it("live_polling_disconnected_does_not_define_projection_truth", () => {
    const live = buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability(), deliveryMode: "live" });
    expect(live.primary).toBe("stale");
    expect(live.deliveryMode).toBe("live");
    const disconnected = buildProjectionStatusModel({ freshness: freshness(), availability: availability(), deliveryMode: "disconnected" });
    expect(disconnected.primary).toBe("stale");
    expect(disconnected.availability.kind).toBe("transport_disconnected");
  });

  it("stale_ui_can_trigger_read_only_hydration", async () => {
    const recover = vi.fn().mockResolvedValue(undefined);
    const intent = createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() }));
    await intent.run(recover);
    expect(recover).toHaveBeenCalledTimes(1);
  });

  it("stale_ui_does_not_trigger_resume", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).mutationCounts.resumeThread).toBe(0);
  });

  it("stale_ui_does_not_trigger_approval_decision", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).mutationCounts.approvalDecision).toBe(0);
  });

  it("stale_ui_does_not_trigger_delete", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).mutationCounts.deleteThread).toBe(0);
  });

  it("stale_ui_does_not_trigger_unsubscribe_replay", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).mutationCounts.upstreamUnsubscribe).toBe(0);
  });

  it("mutation_retry_count_remains_zero", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).retryCount).toBe(0);
  });

  it("mutation_replay_count_remains_zero", () => {
    expect(createReadOnlyHydrationIntent(buildProjectionStatusModel({ freshness: freshness({ thread_detail: "stale" }), availability: availability() })).replayCount).toBe(0);
  });

  it("ui_model_contains_no_remote_client_identity", () => {
    expect(JSON.stringify(buildProjectionStatusModel({ freshness: freshness(), availability: availability() })).toLowerCase()).not.toContain("remoteclientidentity");
  });

  it("ui_model_contains_no_owner_or_lease", () => {
    const text = JSON.stringify(buildProjectionStatusModel({ freshness: freshness(), availability: availability() })).toLowerCase();
    expect(text).not.toMatch(/owner|lease/);
  });
});

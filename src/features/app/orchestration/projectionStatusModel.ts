import type {
  ApprovalRequest,
  AuthoritativeObservationSnapshot,
  ProjectionDeliveryGapEvidence,
  ProjectionFreshnessCoverage,
  ProjectionFreshnessQuerySnapshot,
  ProjectionFreshnessStatus,
  RemoteHostAvailabilitySnapshot,
} from "@/types";

export type ProjectionUiFreshness =
  | "current"
  | "hydrating"
  | "stale"
  | "unavailable"
  | "unknown";

export type ProjectionAvailabilityKind =
  | "ready"
  | "transport_disconnected"
  | "authenticating"
  | "authentication_failed"
  | "daemon_unavailable"
  | "workspace_unavailable"
  | "unknown";

export type ProjectionStatusModel = {
  primary: ProjectionUiFreshness;
  primaryLabel: string;
  coverages: Record<ProjectionFreshnessCoverage, ProjectionUiFreshness>;
  coverageDetails: string;
  mixed: boolean;
  availability: { kind: ProjectionAvailabilityKind; label: string };
  deliveryMode: "live" | "polling" | "disconnected" | null;
  retainsCachedProjection: boolean;
  claimsThreadAbsent: false;
  observationIsCurrent: boolean;
};

const coverageOrder: ProjectionFreshnessCoverage[] = [
  "workspace_catalog",
  "thread_catalog",
  "thread_detail",
  "observation_snapshot",
];

const coverageLabels: Record<ProjectionFreshnessCoverage, string> = {
  workspace_catalog: "Workspace catalog",
  thread_catalog: "Thread catalog",
  thread_detail: "Thread detail",
  observation_snapshot: "Observation snapshot",
};

const freshnessLabels: Record<ProjectionUiFreshness, string> = {
  current: "Current",
  hydrating: "Hydrating",
  stale: "Stale",
  unavailable: "Unavailable",
  unknown: "Unknown",
};

const severity: Record<ProjectionUiFreshness, number> = {
  current: 0,
  hydrating: 1,
  stale: 2,
  unknown: 3,
  unavailable: 4,
};

function toUiFreshness(status: ProjectionFreshnessStatus): ProjectionUiFreshness {
  return status === "not_hydrated" ? "unknown" : status;
}

function availabilityModel(
  snapshot?: RemoteHostAvailabilitySnapshot | null,
  deliveryMode?: "live" | "polling" | "disconnected" | null,
) {
  if (deliveryMode === "disconnected") {
    return { kind: "transport_disconnected" as const, label: "Transport disconnected" };
  }
  if (!snapshot) {
    return { kind: "unknown" as const, label: "Connection status unknown" };
  }
  if (
    snapshot.transport === "DISCONNECTED" ||
    snapshot.transport === "ENDPOINT_UNREACHABLE"
  ) {
    return { kind: "transport_disconnected" as const, label: "Transport disconnected" };
  }
  if (snapshot.transport === "CONNECTING" || snapshot.auth === "AUTHENTICATING") {
    return { kind: "authenticating" as const, label: "Connecting" };
  }
  if (snapshot.auth === "FAILED") {
    return { kind: "authentication_failed" as const, label: "Authentication failed" };
  }
  if (snapshot.daemon !== "AVAILABLE") {
    return { kind: "daemon_unavailable" as const, label: "Daemon unavailable" };
  }
  if (snapshot.runtime.state !== "READY") {
    return { kind: "workspace_unavailable" as const, label: "Workspace unavailable" };
  }
  return { kind: "ready" as const, label: "Remote ready" };
}

function gapMatchesCurrentTransport(
  gap: ProjectionDeliveryGapEvidence,
  snapshot: ProjectionFreshnessQuerySnapshot,
) {
  const catalog = snapshot.coverages.find(
    (coverage) => coverage.coverage === "thread_catalog",
  );
  return (
    gap.workspaceId === snapshot.workspaceId &&
    gap.daemonProcessGeneration ===
      (catalog?.generations.daemonProcessGeneration ?? null) &&
    gap.remoteTransportGeneration ===
      (catalog?.generations.remoteTransportGeneration ?? null)
  );
}

export function buildProjectionStatusModel({
  freshness,
  availability,
  gap,
  deliveryMode = null,
}: {
  freshness: ProjectionFreshnessQuerySnapshot;
  availability?: RemoteHostAvailabilitySnapshot | null;
  gap?: ProjectionDeliveryGapEvidence | null;
  deliveryMode?: "live" | "polling" | "disconnected" | null;
}): ProjectionStatusModel {
  const currentDaemonGeneration = availability?.daemonProcessGeneration ?? null;
  const reconnecting =
    deliveryMode === "disconnected" ||
    availability?.transport === "CONNECTING" ||
    availability?.transport === "DISCONNECTED" ||
    availability?.transport === "ENDPOINT_UNREACHABLE" ||
    availability?.auth === "AUTHENTICATING";
  const gapApplies = Boolean(gap && gapMatchesCurrentTransport(gap, freshness));

  const coverages = Object.fromEntries(
    coverageOrder.map((coverage) => {
      const evidence = freshness.coverages.find(
        (candidate) => candidate.coverage === coverage,
      );
      let status = evidence ? toUiFreshness(evidence.status) : "unknown";
      const daemonChanged = Boolean(
        evidence?.generations.daemonProcessGeneration &&
          currentDaemonGeneration &&
          evidence.generations.daemonProcessGeneration !== currentDaemonGeneration,
      );
      if (
        (gapApplies && gap?.affectedCoverages.includes(coverage)) ||
        daemonChanged ||
        (reconnecting && status === "current")
      ) {
        status = "stale";
      }
      return [coverage, status];
    }),
  ) as Record<ProjectionFreshnessCoverage, ProjectionUiFreshness>;

  const distinct = new Set(Object.values(coverages));
  const primary = Object.values(coverages).reduce<ProjectionUiFreshness>(
    (mostSevere, candidate) =>
      severity[candidate] > severity[mostSevere] ? candidate : mostSevere,
    "current",
  );

  return {
    primary,
    primaryLabel: freshnessLabels[primary],
    coverages,
    coverageDetails: coverageOrder
      .map((coverage) => `${coverageLabels[coverage]}: ${freshnessLabels[coverages[coverage]]}`)
      .join(" · "),
    mixed: distinct.size > 1,
    availability: availabilityModel(availability, deliveryMode),
    deliveryMode,
    retainsCachedProjection: freshness.coverages.some((coverage) =>
      ["current", "stale", "hydrating"].includes(coverage.status),
    ),
    claimsThreadAbsent: false,
    observationIsCurrent: coverages.observation_snapshot === "current",
  };
}

export function assessProjectionEventDelivery({
  generationMatches,
  order,
}: {
  generationMatches: boolean;
  order: "current" | "duplicate" | "unresolved_out_of_order";
  sourceTimestamp?: number;
}) {
  if (!generationMatches) {
    return { apply: false, invalidateAs: "stale" as const };
  }
  if (order === "duplicate") {
    return { apply: false, invalidateAs: null };
  }
  if (order === "unresolved_out_of_order") {
    return { apply: false, invalidateAs: "stale" as const };
  }
  return { apply: true, invalidateAs: null };
}

export function upsertProjectionEntity<T extends { id: string }>(
  entities: T[],
  incoming: T,
) {
  const index = entities.findIndex((entity) => entity.id === incoming.id);
  if (index < 0) {
    return [...entities, incoming];
  }
  const next = [...entities];
  next[index] = incoming;
  return next;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

export function isApprovalProjectionActionable({
  request,
  observation,
  observationFreshness,
}: {
  request: ApprovalRequest;
  observation: AuthoritativeObservationSnapshot | null;
  observationFreshness: ProjectionUiFreshness;
}) {
  if (!observation || observationFreshness !== "current") {
    return false;
  }
  const requestedThreadId =
    typeof request.params?.threadId === "string"
      ? request.params.threadId
      : typeof request.params?.thread_id === "string"
        ? request.params.thread_id
        : null;
  if (!requestedThreadId || requestedThreadId !== observation.threadKey.threadId) {
    return false;
  }
  return observation.pendingApprovals.some((raw) => {
    const pending = asRecord(raw);
    const identity = asRecord(pending?.identity);
    return Boolean(
      pending && identity &&
        String(identity.requestId ?? identity.request_id ?? "") === String(request.request_id) &&
        identity.threadId === requestedThreadId &&
        pending.state === "pending" &&
        identity.workspaceSessionGeneration === observation.workspaceSessionGeneration &&
        identity.appServerConnectionGeneration === observation.appServerConnectionGeneration,
    );
  });
}

export function projectDeleteVisibility(
  threadId: string,
  deleteObservation: Record<string, unknown> | null,
): "visible" | "unknown" | "deleted" {
  const threadKey = asRecord(deleteObservation?.threadKey);
  const observedThreadId = deleteObservation?.threadId ?? threadKey?.threadId;
  if (!deleteObservation || observedThreadId !== threadId) {
    return "visible";
  }
  if (["DELETE_CONFIRMED", "delete_confirmed"].includes(String(deleteObservation.state))) {
    return "deleted";
  }
  if (["DELETE_OUTCOME_UNKNOWN", "delete_outcome_unknown"].includes(String(deleteObservation.state))) {
    return "unknown";
  }
  return "visible";
}

export function createReadOnlyHydrationIntent(model: ProjectionStatusModel) {
  return {
    retryCount: 0,
    replayCount: 0,
    mutationCounts: {
      resumeThread: 0,
      approvalDecision: 0,
      deleteThread: 0,
      upstreamUnsubscribe: 0,
    },
    async run(recover: () => Promise<unknown>) {
      if (model.primary !== "current") {
        await recover();
      }
    },
  };
}

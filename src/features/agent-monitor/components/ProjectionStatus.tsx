import type {
  ProjectionActionCapability,
  ProjectionReconciliationState,
  SurfaceProjectionObservation,
  SurfaceProjectionState,
} from "../global-source/types";

const STATE_LABELS: Record<SurfaceProjectionState, string> = {
  PRESENT: "Present in this surface",
  ABSENT: "Not present in this surface",
  STALE: "Surface still references a deleted Thread",
  UNKNOWN: "Not enough evidence",
  NOT_APPLICABLE: "Not applicable",
};

const CAPABILITY_LABELS: Record<ProjectionActionCapability, string> = {
  REFRESHABLE: "Refreshable",
  INVALIDATABLE: "Invalidatable",
  OBSERVE_ONLY: "Observe only",
  UNSUPPORTED: "Unsupported",
};

function titleCase(value: string) {
  return value.toLowerCase().replace(/_/g, " ").replace(/^./, (character: string) => character.toUpperCase());
}

function reconciliationLabel(observation: SurfaceProjectionObservation) {
  if (
    observation.reconciliationState === "PENDING"
    && observation.key.surface === "DESKTOP"
    && (observation.actionCapability === "OBSERVE_ONLY"
      || observation.actionCapability === "UNSUPPORTED")
  ) {
    return "Waiting for Desktop to refresh";
  }
  const labels: Record<ProjectionReconciliationState, string> = {
    NOT_REQUIRED: "Not required",
    PENDING: "Pending",
    RECONCILED: "Reconciled",
    UNKNOWN: "Unknown",
  };
  return labels[observation.reconciliationState];
}

function projectionLabel(observation: SurfaceProjectionObservation) {
  return `${titleCase(observation.key.surface)} ${titleCase(observation.key.projectionKind)}`;
}

export function ProjectionStatusDetails({
  observations,
}: {
  observations: readonly SurfaceProjectionObservation[];
}) {
  if (observations.length === 0) return null;
  const attention = observations.some((observation) =>
    observation.state === "STALE" || observation.reconciliationState === "PENDING");
  return (
    <details className="agent-monitor-projection-details">
      <summary>Projection status · {attention ? "Attention" : "Observed"}</summary>
      <ul>
        {observations.map((observation) => (
          <li key={`${observation.key.surface}:${observation.key.projectionKind}:${observation.observedAt}`}>
            <strong>{projectionLabel(observation)}</strong>
            <span>{STATE_LABELS[observation.state]}</span>
            <span>Reconciliation: <strong>{reconciliationLabel(observation)}</strong></span>
            <span>Capability: <strong>{CAPABILITY_LABELS[observation.actionCapability]}</strong></span>
            {observation.diagnostics.map((diagnostic) => (
              <span key={diagnostic}>Diagnostic: {diagnostic}</span>
            ))}
          </li>
        ))}
      </ul>
    </details>
  );
}

export function ProjectionIssues({
  observations,
}: {
  observations: readonly SurfaceProjectionObservation[];
}) {
  if (observations.length === 0) return null;
  return (
    <section className="agent-monitor-projection-issues" aria-label="Projection Issues">
      <div className="agent-monitor-section-heading">
        <h2>Projection Issues</h2>
        <span>{observations.length} observed</span>
      </div>
      <ul>
        {observations.map((observation) => (
          <li key={`${observation.key.threadKey.codexHomeIdentity}:${observation.key.threadKey.threadId}:${observation.key.projectionKind}`}>
            <strong>{projectionLabel(observation)}</strong>
            <span>Desktop retained a projection for a Thread that no longer exists.</span>
            <span>{reconciliationLabel(observation)}</span>
            <span>Capability: <strong>{CAPABILITY_LABELS[observation.actionCapability]}</strong></span>
          </li>
        ))}
      </ul>
    </section>
  );
}

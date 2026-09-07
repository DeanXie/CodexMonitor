export type GlobalSourceKind =
  | "monitor-app-server"
  | "codex-cli-rollout"
  | "historical-rollout-scan";

export type GlobalSourceTemporalClass = "LIVE" | "NEAR_LIVE" | "HISTORICAL";
export type GlobalSourceFreshnessState = "fresh" | "stale" | "settled" | "unknown";

export type GlobalSourceFreshness = {
  state: GlobalSourceFreshnessState;
  lastCompleteRecordObservedAtMs: number | null;
  reason: string;
};

export type GlobalSourceProvenance = {
  sourceKind: GlobalSourceKind;
  temporalClass: GlobalSourceTemporalClass;
  sourceInstanceId: string;
  sourceGeneration: string;
  sourceTimestampMs: number | null;
  observedTimestampMs: number;
  freshness: GlobalSourceFreshness;
};

export type GlobalSourceResolvedValue<T> = {
  value: T;
  provenance: GlobalSourceProvenance;
};

export type GlobalSourceThreadKey = {
  codexHomeIdentity: string;
  threadId: string;
};

export type SurfaceProjectionSurface = "MONITOR" | "DESKTOP" | "CLI";
export type SurfaceProjectionKind =
  | "SESSION_LIST"
  | "GLOBAL_SOURCE_SNAPSHOT"
  | "CURRENT_SESSION"
  | "HISTORY_LIST"
  | "CATALOG"
  | "SIDEBAR"
  | "PROJECT"
  | "DISCOVERABILITY";
export type SurfaceProjectionState =
  | "PRESENT"
  | "ABSENT"
  | "STALE"
  | "UNKNOWN"
  | "NOT_APPLICABLE";
export type SurfaceProjectionCoverage =
  | "COMPLETE"
  | "BOUNDED"
  | "PARTIAL"
  | "FAILED"
  | "NOT_OBSERVED"
  | "NOT_APPLICABLE";
export type ProjectionReconciliationState =
  | "NOT_REQUIRED"
  | "PENDING"
  | "RECONCILED"
  | "UNKNOWN";
export type ProjectionActionCapability =
  | "REFRESHABLE"
  | "INVALIDATABLE"
  | "OBSERVE_ONLY"
  | "UNSUPPORTED";
export type ProjectionMembershipExpectation = "REQUIRED" | "OPTIONAL" | "UNKNOWN";

export type SurfaceProjectionObservation = {
  key: {
    threadKey: GlobalSourceThreadKey;
    surface: SurfaceProjectionSurface;
    projectionKind: SurfaceProjectionKind;
  };
  state: SurfaceProjectionState;
  coverage: SurfaceProjectionCoverage;
  observedAt: number;
  provenance: string[];
  diagnostics: string[];
  reconciliationState: ProjectionReconciliationState;
  actionCapability: ProjectionActionCapability;
  membershipExpectation: ProjectionMembershipExpectation;
};

export type GlobalSourceTurnKey = {
  threadKey: GlobalSourceThreadKey;
  turnId: string;
};

export type GlobalSourceLifecycle = "running" | "waiting" | "completed";
export type GlobalSourceProducerSurface =
  | "MONITOR"
  | "DESKTOP"
  | "CLI"
  | "IDE"
  | "AMBIGUOUS"
  | "UNKNOWN";
export type GlobalSourceEvidenceConfidence = "confirmed" | "inferred" | "unknown";

export type GlobalSourceProducerClassification = {
  surface: GlobalSourceProducerSurface;
  confidence: GlobalSourceEvidenceConfidence;
  evidence: string[];
  provenance: string[];
};

export type GlobalSourceWorkspaceAssignment = {
  state: "ASSIGNED" | "AMBIGUOUS" | "UNASSIGNED";
  workspaceId: string | null;
  provenance: string;
  matchedPath: string | null;
  candidateWorkspaceIds: string[];
};

export type GlobalSourceTokenSnapshot = {
  inputTokens: number;
  cachedInputTokens: number;
  cacheWriteInputTokens: number | null;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type GlobalSourceTurn = {
  key: GlobalSourceTurnKey;
  lifecycle: GlobalSourceResolvedValue<GlobalSourceLifecycle> | null;
  startedAt: GlobalSourceProvenance | null;
  completedAt: GlobalSourceProvenance | null;
};

export type GlobalSourceThread = {
  key: GlobalSourceThreadKey;
  parentThreadKey: GlobalSourceResolvedValue<GlobalSourceThreadKey> | null;
  agentPath: GlobalSourceResolvedValue<string> | null;
  currentTurn: GlobalSourceTurn | null;
  lifecycle: GlobalSourceResolvedValue<GlobalSourceLifecycle> | null;
  observedModel: GlobalSourceResolvedValue<string> | null;
  tokenSnapshot: GlobalSourceResolvedValue<GlobalSourceTokenSnapshot> | null;
  producerSurface?: GlobalSourceProducerClassification;
  workspaceAssignment?: GlobalSourceWorkspaceAssignment | null;
  authorityProvenance: GlobalSourceProvenance | null;
  liveLaneCount: number;
  nearLiveLaneCount: number;
  historicalLaneCount: number;
};

export type GlobalSourceSnapshot = {
  revision: number;
  generatedAtMs: number;
  workspaceCodexHomeIdentities: Record<string, string>;
  threads: GlobalSourceThread[];
  /** Optional for snapshots produced before the Phase 3.4 presentation bridge. */
  surfaceProjections?: SurfaceProjectionObservation[];
};

export const EMPTY_GLOBAL_SOURCE_SNAPSHOT: GlobalSourceSnapshot = {
  revision: 0,
  generatedAtMs: 0,
  workspaceCodexHomeIdentities: {},
  threads: [],
};

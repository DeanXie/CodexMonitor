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
};

export const EMPTY_GLOBAL_SOURCE_SNAPSHOT: GlobalSourceSnapshot = {
  revision: 0,
  generatedAtMs: 0,
  workspaceCodexHomeIdentities: {},
  threads: [],
};

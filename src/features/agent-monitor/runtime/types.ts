export type RuntimeRecordSource = "CLIENT" | "SERVER" | "EVENT" | "STDERR" | "HYDRATION";

export type RuntimeProtocolRecord = {
  source: RuntimeRecordSource;
  capturedAt: string;
  label: string;
  payload: Record<string, unknown>;
};

export type RuntimeProvenance = {
  eventKey: string;
  method: string;
  recordSource: RuntimeRecordSource;
  serverTimeMs: number | null;
  observedAtMs: number;
};

export type RuntimeObservation<T> = {
  value: T;
  provenance: RuntimeProvenance;
};

export type RuntimeTimestamp = {
  valueMs: number;
  provenance: RuntimeProvenance;
};

export type RuntimeTokenUsage = {
  cacheWriteInputTokens: number;
  cachedInputTokens: number;
  inputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type ThreadTokenSnapshot = {
  last: RuntimeTokenUsage;
  total: RuntimeTokenUsage;
  modelContextWindow: number | null;
  provenance: RuntimeProvenance;
};

export type ThreadRuntimeStatus = "active" | "idle" | "notLoaded";
export type TurnRuntimeStatus = "running" | "waiting" | "completed" | "failed";
export type ObservedModelSource = "threadStartResponse" | "threadSettingsUpdated";

export type ObservedModel = RuntimeObservation<string> & {
  source: ObservedModelSource;
};

export type ThreadRuntimeState = {
  threadId: string;
  workspaceId: string | null;
  identityProvenance: RuntimeProvenance | null;
  parentThreadId: string | null;
  parentProvenance: RuntimeProvenance | null;
  childThreadIds: string[];
  status: RuntimeObservation<ThreadRuntimeStatus> | null;
  observedModel: ObservedModel | null;
  tokenUsage: ThreadTokenSnapshot | null;
  createdAt: RuntimeTimestamp | null;
  lastActivityAt: RuntimeTimestamp | null;
};

export type TurnRuntimeState = {
  turnId: string;
  threadId: string;
  status: RuntimeObservation<TurnRuntimeStatus> | null;
  requestedModel: RuntimeObservation<string> | null;
  startedAt: RuntimeTimestamp | null;
  completedAt: RuntimeTimestamp | null;
  durationMs: RuntimeObservation<number> | null;
  tokenDelta: RuntimeTokenUsage;
  lastActivityAt: RuntimeTimestamp | null;
};

export type AgentAssignment = {
  assignmentId: string;
  parentThreadId: string;
  childThreadId: string;
  agentThreadId: string;
  agentPath: string | null;
  spawnedAt: RuntimeTimestamp;
  provenance: RuntimeProvenance;
};

export type AgentRuntimeStore = {
  threads: Record<string, ThreadRuntimeState>;
  turns: Record<string, TurnRuntimeState>;
  assignments: Record<string, AgentAssignment>;
  pendingTurnRequestsByThread: Record<string, RuntimeObservation<string>>;
  appliedEventKeys: Record<string, true>;
};

type RuntimeEventBase = {
  eventKey: string;
  workspaceId: string | null;
  provenance: RuntimeProvenance;
};

export type NormalizedRuntimeEvent =
  | (RuntimeEventBase & {
      type: "threadStarted";
      threadId: string;
      parentThreadId: string | null;
      status: ThreadRuntimeStatus | null;
      createdAtMs: number | null;
    })
  | (RuntimeEventBase & {
      type: "threadHydrated";
      threadId: string;
      parentThreadId: string | null;
      status: ThreadRuntimeStatus | null;
      createdAtMs: number | null;
    })
  | (RuntimeEventBase & {
      type: "threadStatusChanged";
      threadId: string;
      status: ThreadRuntimeStatus;
    })
  | (RuntimeEventBase & {
      type: "observedModelConfirmed";
      threadId: string;
      model: string;
      source: ObservedModelSource;
    })
  | (RuntimeEventBase & {
      type: "turnRequested";
      threadId: string;
      requestedModel: string;
    })
  | (RuntimeEventBase & {
      type: "turnStarted";
      threadId: string;
      turnId: string;
      startedAtMs: number | null;
    })
  | (RuntimeEventBase & {
      type: "turnHydrated";
      threadId: string;
      turnId: string;
      status: Exclude<TurnRuntimeStatus, "failed">;
      startedAtMs: number | null;
      completedAtMs: number | null;
      durationMs: number | null;
    })
  | (RuntimeEventBase & {
      type: "turnWaiting" | "turnResumed";
      threadId: string;
      turnId: string;
    })
  | (RuntimeEventBase & {
      type: "turnCompleted";
      threadId: string;
      turnId: string;
      startedAtMs: number | null;
      completedAtMs: number | null;
      durationMs: number | null;
    })
  | (RuntimeEventBase & {
      type: "threadTokensUpdated";
      threadId: string;
      turnId: string | null;
      last: RuntimeTokenUsage;
      total: RuntimeTokenUsage;
      modelContextWindow: number | null;
    })
  | (RuntimeEventBase & {
      type: "assignmentStarted";
      assignmentId: string;
      parentThreadId: string;
      childThreadId: string;
      agentPath: string | null;
      spawnedAtMs: number;
    });

export const EMPTY_RUNTIME_TOKEN_USAGE: RuntimeTokenUsage = {
  cacheWriteInputTokens: 0,
  cachedInputTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  reasoningOutputTokens: 0,
  totalTokens: 0,
};

import { normalizeRuntimeRecord } from "./eventNormalizer";
import {
  EMPTY_RUNTIME_TOKEN_USAGE,
  type AgentRuntimeStore,
  type NormalizedRuntimeEvent,
  type ObservedModel,
  type RuntimeObservation,
  type RuntimeProtocolRecord,
  type RuntimeProvenance,
  type RuntimeTimestamp,
  type RuntimeTokenUsage,
  type ThreadRuntimeState,
  type TurnRuntimeState,
  type TurnRuntimeStatus,
} from "./types";

function evidenceTime(provenance: RuntimeProvenance): number {
  return provenance.serverTimeMs ?? provenance.observedAtMs;
}

function timestamp(valueMs: number, provenance: RuntimeProvenance): RuntimeTimestamp {
  return {
    valueMs,
    provenance: provenance.recordSource === "HYDRATION"
      ? provenance
      : { ...provenance, serverTimeMs: valueMs },
  };
}

function activityTimestamp(provenance: RuntimeProvenance): RuntimeTimestamp {
  return {
    valueMs: evidenceTime(provenance),
    provenance,
  };
}

function isNewerOrEqual(
  incoming: RuntimeProvenance,
  existing: RuntimeProvenance | null | undefined,
): boolean {
  return !existing || evidenceTime(incoming) >= evidenceTime(existing);
}

function emptyThread(threadId: string, workspaceId: string | null): ThreadRuntimeState {
  return {
    threadId,
    workspaceId,
    identityProvenance: null,
    parentThreadId: null,
    parentProvenance: null,
    childThreadIds: [],
    status: null,
    observedModel: null,
    tokenUsage: null,
    createdAt: null,
    lastActivityAt: null,
  };
}

function emptyTurn(turnId: string, threadId: string): TurnRuntimeState {
  return {
    turnId,
    threadId,
    status: null,
    requestedModel: null,
    startedAt: null,
    completedAt: null,
    durationMs: null,
    tokenDelta: { ...EMPTY_RUNTIME_TOKEN_USAGE },
    lastActivityAt: null,
  };
}

function addUsage(left: RuntimeTokenUsage, right: RuntimeTokenUsage): RuntimeTokenUsage {
  return {
    cacheWriteInputTokens: Math.max(0, left.cacheWriteInputTokens + right.cacheWriteInputTokens),
    cachedInputTokens: Math.max(0, left.cachedInputTokens + right.cachedInputTokens),
    inputTokens: Math.max(0, left.inputTokens + right.inputTokens),
    outputTokens: Math.max(0, left.outputTokens + right.outputTokens),
    reasoningOutputTokens: Math.max(
      0,
      left.reasoningOutputTokens + right.reasoningOutputTokens,
    ),
    totalTokens: Math.max(0, left.totalTokens + right.totalTokens),
  };
}

function terminal(status: TurnRuntimeStatus | undefined): boolean {
  return status === "completed" || status === "failed";
}

function updateThreadActivity(
  thread: ThreadRuntimeState,
  provenance: RuntimeProvenance,
): ThreadRuntimeState {
  if (!isNewerOrEqual(provenance, thread.lastActivityAt?.provenance)) return thread;
  return { ...thread, lastActivityAt: activityTimestamp(provenance) };
}

function updateTurnActivity(
  turn: TurnRuntimeState,
  provenance: RuntimeProvenance,
): TurnRuntimeState {
  if (!isNewerOrEqual(provenance, turn.lastActivityAt?.provenance)) return turn;
  return { ...turn, lastActivityAt: activityTimestamp(provenance) };
}

function chooseObservedModel(
  current: ObservedModel | null,
  incoming: ObservedModel,
): ObservedModel {
  if (!current) return incoming;
  const rank = { threadStartResponse: 1, threadSettingsUpdated: 2 } as const;
  if (rank[incoming.source] !== rank[current.source]) {
    return rank[incoming.source] > rank[current.source] ? incoming : current;
  }
  return isNewerOrEqual(incoming.provenance, current.provenance) ? incoming : current;
}

export function createRuntimeState(): AgentRuntimeStore {
  return {
    threads: {},
    turns: {},
    assignments: {},
    pendingTurnRequestsByThread: {},
    appliedEventKeys: {},
  };
}

export function getLiveRuntimeClearState(state: AgentRuntimeStore): {
  canClear: boolean;
  activeTurnIds: string[];
} {
  const activeTurnIds = Object.values(state.turns)
    .filter((turn) => turn.status?.value === "running" || turn.status?.value === "waiting")
    .map((turn) => turn.turnId);
  return {
    canClear: activeTurnIds.length === 0,
    activeTurnIds,
  };
}

export function clearLiveRuntimeState(state: AgentRuntimeStore): AgentRuntimeStore {
  return getLiveRuntimeClearState(state).canClear ? createRuntimeState() : state;
}

export function applyRuntimeEvent(
  state: AgentRuntimeStore,
  event: NormalizedRuntimeEvent,
): AgentRuntimeStore {
  if (state.appliedEventKeys[event.eventKey]) return state;

  const next: AgentRuntimeStore = {
    ...state,
    threads: { ...state.threads },
    turns: { ...state.turns },
    assignments: { ...state.assignments },
    pendingTurnRequestsByThread: { ...state.pendingTurnRequestsByThread },
    appliedEventKeys: { ...state.appliedEventKeys, [event.eventKey]: true },
  };

  const getThread = (threadId: string) => {
    const existing = next.threads[threadId] ?? emptyThread(threadId, event.workspaceId);
    const thread = {
      ...existing,
      workspaceId: existing.workspaceId ?? event.workspaceId,
      childThreadIds: [...existing.childThreadIds],
    };
    next.threads[threadId] = thread;
    return thread;
  };
  const getTurn = (turnId: string, threadId: string) => {
    const turn = { ...(next.turns[turnId] ?? emptyTurn(turnId, threadId)) };
    next.turns[turnId] = turn;
    return turn;
  };

  if (event.type === "threadStarted") {
    let thread = getThread(event.threadId);
    thread.identityProvenance = event.provenance;
    if (event.parentThreadId && isNewerOrEqual(event.provenance, thread.parentProvenance)) {
      thread.parentThreadId = event.parentThreadId;
      thread.parentProvenance = event.provenance;
      const parent = getThread(event.parentThreadId);
      if (!parent.childThreadIds.includes(event.threadId)) parent.childThreadIds.push(event.threadId);
    }
    if (event.status && isNewerOrEqual(event.provenance, thread.status?.provenance)) {
      thread.status = { value: event.status, provenance: event.provenance };
    }
    if (event.createdAtMs !== null) {
      thread.createdAt = timestamp(event.createdAtMs, event.provenance);
    }
    thread = updateThreadActivity(thread, event.provenance);
    next.threads[event.threadId] = thread;
    return next;
  }

  if (event.type === "threadHydrated") {
    let thread = getThread(event.threadId);
    if (!thread.identityProvenance || isNewerOrEqual(event.provenance, thread.identityProvenance)) {
      thread.identityProvenance = event.provenance;
    }
    if (event.parentThreadId && !thread.parentThreadId) {
      thread.parentThreadId = event.parentThreadId;
      thread.parentProvenance = event.provenance;
      const parent = getThread(event.parentThreadId);
      if (!parent.childThreadIds.includes(event.threadId)) parent.childThreadIds.push(event.threadId);
    }
    if (event.status && !thread.status) {
      thread.status = { value: event.status, provenance: event.provenance };
    }
    if (event.createdAtMs !== null && !thread.createdAt) {
      thread.createdAt = timestamp(event.createdAtMs, event.provenance);
    }
    thread = updateThreadActivity(thread, event.provenance);
    next.threads[event.threadId] = thread;
    return next;
  }

  if (event.type === "observedModelConfirmed") {
    let thread = getThread(event.threadId);
    thread.observedModel = chooseObservedModel(thread.observedModel, {
      value: event.model,
      source: event.source,
      provenance: event.provenance,
    });
    thread = updateThreadActivity(thread, event.provenance);
    next.threads[event.threadId] = thread;
    return next;
  }

  if (event.type === "threadStatusChanged") {
    let thread = getThread(event.threadId);
    if (isNewerOrEqual(event.provenance, thread.status?.provenance)) {
      thread.status = { value: event.status, provenance: event.provenance };
    }
    thread = updateThreadActivity(thread, event.provenance);
    next.threads[event.threadId] = thread;
    return next;
  }

  if (event.type === "turnRequested") {
    const requested: RuntimeObservation<string> = {
      value: event.requestedModel,
      provenance: event.provenance,
    };
    const candidates = Object.values(next.turns).filter(
      (turn) => turn.threadId === event.threadId && turn.requestedModel === null,
    );
    if (candidates.length === 1) {
      candidates[0].requestedModel = requested;
      next.turns[candidates[0].turnId] = candidates[0];
    } else {
      next.pendingTurnRequestsByThread[event.threadId] = requested;
    }
    return next;
  }

  if (event.type === "turnStarted") {
    const thread = updateThreadActivity(getThread(event.threadId), event.provenance);
    next.threads[event.threadId] = thread;
    let turn = getTurn(event.turnId, event.threadId);
    if (!terminal(turn.status?.value)) {
      turn.status = { value: "running", provenance: event.provenance };
    }
    if (event.startedAtMs !== null && !turn.startedAt) {
      turn.startedAt = timestamp(event.startedAtMs, event.provenance);
    }
    const pending = next.pendingTurnRequestsByThread[event.threadId];
    if (!turn.requestedModel && pending) {
      turn.requestedModel = pending;
      delete next.pendingTurnRequestsByThread[event.threadId];
    }
    turn = updateTurnActivity(turn, event.provenance);
    next.turns[event.turnId] = turn;
    return next;
  }

  if (event.type === "turnHydrated") {
    const thread = updateThreadActivity(getThread(event.threadId), event.provenance);
    next.threads[event.threadId] = thread;
    let turn = getTurn(event.turnId, event.threadId);
    if (!turn.status) {
      turn.status = { value: event.status, provenance: event.provenance };
    }
    if (event.startedAtMs !== null && !turn.startedAt) {
      turn.startedAt = timestamp(event.startedAtMs, event.provenance);
    }
    if (event.completedAtMs !== null && !turn.completedAt) {
      turn.completedAt = timestamp(event.completedAtMs, event.provenance);
    }
    if (event.durationMs !== null && event.durationMs >= 0 && !turn.durationMs) {
      turn.durationMs = { value: event.durationMs, provenance: event.provenance };
    }
    turn = updateTurnActivity(turn, event.provenance);
    next.turns[event.turnId] = turn;
    return next;
  }

  if (event.type === "turnWaiting" || event.type === "turnResumed") {
    let turn = getTurn(event.turnId, event.threadId);
    if (!terminal(turn.status?.value)) {
      turn.status = {
        value: event.type === "turnWaiting" ? "waiting" : "running",
        provenance: event.provenance,
      };
    }
    turn = updateTurnActivity(turn, event.provenance);
    next.turns[event.turnId] = turn;
    next.threads[event.threadId] = updateThreadActivity(
      getThread(event.threadId),
      event.provenance,
    );
    return next;
  }

  if (event.type === "turnCompleted") {
    let turn = getTurn(event.turnId, event.threadId);
    const completionIsValid =
      event.completedAtMs === null ||
      turn.startedAt === null ||
      event.completedAtMs >= turn.startedAt.valueMs;
    if (completionIsValid && !terminal(turn.status?.value)) {
      turn.status = { value: "completed", provenance: event.provenance };
    }
    if (event.startedAtMs !== null && !turn.startedAt) {
      turn.startedAt = timestamp(event.startedAtMs, event.provenance);
    }
    if (event.completedAtMs !== null && !turn.completedAt) {
      turn.completedAt = timestamp(event.completedAtMs, event.provenance);
    }
    if (event.durationMs !== null && event.durationMs >= 0 && !turn.durationMs) {
      turn.durationMs = { value: event.durationMs, provenance: event.provenance };
    }
    turn = updateTurnActivity(turn, event.provenance);
    next.turns[event.turnId] = turn;
    next.threads[event.threadId] = updateThreadActivity(
      getThread(event.threadId),
      event.provenance,
    );
    return next;
  }

  if (event.type === "threadTokensUpdated") {
    let thread = getThread(event.threadId);
    if (isNewerOrEqual(event.provenance, thread.tokenUsage?.provenance)) {
      thread.tokenUsage = {
        last: event.last,
        total: event.total,
        modelContextWindow: event.modelContextWindow,
        provenance: event.provenance,
      };
    }
    thread = updateThreadActivity(thread, event.provenance);
    next.threads[event.threadId] = thread;
    if (event.turnId) {
      let turn = getTurn(event.turnId, event.threadId);
      turn.tokenDelta = addUsage(turn.tokenDelta, event.last);
      turn = updateTurnActivity(turn, event.provenance);
      next.turns[event.turnId] = turn;
    }
    return next;
  }

  if (event.type === "assignmentStarted") {
    if (!next.assignments[event.assignmentId]) {
      next.assignments[event.assignmentId] = {
        assignmentId: event.assignmentId,
        parentThreadId: event.parentThreadId,
        childThreadId: event.childThreadId,
        agentThreadId: event.childThreadId,
        agentPath: event.agentPath,
        spawnedAt: { valueMs: event.spawnedAtMs, provenance: event.provenance },
        provenance: event.provenance,
      };
    }
    let parent = getThread(event.parentThreadId);
    if (!parent.childThreadIds.includes(event.childThreadId)) {
      parent.childThreadIds.push(event.childThreadId);
    }
    parent = updateThreadActivity(parent, event.provenance);
    next.threads[event.parentThreadId] = parent;
    let child = getThread(event.childThreadId);
    if (!child.parentThreadId || isNewerOrEqual(event.provenance, child.parentProvenance)) {
      child.parentThreadId = event.parentThreadId;
      child.parentProvenance = event.provenance;
    }
    child = updateThreadActivity(child, event.provenance);
    next.threads[event.childThreadId] = child;
    return next;
  }

  return next;
}

export function applyRuntimeRecords(
  state: AgentRuntimeStore,
  records: RuntimeProtocolRecord[],
  observedAtStartMs = Date.now(),
): AgentRuntimeStore {
  return records.reduce((current, record, index) => {
    const events = normalizeRuntimeRecord(record, observedAtStartMs + index);
    return events.reduce(applyRuntimeEvent, current);
  }, state);
}

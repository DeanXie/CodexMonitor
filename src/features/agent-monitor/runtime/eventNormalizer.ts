import type {
  NormalizedRuntimeEvent,
  RuntimeProvenance,
  RuntimeProtocolRecord,
  RuntimeRecordSource,
  RuntimeTokenUsage,
  ThreadRuntimeStatus,
} from "./types";

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asNonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function asFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asNonNegativeNumber(value: unknown): number {
  const number = asFiniteNumber(value);
  return number === null ? 0 : Math.max(0, number);
}

function parseTokenUsage(value: unknown): RuntimeTokenUsage {
  const usage = asRecord(value) ?? {};
  return {
    cacheWriteInputTokens: asNonNegativeNumber(usage.cacheWriteInputTokens),
    cachedInputTokens: asNonNegativeNumber(usage.cachedInputTokens),
    inputTokens: asNonNegativeNumber(usage.inputTokens),
    outputTokens: asNonNegativeNumber(usage.outputTokens),
    reasoningOutputTokens: asNonNegativeNumber(usage.reasoningOutputTokens),
    totalTokens: asNonNegativeNumber(usage.totalTokens),
  };
}

function stableSerialize(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableSerialize).join(",")}]`;
  }
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableSerialize(record[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value) ?? "undefined";
}

function hashString(value: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function getEventParts(record: RuntimeProtocolRecord) {
  if (record.source === "EVENT") {
    const message = asRecord(record.payload.message);
    return {
      method: asNonEmptyString(message?.method) ?? record.label,
      params: asRecord(message?.params) ?? {},
      result: null,
      workspaceId: asNonEmptyString(record.payload.workspace_id),
      serverTimeMs: asFiniteNumber(message?.emittedAtMs),
    };
  }
  return {
    method: record.label.replace(/ response$/, ""),
    params: record.source === "CLIENT" ? record.payload : {},
    result: record.source === "SERVER" ? asRecord(record.payload.result) : null,
    workspaceId:
      asNonEmptyString(record.payload.workspaceId) ??
      asNonEmptyString(record.payload.workspace_id),
    serverTimeMs: null,
  };
}

function makeProvenance(
  record: RuntimeProtocolRecord,
  observedAtMs: number,
  method: string,
  serverTimeMs: number | null,
  suffix: string,
): RuntimeProvenance {
  const keyPayload = stableSerialize({
    source: record.source,
    label: record.label,
    payload: record.payload,
  });
  const eventKey = `${record.source}:${record.label}:${hashString(keyPayload)}:${suffix}`;
  return {
    eventKey,
    method,
    recordSource: record.source as RuntimeRecordSource,
    serverTimeMs,
    observedAtMs,
  };
}

type RuntimeEventWithoutEvidence<T> = T extends NormalizedRuntimeEvent
  ? Omit<T, "eventKey" | "provenance">
  : never;

function withBase(
  event: RuntimeEventWithoutEvidence<NormalizedRuntimeEvent>,
  provenance: RuntimeProvenance,
): NormalizedRuntimeEvent {
  return { ...event, eventKey: provenance.eventKey, provenance } as NormalizedRuntimeEvent;
}

function parseThreadStatus(value: unknown): ThreadRuntimeStatus | null {
  const raw = asNonEmptyString(asRecord(value)?.type ?? value);
  return raw === "active" || raw === "idle" || raw === "notLoaded" ? raw : null;
}

export function normalizeRuntimeRecord(
  record: RuntimeProtocolRecord,
  observedAtMs: number,
): NormalizedRuntimeEvent[] {
  const { method, params, result, workspaceId, serverTimeMs } = getEventParts(record);
  const provenance = (suffix: string) =>
    makeProvenance(record, observedAtMs, method, serverTimeMs, suffix);

  if (record.source === "HYDRATION" && record.label === "app/runtime hydration") {
    const threadId = asNonEmptyString(record.payload.threadId);
    if (!threadId) return [];
    const evidence = provenance("threadHydrated");
    const events: NormalizedRuntimeEvent[] = [
      withBase(
        {
          type: "threadHydrated",
          workspaceId: asNonEmptyString(record.payload.workspaceId),
          threadId,
          parentThreadId: asNonEmptyString(record.payload.parentThreadId),
          status: parseThreadStatus(record.payload.threadStatus),
          createdAtMs: asFiniteNumber(record.payload.createdAtMs),
        },
        evidence,
      ),
    ];
    const activeTurn = asRecord(record.payload.activeTurn);
    const turnId = asNonEmptyString(activeTurn?.turnId);
    const turnStatus = asNonEmptyString(activeTurn?.status);
    if (
      turnId &&
      (turnStatus === "running" ||
        turnStatus === "waiting" ||
        turnStatus === "completed")
    ) {
      const turnEvidence = provenance(`turnHydrated:${turnId}`);
      events.push(
        withBase(
          {
            type: "turnHydrated",
            workspaceId: asNonEmptyString(record.payload.workspaceId),
            threadId,
            turnId,
            status: turnStatus,
            startedAtMs: asFiniteNumber(activeTurn?.startedAtMs),
            completedAtMs: asFiniteNumber(activeTurn?.completedAtMs),
            durationMs: asFiniteNumber(activeTurn?.durationMs),
          },
          turnEvidence,
        ),
      );
    }
    return events;
  }

  if (record.source === "CLIENT" && record.label === "turn/start") {
    const threadId = asNonEmptyString(params.threadId);
    const requestedModel = asNonEmptyString(params.model);
    if (!threadId || !requestedModel) return [];
    const evidence = provenance("turnRequested");
    return [
      withBase(
        { type: "turnRequested", workspaceId, threadId, requestedModel },
        evidence,
      ),
    ];
  }

  if (record.source === "SERVER" && record.label === "thread/start response") {
    const thread = asRecord(result?.thread);
    const threadId = asNonEmptyString(thread?.id);
    const model = asNonEmptyString(result?.model);
    if (!threadId || !model) return [];
    const evidence = provenance("observedModelConfirmed");
    return [
      withBase(
        {
          type: "observedModelConfirmed",
          workspaceId,
          threadId,
          model,
          source: "threadStartResponse",
        },
        evidence,
      ),
    ];
  }

  if (record.source !== "EVENT") return [];

  if (method === "thread/started") {
    const thread = asRecord(params.thread);
    const threadId = asNonEmptyString(thread?.id);
    if (!threadId) return [];
    const parentThreadId = asNonEmptyString(thread?.parentThreadId);
    const createdAtSeconds = asFiniteNumber(thread?.createdAt);
    const evidence = provenance("threadStarted");
    return [
      withBase(
        {
          type: "threadStarted",
          workspaceId,
          threadId,
          parentThreadId,
          status: parseThreadStatus(thread?.status),
          createdAtMs: createdAtSeconds === null ? null : createdAtSeconds * 1_000,
        },
        evidence,
      ),
    ];
  }

  if (method === "thread/settings/updated") {
    const threadId = asNonEmptyString(params.threadId);
    const model = asNonEmptyString(asRecord(params.threadSettings)?.model);
    if (!threadId || !model) return [];
    const evidence = provenance("observedModelConfirmed");
    return [
      withBase(
        {
          type: "observedModelConfirmed",
          workspaceId,
          threadId,
          model,
          source: "threadSettingsUpdated",
        },
        evidence,
      ),
    ];
  }

  if (method === "thread/status/changed") {
    const threadId = asNonEmptyString(params.threadId);
    const status = parseThreadStatus(params.status);
    if (!threadId || !status) return [];
    const evidence = provenance("threadStatusChanged");
    return [
      withBase({ type: "threadStatusChanged", workspaceId, threadId, status }, evidence),
    ];
  }

  if (method === "turn/started") {
    const threadId = asNonEmptyString(params.threadId);
    const turn = asRecord(params.turn);
    const turnId = asNonEmptyString(turn?.id);
    if (!threadId || !turnId) return [];
    const startedAtSeconds = asFiniteNumber(turn?.startedAt);
    const evidence = provenance("turnStarted");
    return [
      withBase(
        {
          type: "turnStarted",
          workspaceId,
          threadId,
          turnId,
          startedAtMs: startedAtSeconds === null ? null : startedAtSeconds * 1_000,
        },
        evidence,
      ),
    ];
  }

  if (method === "turn/completed") {
    const threadId = asNonEmptyString(params.threadId);
    const turn = asRecord(params.turn);
    const turnId = asNonEmptyString(turn?.id);
    if (!threadId || !turnId || turn?.status !== "completed") return [];
    const startedAtSeconds = asFiniteNumber(turn.startedAt);
    const completedAtSeconds = asFiniteNumber(turn.completedAt);
    const evidence = provenance("turnCompleted");
    return [
      withBase(
        {
          type: "turnCompleted",
          workspaceId,
          threadId,
          turnId,
          startedAtMs: startedAtSeconds === null ? null : startedAtSeconds * 1_000,
          completedAtMs: completedAtSeconds === null ? null : completedAtSeconds * 1_000,
          durationMs: asFiniteNumber(turn.durationMs),
        },
        evidence,
      ),
    ];
  }

  if (method === "thread/tokenUsage/updated") {
    const threadId = asNonEmptyString(params.threadId);
    const turnId = asNonEmptyString(params.turnId);
    const tokenUsage = asRecord(params.tokenUsage);
    if (!threadId || !tokenUsage) return [];
    const evidence = provenance("threadTokensUpdated");
    return [
      withBase(
        {
          type: "threadTokensUpdated",
          workspaceId,
          threadId,
          turnId,
          last: parseTokenUsage(tokenUsage.last),
          total: parseTokenUsage(tokenUsage.total),
          modelContextWindow: asFiniteNumber(tokenUsage.modelContextWindow),
        },
        evidence,
      ),
    ];
  }

  if (method === "item/started" || method === "item/completed") {
    const threadId = asNonEmptyString(params.threadId);
    const turnId = asNonEmptyString(params.turnId);
    const item = asRecord(params.item);
    if (!threadId || !item) return [];

    if (
      method === "item/started" &&
      item.type === "subAgentActivity" &&
      item.kind === "started"
    ) {
      const childThreadId = asNonEmptyString(item.agentThreadId);
      const assignmentId = asNonEmptyString(item.id);
      if (!childThreadId || !assignmentId) return [];
      const startedAtMs = asFiniteNumber(params.startedAtMs);
      const spawnedAtMs = startedAtMs ?? serverTimeMs ?? observedAtMs;
      const evidence = {
        ...provenance("assignmentStarted"),
        serverTimeMs: startedAtMs ?? serverTimeMs,
      };
      return [
        withBase(
          {
            type: "assignmentStarted",
            workspaceId,
            assignmentId,
            parentThreadId: threadId,
            childThreadId,
            agentPath: asNonEmptyString(item.agentPath),
            spawnedAtMs,
          },
          evidence,
        ),
      ];
    }

    if (item.type === "collabAgentToolCall" && item.tool === "wait" && turnId) {
      const evidence = provenance(method === "item/started" ? "turnWaiting" : "turnResumed");
      return [
        withBase(
          {
            type: method === "item/started" ? "turnWaiting" : "turnResumed",
            workspaceId,
            threadId,
            turnId,
          },
          evidence,
        ),
      ];
    }

  }

  return [];
}

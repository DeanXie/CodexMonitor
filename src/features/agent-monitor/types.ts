import type { ThreadSummary, ThreadTokenUsage, TokenUsageBreakdown } from "@/types";
import type { GlobalSourceProducerClassification } from "./global-source/types";

export type AgentMonitorStatus =
  | "active"
  | "idle"
  | "notLoaded"
  | "reviewing"
  | "running"
  | "waiting"
  | "completed"
  | "failed"
  | "unavailable";

export type AgentMonitorThreadStatus = {
  isProcessing?: boolean;
  isReviewing?: boolean;
  processingStartedAt?: number | null;
  lastDurationMs?: number | null;
};

export type AgentMonitorWorkspaceOption = {
  id: string;
  label: string;
  path?: string;
};

export type AgentMonitorSourceInfo = {
  sourceKind:
    | "monitor-app-server"
    | "codex-cli-rollout"
    | "historical-rollout-scan";
  temporalClass: "LIVE" | "NEAR_LIVE" | "HISTORICAL";
  freshnessState: "fresh" | "stale" | "settled" | "unknown";
  ageMs: number | null;
  observedAgeMs?: number | null;
  sourceTimestampMs: number | null;
  observedTimestampMs: number | null;
  sourceInstanceId?: string | null;
  sourceGeneration?: string | null;
  freshnessReason?: string | null;
};

export type AgentMonitorNode = {
  threadId: string;
  name: string;
  producer: GlobalSourceProducerClassification;
  modelId: string | null;
  effort: string | null;
  role: string | null;
  isSubagent: boolean;
  status: AgentMonitorStatus;
  runtimeMs: number | null;
  totalTokens: number | null;
  tokenUsage: TokenUsageBreakdown | null;
  source: AgentMonitorSourceInfo;
  modelSource?: AgentMonitorSourceInfo | null;
  children: AgentMonitorNode[];
};

export type AgentMonitorRuntimeThread = Omit<AgentMonitorNode, "children"> & {
  codexHomeIdentity: string | null;
  workspaceId: string | null;
  parentThreadId: string | null;
  createdAtMs: number | null;
  isCurrentEligible: boolean;
};

export type AgentMonitorRuntimeView = {
  roots: AgentMonitorNode[];
  threads: AgentMonitorRuntimeThread[];
};

export type AgentMonitorSessionOption = {
  threadId: string;
  label: string;
  isCurrent: boolean;
};

export type AgentMonitorTreeInput = {
  threads: ThreadSummary[];
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, AgentMonitorThreadStatus | undefined>;
  tokenUsageByThread: Record<string, ThreadTokenUsage | undefined>;
  now: number;
};

import type { ThreadSummary, ThreadTokenUsage, TokenUsageBreakdown } from "@/types";

export type AgentMonitorStatus = "idle" | "reviewing" | "running";

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

export type AgentMonitorNode = {
  threadId: string;
  name: string;
  modelId: string | null;
  effort: string | null;
  role: string | null;
  isSubagent: boolean;
  status: AgentMonitorStatus;
  runtimeMs: number | null;
  totalTokens: number;
  tokenUsage: TokenUsageBreakdown;
  children: AgentMonitorNode[];
};

export type AgentMonitorTreeInput = {
  threads: ThreadSummary[];
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, AgentMonitorThreadStatus | undefined>;
  tokenUsageByThread: Record<string, ThreadTokenUsage | undefined>;
  now: number;
};

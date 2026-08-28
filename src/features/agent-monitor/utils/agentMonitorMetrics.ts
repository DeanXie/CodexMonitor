import type { AgentMonitorNode } from "../types";
import type { AgentMonitorActivityFilter } from "./agentMonitorActivity";

export type AgentMonitorActivityMetric = {
  label: "Active" | "Recorded Active";
  value: number;
  tooltip: string | null;
};

export type AgentMonitorSummary = {
  totalAgents: number;
  activityMetric: AgentMonitorActivityMetric | null;
  totalTokens: number | null;
  primaryModel: string | null;
};

export type AgentMonitorModelUsage = {
  model: string;
  tokens: number;
  sharePercent: number;
};

function flattenNodes(nodes: AgentMonitorNode[]): AgentMonitorNode[] {
  return nodes.flatMap((node) => [node, ...flattenNodes(node.children)]);
}

export function buildAgentMonitorModelUsage(
  roots: AgentMonitorNode[],
): AgentMonitorModelUsage[] {
  const tokensByModel = new Map<string, number>();
  flattenNodes(roots).forEach((node) => {
    if (!node.modelId || !node.totalTokens || node.totalTokens <= 0) return;
    tokensByModel.set(node.modelId, (tokensByModel.get(node.modelId) ?? 0) + node.totalTokens);
  });
  const totalTokens = Array.from(tokensByModel.values()).reduce((total, tokens) => total + tokens, 0);
  if (!totalTokens) return [];
  return Array.from(tokensByModel.entries())
    .map(([model, tokens]) => ({ model, tokens, sharePercent: Math.round((tokens / totalTokens) * 1000) / 10 }))
    .sort((left, right) => right.tokens - left.tokens);
}

export function buildAgentMonitorSummary(
  roots: AgentMonitorNode[],
  activityFilter: AgentMonitorActivityFilter,
): AgentMonitorSummary {
  const nodes = flattenNodes(roots);
  const activeStatuses = new Set(["active", "running", "waiting", "reviewing"]);
  const observedModels = Array.from(
    new Set(nodes.map((node) => node.modelId).filter((model): model is string => Boolean(model))),
  );
  const activityMetric = activityFilter === "settled"
    ? null
    : {
        label: activityFilter === "all" ? "Recorded Active" as const : "Active" as const,
        value: nodes.filter((node) => activeStatuses.has(node.status)
          && (activityFilter === "all" || node.source.freshnessState === "fresh")).length,
        tooltip: activityFilter === "all"
          ? "Includes stale unresolved agents whose last recorded lifecycle was Running or Waiting."
          : null,
      };

  return {
    totalAgents: nodes.length,
    activityMetric,
    totalTokens: roots.length === 1 ? roots[0].totalTokens : null,
    primaryModel: observedModels.length === 1 ? observedModels[0] : null,
  };
}

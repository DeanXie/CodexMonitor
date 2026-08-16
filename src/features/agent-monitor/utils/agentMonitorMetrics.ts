import type { AgentMonitorNode } from "../types";

export type AgentMonitorSummary = {
  totalAgents: number;
  activeAgents: number;
  totalTokens: number;
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
): AgentMonitorSummary {
  const nodes = flattenNodes(roots);
  const tokensByModel = new Map<string, number>();
  nodes.forEach((node) => {
    if (!node.modelId || node.totalTokens === null) {
      return;
    }
    tokensByModel.set(
      node.modelId,
      (tokensByModel.get(node.modelId) ?? 0) + node.totalTokens,
    );
  });
  const primaryModel = Array.from(tokensByModel.entries()).sort(
    ([, left], [, right]) => right - left,
  )[0]?.[0] ?? null;

  return {
    totalAgents: nodes.length,
    activeAgents: nodes.filter((node) => node.status !== "idle").length,
    totalTokens: nodes.reduce((total, node) => total + (node.totalTokens ?? 0), 0),
    primaryModel,
  };
}

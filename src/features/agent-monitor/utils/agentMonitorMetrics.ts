import type { AgentMonitorNode } from "../types";

export type AgentMonitorSummary = {
  totalAgents: number;
  activeAgents: number;
  totalTokens: number;
  primaryModel: string | null;
};

function flattenNodes(nodes: AgentMonitorNode[]): AgentMonitorNode[] {
  return nodes.flatMap((node) => [node, ...flattenNodes(node.children)]);
}

export function buildAgentMonitorSummary(
  roots: AgentMonitorNode[],
): AgentMonitorSummary {
  const nodes = flattenNodes(roots);
  const tokensByModel = new Map<string, number>();
  nodes.forEach((node) => {
    if (!node.modelId) {
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
    totalTokens: nodes.reduce((total, node) => total + node.totalTokens, 0),
    primaryModel,
  };
}

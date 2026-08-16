import type { AgentMonitorNode } from "../types";
import { AgentTreeNode } from "./AgentTreeNode";

export function AgentCallTree({ roots }: { roots: AgentMonitorNode[] }) {
  if (!roots.length) {
    return <div className="agent-monitor-empty">No agent threads are available yet.</div>;
  }
  return <ul className="agent-monitor-tree">{roots.map((root) => <AgentTreeNode key={root.threadId} node={root} />)}</ul>;
}

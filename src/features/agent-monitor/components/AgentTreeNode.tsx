import { useState } from "react";
import type { AgentMonitorNode } from "../types";

function formatRuntime(runtimeMs: number | null) {
  if (runtimeMs === null) return "—";
  const seconds = Math.floor(runtimeMs / 1_000);
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

export function AgentTreeNode({ node, depth = 0 }: { node: AgentMonitorNode; depth?: number }) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;
  return (
    <li className="agent-monitor-tree-node" style={{ "--agent-depth": depth } as React.CSSProperties}>
      <div className="agent-monitor-agent-card">
        {hasChildren ? (
          <button type="button" className="agent-monitor-toggle" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
            {expanded ? "−" : "+"}
          </button>
        ) : <span className="agent-monitor-toggle-placeholder" />}
        <span className={`agent-monitor-status is-${node.status}`} aria-label={node.status} />
        <div className="agent-monitor-agent-identity">
          <strong>{node.name}</strong>
          <span>{node.isSubagent ? node.role ?? "Sub Agent" : "Main Agent"}</span>
        </div>
        <div className="agent-monitor-agent-metric"><span>Model</span><strong>{node.modelId ?? "Unknown"}</strong></div>
        <div className="agent-monitor-agent-metric"><span>Status</span><strong>{node.status === "running" ? "Running" : node.status === "reviewing" ? "Reviewing" : "Idle"}</strong></div>
        <div className="agent-monitor-agent-metric"><span>Runtime</span><strong>{formatRuntime(node.runtimeMs)}</strong></div>
        <div className="agent-monitor-agent-metric"><span>Tokens</span><strong>{node.totalTokens.toLocaleString()}</strong></div>
      </div>
      {hasChildren && expanded ? <ul>{node.children.map((child) => <AgentTreeNode key={child.threadId} node={child} depth={depth + 1} />)}</ul> : null}
    </li>
  );
}

import { useState } from "react";
import type { AgentMonitorNode } from "../types";

function formatRuntime(runtimeMs: number | null) {
  if (runtimeMs === null) return "—";
  const seconds = Math.floor(runtimeMs / 1_000);
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

function formatTokens(value: number | null | undefined) {
  return value === null || value === undefined ? "unavailable" : value.toLocaleString();
}

function formatAge(value: number | null) {
  if (value === null) return "unavailable";
  if (value < 1_000) return `${Math.round(value)} ms`;
  if (value < 60_000) return `${Math.floor(value / 1_000)}s`;
  return `${Math.floor(value / 60_000)}m`;
}

function formatSource(source: AgentMonitorNode["source"]) {
  const temporalClass = source.temporalClass.replace("_", " ");
  if (source.temporalClass === "HISTORICAL") return "HISTORICAL";
  if (source.freshnessState !== "fresh") {
    return `${temporalClass} · ${source.freshnessState}`;
  }
  if (source.temporalClass === "LIVE") return "LIVE";
  const ageMs = source.ageMs;
  if (ageMs === null) return temporalClass;
  if (ageMs < 1_000) return `${temporalClass} · ${Math.round(ageMs)} ms`;
  if (ageMs < 60_000) return `${temporalClass} · ${Math.floor(ageMs / 1_000)}s`;
  return `${temporalClass} · ${Math.floor(ageMs / 60_000)}m`;
}

function formatProvenance(source: AgentMonitorNode["source"]) {
  return [
    `source: ${source.sourceKind}`,
    `class: ${source.temporalClass}`,
    `instance: ${source.sourceInstanceId ?? "unavailable"}`,
    `generation: ${source.sourceGeneration ?? "unavailable"}`,
    `source timestamp: ${source.sourceTimestampMs ?? "unavailable"}`,
    `observed timestamp: ${source.observedTimestampMs ?? "unavailable"}`,
    `source age: ${formatAge(source.ageMs)}`,
    `observed age: ${formatAge(source.observedAgeMs ?? null)}`,
    `freshness: ${source.freshnessState}`,
    `reason: ${source.freshnessReason ?? "unavailable"}`,
  ].join("; ");
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
          <strong title={node.name}>{node.name}</strong>
          <span>{node.isSubagent ? node.role ?? "Sub Agent" : "Main Agent"}</span>
          <span className={`agent-monitor-source-badge is-${node.source.temporalClass.toLowerCase()}`} title={formatProvenance(node.source)}>{formatSource(node.source)}</span>
        </div>
        <div className="agent-monitor-agent-overview">
          <div className="agent-monitor-agent-metric agent-monitor-model"><span>Model</span><strong title={node.modelId && node.modelSource ? `${node.modelId}; ${formatProvenance(node.modelSource)}` : node.modelId ?? "unavailable"}>{node.modelId ?? "unavailable"}</strong></div>
          <div className="agent-monitor-agent-metric"><span>Status</span><strong>{node.status === "notLoaded" ? "Not loaded" : node.status === "unavailable" ? "unavailable" : `${node.status.charAt(0).toUpperCase()}${node.status.slice(1)}`}</strong></div>
          <div className="agent-monitor-agent-metric"><span>Runtime</span><strong>{formatRuntime(node.runtimeMs)}</strong></div>
        </div>
        <div className="agent-monitor-token-grid">
          <div className="agent-monitor-agent-metric"><span>Input</span><strong>{formatTokens(node.tokenUsage?.inputTokens)}</strong></div>
          <div className="agent-monitor-agent-metric"><span>Output</span><strong>{formatTokens(node.tokenUsage?.outputTokens)}</strong></div>
          <div className="agent-monitor-agent-metric"><span>Cached</span><strong>{formatTokens(node.tokenUsage?.cachedInputTokens)}</strong></div>
          <div className="agent-monitor-agent-metric"><span>Total</span><strong>{formatTokens(node.totalTokens)}</strong></div>
        </div>
      </div>
      {hasChildren && expanded ? <ul>{node.children.map((child) => <AgentTreeNode key={child.threadId} node={child} depth={depth + 1} />)}</ul> : null}
    </li>
  );
}

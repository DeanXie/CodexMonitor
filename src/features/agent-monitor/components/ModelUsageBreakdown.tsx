import type { LocalUsageSnapshot } from "@/types";
import type { AgentMonitorModelUsage } from "../utils/agentMonitorMetrics";

export function ModelUsageBreakdown({ snapshot, models: filteredModels }: { snapshot: LocalUsageSnapshot | null; models?: AgentMonitorModelUsage[] | null }) {
  const models = filteredModels ?? snapshot?.topModels ?? [];
  const sourceLabel = filteredModels ? "Filtered live thread usage" : "Local session history";
  return <section className="agent-monitor-models" aria-label="Model usage proportions">
    <div className="agent-monitor-section-heading"><h2>Model usage</h2><span>{sourceLabel}</span></div>
    {models.length ? <ul>{models.map((model) => <li key={model.model}><span>{model.model}</span><span>{model.tokens.toLocaleString()} tokens · <b>{model.sharePercent}%</b></span><div><i style={{ width: `${Math.min(100, Math.max(0, model.sharePercent))}%` }} /></div></li>)}</ul> : <div className="agent-monitor-empty">No historical model usage is available yet.</div>}
  </section>;
}

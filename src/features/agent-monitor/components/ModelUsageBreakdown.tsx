import type { LocalUsageSnapshot } from "@/types";

export function ModelUsageBreakdown({ snapshot }: { snapshot: LocalUsageSnapshot | null }) {
  const models = snapshot?.topModels ?? [];
  return <section className="agent-monitor-models" aria-label="Model usage proportions">
    <div className="agent-monitor-section-heading"><h2>Model usage</h2><span>Local session history</span></div>
    {models.length ? <ul>{models.map((model) => <li key={model.model}><span>{model.model}</span><span>{model.tokens.toLocaleString()} tokens · <b>{model.sharePercent}%</b></span><div><i style={{ width: `${Math.min(100, Math.max(0, model.sharePercent))}%` }} /></div></li>)}</ul> : <div className="agent-monitor-empty">No historical model usage is available yet.</div>}
  </section>;
}

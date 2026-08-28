import type { AgentMonitorSummary as Summary } from "../utils/agentMonitorMetrics";

export function AgentMonitorSummary({ summary }: { summary: Summary }) {
  const cards = [
    { label: "Agents", value: summary.totalAgents, tooltip: null },
    ...(summary.activityMetric ? [summary.activityMetric] : []),
    { label: "Root Thread Tokens", value: summary.totalTokens === null ? "unavailable" : summary.totalTokens.toLocaleString(), tooltip: null },
    { label: "Primary model", value: summary.primaryModel ?? "unavailable", tooltip: null },
  ];
  return (
    <div className="agent-monitor-summary" aria-label="Agent Monitor summary">
      {cards.map((card) => (
        <div className="agent-monitor-summary-card" key={card.label} title={card.tooltip ?? undefined}>
          <span>{card.label}</span>
          <strong>{card.value}</strong>
        </div>
      ))}
    </div>
  );
}

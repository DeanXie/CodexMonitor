import type { AgentMonitorSummary as Summary } from "../utils/agentMonitorMetrics";

export function AgentMonitorSummary({ summary, historyNotLinked = false }: { summary: Summary; historyNotLinked?: boolean }) {
  const cards = [
    ["Agents", summary.totalAgents],
    ["Active", summary.activeAgents],
    ["Tokens", historyNotLinked ? "not linked" : summary.totalTokens.toLocaleString()],
    ["Primary model", historyNotLinked ? "not linked" : summary.primaryModel ?? "unavailable"],
  ];
  return (
    <div className="agent-monitor-summary" aria-label="Agent Monitor summary">
      {cards.map(([label, value]) => (
        <div className="agent-monitor-summary-card" key={label}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  );
}

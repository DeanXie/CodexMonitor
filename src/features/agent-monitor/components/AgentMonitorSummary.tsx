import type { AgentMonitorSummary as Summary } from "../utils/agentMonitorMetrics";

export function AgentMonitorSummary({ summary }: { summary: Summary }) {
  const cards = [
    ["Agents", summary.totalAgents],
    ["Active", summary.activeAgents],
    ["Tokens", summary.totalTokens.toLocaleString()],
    ["Primary model", summary.primaryModel ?? "Unknown"],
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

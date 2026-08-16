import { useEffect, useMemo, useState } from "react";
import type { LocalUsageSnapshot, ThreadSummary, ThreadTokenUsage } from "@/types";
import type { AgentMonitorThreadStatus } from "../types";
import { buildAgentMonitorSummary } from "../utils/agentMonitorMetrics";
import { buildAgentMonitorForest } from "../utils/agentMonitorTree";
import { AgentCallTree } from "./AgentCallTree";
import { AgentMonitorSummary } from "./AgentMonitorSummary";
import { ModelUsageBreakdown } from "./ModelUsageBreakdown";

type AgentMonitorPageProps = {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, AgentMonitorThreadStatus | undefined>;
  tokenUsageByThread: Record<string, ThreadTokenUsage | undefined>;
  localUsageSnapshot: LocalUsageSnapshot | null;
  now?: number;
  onBack?: () => void;
};

export function AgentMonitorPage({ threadsByWorkspace, threadParentById, threadStatusById, tokenUsageByThread, localUsageSnapshot, now: fixedNow, onBack }: AgentMonitorPageProps) {
  const [clockNow, setClockNow] = useState(() => Date.now());
  useEffect(() => {
    if (fixedNow !== undefined) return;
    const interval = window.setInterval(() => setClockNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [fixedNow]);
  const now = fixedNow ?? clockNow;
  const threads = useMemo(() => Object.values(threadsByWorkspace).flat(), [threadsByWorkspace]);
  const forest = useMemo(() => buildAgentMonitorForest({ threads, threadParentById, threadStatusById, tokenUsageByThread, now }), [now, threadParentById, threadStatusById, threads, tokenUsageByThread]);
  const summary = useMemo(() => buildAgentMonitorSummary(forest), [forest]);
  return <main className="agent-monitor-page">
    <header><p>Live workspace activity</p><h1>Agent Monitor</h1><span>Main Agent and Sub Agent hierarchy from current thread state.</span>{onBack ? <button type="button" onClick={onBack}>Back to Home</button> : null}</header>
    <AgentMonitorSummary summary={summary} />
    <section className="agent-monitor-call-tree"><div className="agent-monitor-section-heading"><h2>Agent call tree</h2><span>{summary.totalAgents} visible</span></div><AgentCallTree roots={forest} /></section>
    <ModelUsageBreakdown snapshot={localUsageSnapshot} />
  </main>;
}

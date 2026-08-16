import { useEffect, useMemo, useState } from "react";
import type { LocalUsageSnapshot, ThreadSummary, ThreadTokenUsage } from "@/types";
import type { AgentMonitorThreadStatus, AgentMonitorWorkspaceOption } from "../types";
import { buildAgentMonitorSummary } from "../utils/agentMonitorMetrics";
import { filterAgentMonitorThreads } from "../utils/agentMonitorFilters";
import { buildAgentMonitorForest } from "../utils/agentMonitorTree";
import { AgentCallTree } from "./AgentCallTree";
import { AgentMonitorSummary } from "./AgentMonitorSummary";
import { ModelUsageBreakdown } from "./ModelUsageBreakdown";
import { useLocalUsage } from "@/features/home/hooks/useLocalUsage";

type AgentMonitorPageProps = {
  threadsByWorkspace: Record<string, ThreadSummary[]>;
  threadParentById: Record<string, string>;
  threadStatusById: Record<string, AgentMonitorThreadStatus | undefined>;
  tokenUsageByThread: Record<string, ThreadTokenUsage | undefined>;
  localUsageSnapshot: LocalUsageSnapshot | null;
  workspaceOptions?: AgentMonitorWorkspaceOption[];
  now?: number;
  onBack?: () => void;
};

export function AgentMonitorPage({ threadsByWorkspace, threadParentById, threadStatusById, tokenUsageByThread, localUsageSnapshot, workspaceOptions = [], now: fixedNow, onBack }: AgentMonitorPageProps) {
  const [clockNow, setClockNow] = useState(() => Date.now());
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  useEffect(() => {
    if (fixedNow !== undefined) return;
    const interval = window.setInterval(() => setClockNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [fixedNow]);
  const now = fixedNow ?? clockNow;
  const workspaceThreads = useMemo(() => workspaceId ? threadsByWorkspace[workspaceId] ?? [] : Object.values(threadsByWorkspace).flat(), [threadsByWorkspace, workspaceId]);
  const selectedWorkspace = useMemo(() => workspaceOptions.find((workspace) => workspace.id === workspaceId) ?? null, [workspaceId, workspaceOptions]);
  const { snapshot: workspaceUsageSnapshot } = useLocalUsage(Boolean(workspaceId), selectedWorkspace?.path ?? null);
  const sessionOptions = useMemo(() => workspaceThreads.filter((thread) => !threadParentById[thread.id]), [threadParentById, workspaceThreads]);
  const selectedSession = useMemo(() => sessionOptions.find((thread) => thread.id === sessionId) ?? null, [sessionId, sessionOptions]);
  const { snapshot: sessionUsageSnapshot } = useLocalUsage(Boolean(sessionId), selectedWorkspace?.path ?? null, sessionId);
  useEffect(() => {
    if (sessionId && !sessionOptions.some((thread) => thread.id === sessionId)) setSessionId(null);
  }, [sessionId, sessionOptions]);
  const threads = useMemo(() => filterAgentMonitorThreads({ threadsByWorkspace, threadParentById, workspaceId, sessionId }), [sessionId, threadParentById, threadsByWorkspace, workspaceId]);
  const liveForest = useMemo(() => buildAgentMonitorForest({ threads, threadParentById, threadStatusById, tokenUsageByThread, now }), [now, threadParentById, threadStatusById, threads, tokenUsageByThread]);
  const historicalUsage = useMemo(() => {
    if (!selectedSession || sessionUsageSnapshot?.sessionLinked !== true) return null;
    return sessionUsageSnapshot.days.reduce((total, day) => ({
      inputTokens: total.inputTokens + day.inputTokens,
      cachedInputTokens: total.cachedInputTokens + day.cachedInputTokens,
      outputTokens: total.outputTokens + day.outputTokens,
      reasoningOutputTokens: total.reasoningOutputTokens,
      totalTokens: total.totalTokens + day.totalTokens,
    }), { inputTokens: 0, cachedInputTokens: 0, outputTokens: 0, reasoningOutputTokens: 0, totalTokens: 0 });
  }, [selectedSession, sessionUsageSnapshot]);
  const forest = useMemo(() => {
    if (!selectedSession || !historicalUsage) return liveForest;
    const primaryModel = sessionUsageSnapshot?.topModels[0]?.model ?? null;
    return liveForest.map((node) => node.threadId === selectedSession.id ? { ...node, modelId: node.modelId ?? primaryModel, totalTokens: historicalUsage.totalTokens, tokenUsage: historicalUsage } : node);
  }, [historicalUsage, liveForest, selectedSession, sessionUsageSnapshot]);
  const summary = useMemo(() => buildAgentMonitorSummary(forest), [forest]);
  const filteredModels = useMemo(() => sessionId ? sessionUsageSnapshot?.sessionLinked === true ? sessionUsageSnapshot.topModels : null : null, [sessionId, sessionUsageSnapshot]);
  const usageSnapshot = sessionId ? sessionUsageSnapshot : workspaceId ? workspaceUsageSnapshot : localUsageSnapshot;
  return <main className="agent-monitor-page">
    <header><p>Live workspace activity</p><h1>Agent Monitor</h1><span>Main Agent and Sub Agent hierarchy from current thread state.</span>{onBack ? <button type="button" onClick={onBack}>Back to Home</button> : null}</header>
    <section className="agent-monitor-filters" aria-label="Agent monitor filters">
      <label>Workspace<select aria-label="Workspace" value={workspaceId ?? ""} onChange={(event) => { setWorkspaceId(event.target.value || null); setSessionId(null); }}><option value="">All Workspaces</option>{workspaceOptions.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.label}</option>)}</select></label>
      <label>Session<select aria-label="Session" value={sessionId ?? ""} onChange={(event) => setSessionId(event.target.value || null)}><option value="">All Sessions</option>{sessionOptions.map((thread) => <option key={thread.id} value={thread.id}>{thread.name}</option>)}</select></label>
    </section>
    {selectedSession ? <div className="agent-monitor-session-details"><strong>{selectedSession.name}</strong><span>Created: {selectedSession.createdAt ? new Date(selectedSession.createdAt).toLocaleString() : "unavailable"}</span>{sessionUsageSnapshot?.sessionLinked === false ? <span>History: not linked</span> : null}</div> : null}
    <AgentMonitorSummary summary={summary} historyNotLinked={Boolean(sessionId && sessionUsageSnapshot?.sessionLinked === false)} />
    <section className="agent-monitor-call-tree"><div className="agent-monitor-section-heading"><h2>Agent call tree</h2><span>{summary.totalAgents} visible</span></div><AgentCallTree roots={forest} /></section>
    <ModelUsageBreakdown snapshot={usageSnapshot} models={filteredModels} />
  </main>;
}

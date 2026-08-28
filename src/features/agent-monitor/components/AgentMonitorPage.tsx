import { useEffect, useMemo, useRef, useState } from "react";
import type { LocalUsageSnapshot } from "@/types";
import { useLocalUsage } from "@/features/home/hooks/useLocalUsage";
import type { AgentRuntimeStore } from "../runtime";
import {
  EMPTY_GLOBAL_SOURCE_SNAPSHOT,
  type GlobalSourceSnapshot,
} from "../global-source/types";
import type { AgentMonitorWorkspaceOption } from "../types";
import { buildAgentMonitorSummary } from "../utils/agentMonitorMetrics";
import type { AgentMonitorActivityFilter } from "../utils/agentMonitorActivity";
import { selectAgentMonitorSessionOptions } from "../utils/agentRuntimeSelector";
import {
  selectUnifiedAgentMonitorView,
  type AgentMonitorSourceFilter,
} from "../utils/globalSourceSelector";
import { AgentCallTree } from "./AgentCallTree";
import { AgentMonitorSummary } from "./AgentMonitorSummary";
import { ModelUsageBreakdown } from "./ModelUsageBreakdown";

type AgentMonitorPageProps = {
  runtimeState: AgentRuntimeStore;
  globalSourceSnapshot?: GlobalSourceSnapshot;
  localUsageSnapshot: LocalUsageSnapshot | null;
  workspaceOptions?: AgentMonitorWorkspaceOption[];
  now?: number;
  onBack?: () => void;
  variant?: "page" | "split";
  onClose?: () => void;
  currentThreadId?: string | null;
  threadTitlesById?: Record<string, string>;
  canClearLiveRuntime?: boolean;
  activeRuntimeTurnCount?: number;
  onClearLiveRuntime?: () => void;
  excludedThreadIds?: ReadonlySet<string>;
};

export function AgentMonitorPage({
  runtimeState,
  globalSourceSnapshot = EMPTY_GLOBAL_SOURCE_SNAPSHOT,
  localUsageSnapshot,
  workspaceOptions = [],
  now: fixedNow,
  onBack,
  variant = "page",
  onClose,
  currentThreadId = null,
  threadTitlesById = {},
  canClearLiveRuntime = true,
  activeRuntimeTurnCount = 0,
  onClearLiveRuntime,
  excludedThreadIds = new Set<string>(),
}: AgentMonitorPageProps) {
  const [clockNow, setClockNow] = useState(() => Date.now());
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [sourceFilter, setSourceFilter] = useState<AgentMonitorSourceFilter>("all");
  const [activityFilter, setActivityFilter] = useState<AgentMonitorActivityFilter>("active-fresh");
  const [historyWorkspaceId, setHistoryWorkspaceId] = useState<string | null>(null);
  const [historySessionId, setHistorySessionId] = useState<string | null>(null);
  const manualSessionSelectionRef = useRef(false);
  const autoSelectedCurrentRef = useRef<string | null>(null);
  useEffect(() => {
    if (fixedNow !== undefined) return;
    const interval = window.setInterval(() => setClockNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [fixedNow]);
  const now = fixedNow ?? clockNow;
  const unfilteredView = useMemo(
    () => selectUnifiedAgentMonitorView(runtimeState, globalSourceSnapshot, now, {
      excludedThreadIds,
      sourceFilter,
      activityFilter,
      currentThreadId,
    }),
    [activityFilter, currentThreadId, excludedThreadIds, globalSourceSnapshot, now, runtimeState, sourceFilter],
  );
  const historicalWorkspace = useMemo(
    () => workspaceOptions.find((workspace) => workspace.id === historyWorkspaceId) ?? null,
    [historyWorkspaceId, workspaceOptions],
  );
  const { snapshot: workspaceUsageSnapshot } = useLocalUsage(
    Boolean(historyWorkspaceId),
    historicalWorkspace?.path ?? null,
  );
  const sessionSelection = useMemo(
    () => selectAgentMonitorSessionOptions(unfilteredView.threads, {
      currentThreadId,
      workspaceId,
      titlesByThreadId: threadTitlesById,
    }),
    [currentThreadId, threadTitlesById, unfilteredView.threads, workspaceId],
  );
  const sessionOptions = sessionSelection.options;
  const selectedSession = useMemo(
    () => unfilteredView.threads.find((thread) => thread.threadId === sessionId) ?? null,
    [sessionId, unfilteredView.threads],
  );
  const { snapshot: sessionUsageSnapshot } = useLocalUsage(
    Boolean(historySessionId),
    historicalWorkspace?.path ?? null,
    historySessionId,
  );
  useEffect(() => {
    manualSessionSelectionRef.current = false;
    autoSelectedCurrentRef.current = null;
    setSessionId(null);
  }, [currentThreadId]);
  useEffect(() => {
    if (
      !currentThreadId ||
      !sessionSelection.currentObserved ||
      manualSessionSelectionRef.current ||
      autoSelectedCurrentRef.current === currentThreadId
    ) {
      return;
    }
    setSessionId(currentThreadId);
    setHistorySessionId(currentThreadId);
    autoSelectedCurrentRef.current = currentThreadId;
  }, [currentThreadId, sessionSelection.currentObserved]);
  useEffect(() => {
    if (sessionId && !sessionOptions.some((thread) => thread.threadId === sessionId)) {
      setSessionId(null);
      setHistorySessionId(null);
    }
  }, [sessionId, sessionOptions]);
  const forest = useMemo(
    () => selectUnifiedAgentMonitorView(
      runtimeState,
      globalSourceSnapshot,
      now,
      { workspaceId, sessionId, excludedThreadIds, sourceFilter, activityFilter, currentThreadId },
    ).roots,
    [activityFilter, currentThreadId, excludedThreadIds, globalSourceSnapshot, now, runtimeState, sessionId, sourceFilter, workspaceId],
  );
  const summary = useMemo(() => buildAgentMonitorSummary(forest, activityFilter), [activityFilter, forest]);
  const filteredModels = useMemo(
    () => historySessionId && sessionUsageSnapshot?.sessionLinked === true
      ? sessionUsageSnapshot.topModels
      : null,
    [historySessionId, sessionUsageSnapshot],
  );
  const usageSnapshot = historySessionId
    ? sessionUsageSnapshot
    : historyWorkspaceId
      ? workspaceUsageSnapshot
      : localUsageSnapshot;
  const clearBlockedTitle = activeRuntimeTurnCount === 1
    ? "Cannot clear while 1 Runtime turn is running or waiting."
    : `Cannot clear while ${activeRuntimeTurnCount} Runtime turns are running or waiting.`;
  const handleClearLiveRuntime = () => {
    if (!canClearLiveRuntime || !onClearLiveRuntime) return;
    onClearLiveRuntime();
    setWorkspaceId(null);
    setSessionId(null);
    manualSessionSelectionRef.current = false;
    autoSelectedCurrentRef.current = null;
  };

  return <main className={`agent-monitor-page${variant === "split" ? " is-split" : ""}`}>
    <header><p>Live workspace activity</p><h1>Agent Monitor</h1><span>Main Agent and Sub Agent hierarchy from observed Runtime events.</span><div className="agent-monitor-header-actions">{onClearLiveRuntime ? <button type="button" onClick={handleClearLiveRuntime} disabled={!canClearLiveRuntime} title={canClearLiveRuntime ? "Clears Monitor LIVE state only. CLI/rollout sources are preserved." : `${clearBlockedTitle} Clears Monitor LIVE state only; CLI/rollout sources are preserved.`}>Clear Live Runtime</button> : null}{variant === "split" && onClose ? <button type="button" onClick={onClose} aria-label="Close Agent Monitor">Close</button> : onBack ? <button type="button" onClick={onBack} aria-label="Home">← Home</button> : null}</div></header>
    <section className="agent-monitor-filters" aria-label="Agent monitor filters">
      <label>Source<select aria-label="Source" value={sourceFilter} onChange={(event) => { setSourceFilter(event.target.value as AgentMonitorSourceFilter); setSessionId(null); setHistorySessionId(null); manualSessionSelectionRef.current = true; }}><option value="all">All Sources</option><option value="monitor-live">Monitor LIVE</option><option value="cli-near-live">CLI NEAR LIVE</option></select></label>
      <label>Activity<select aria-label="Activity" value={activityFilter} onChange={(event) => { setActivityFilter(event.target.value as AgentMonitorActivityFilter); setSessionId(null); setHistorySessionId(null); manualSessionSelectionRef.current = true; }}><option value="active-fresh">Active / Fresh</option><option value="all">All</option><option value="settled">Settled</option></select></label>
      <label>Workspace<select aria-label="Workspace" value={workspaceId ?? ""} onChange={(event) => { const nextWorkspaceId = event.target.value || null; setWorkspaceId(nextWorkspaceId); setHistoryWorkspaceId(nextWorkspaceId); setSessionId(null); setHistorySessionId(null); manualSessionSelectionRef.current = true; }}><option value="">All Workspaces</option>{workspaceOptions.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.label}</option>)}</select></label>
      <label>Session<select aria-label="Session" value={sessionId ?? ""} onChange={(event) => { const nextSessionId = event.target.value || null; setSessionId(nextSessionId); setHistorySessionId(nextSessionId); manualSessionSelectionRef.current = true; }}><option value="">All Sessions</option>{sessionOptions.map((option) => <option key={option.threadId} value={option.threadId}>{option.label}</option>)}</select></label>
    </section>
    {currentThreadId && !sessionSelection.currentObserved ? <div className="agent-monitor-current-unobserved">Current session not observed yet</div> : null}
    {selectedSession ? <div className="agent-monitor-session-details"><strong>{selectedSession.name}</strong><span>Created: {selectedSession.createdAtMs ? new Date(selectedSession.createdAtMs).toLocaleString() : "unavailable"}</span>{sessionUsageSnapshot?.sessionLinked === false ? <span>History: not linked</span> : null}</div> : null}
    <AgentMonitorSummary summary={summary} />
    <section className="agent-monitor-call-tree" aria-label="Live Agent Runtime"><div className="agent-monitor-section-heading"><h2>Live agent call tree</h2><span>{summary.totalAgents} visible</span></div><AgentCallTree roots={forest} /></section>
    <ModelUsageBreakdown snapshot={usageSnapshot} models={filteredModels} />
  </main>;
}

# Agent Monitor V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only Agent Monitor page that visualizes Main Agent to Sub Agent relationships and shows each agent's model, status, runtime, and current token usage.

**Architecture:** Derive a monitor-specific view model entirely from existing frontend thread state: `threadsByWorkspace`, `threadParentById`, `threadStatusById`, and `tokenUsageByThread`. Render the tree and metrics in a new feature module, and integrate it through the existing primary layout; do not add IPC commands or modify app-server transport.

**Tech Stack:** React 19, TypeScript, Vitest, existing CodexMonitor design tokens and feature aliases.

## Global Constraints

- Do not modify Codex app-server, Tauri IPC, Rust shared cores, or daemon RPC.
- Preserve all existing thread/sidebar/home behavior.
- Model usage ratio in V1 consumes the existing `LocalUsageSnapshot.topModels` result and is labeled as local session history.
- Write and run tests before each production implementation change.

---

### Task 1: Build the pure Agent Monitor tree and metrics model

**Files:**
- Create: `src/features/agent-monitor/types.ts`
- Create: `src/features/agent-monitor/utils/agentMonitorTree.ts`
- Create: `src/features/agent-monitor/utils/agentMonitorTree.test.ts`
- Create: `src/features/agent-monitor/utils/agentMonitorMetrics.ts`
- Create: `src/features/agent-monitor/utils/agentMonitorMetrics.test.ts`

**Interfaces:**
- Consumes: `ThreadSummary`, `ThreadTokenUsage`, and existing `threadParentById` / `threadStatusById` state.
- Produces: `AgentMonitorNode`, `buildAgentMonitorForest`, and `buildAgentMonitorSummary`.

- [ ] Write tests for nested parent-child trees, orphaned subagents, and cycle-safe traversal.
- [ ] Verify tests fail because Agent Monitor model functions do not exist.
- [ ] Implement minimal pure tree builder and summary metrics functions.
- [ ] Run focused utility tests and verify green.

### Task 2: Add Agent Monitor view-model hook and visual components

**Files:**
- Create: `src/features/agent-monitor/hooks/useAgentMonitorViewModel.ts`
- Create: `src/features/agent-monitor/components/AgentMonitorSummary.tsx`
- Create: `src/features/agent-monitor/components/AgentCallTree.tsx`
- Create: `src/features/agent-monitor/components/AgentTreeNode.tsx`
- Create: `src/features/agent-monitor/components/ModelUsageBreakdown.tsx`
- Create: `src/features/agent-monitor/components/AgentMonitorPage.tsx`
- Create: matching `*.test.tsx` files.

**Interfaces:**
- Consumes: the Task 1 pure model, `LocalUsageSnapshot`, and existing design-system styling conventions.
- Produces: `AgentMonitorPage` with no side effects or backend calls.

- [ ] Write component tests for model/status/runtime/token rendering and the empty state.
- [ ] Verify tests fail because the page/components do not exist.
- [ ] Implement components with local expand/collapse state and one-second active-runtime refresh.
- [ ] Run focused component tests and verify green.

### Task 3: Integrate the read-only page into the primary app layout

**Files:**
- Modify: `src/features/app/components/MainApp.tsx`
- Modify: `src/features/app/hooks/useMainAppLayoutSurfaces.ts`
- Modify: relevant layout type files and focused tests discovered during integration.

**Interfaces:**
- Consumes: existing thread state and local usage snapshot already owned by `MainApp`.
- Produces: an Agent Monitor navigation destination without changing Tauri, daemon, or app-server behavior.

- [ ] Write an integration test proving navigation renders the page while existing tabs remain available.
- [ ] Verify it fails before layout integration.
- [ ] Add the smallest compatible tab/navigation surface and pass existing data as props.
- [ ] Run integration tests, `npm run typecheck`, and the frontend test suite.

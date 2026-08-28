# Phase 2.4 CLI Near-Live Agent Monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display external Codex CLI rollout sessions in Agent Monitor as canonical NEAR LIVE agents without changing Phase 1 Runtime semantics.

**Architecture:** Add immutable canonical snapshots to Source Authority Registry, cache and emit revisioned snapshots from the Tauri-owned watcher, mirror them in one MainApp-scoped frontend store, and merge them with Phase 1 Runtime only in the Agent Monitor selector.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-08-26-phase-2-4-cli-near-live-agent-monitor-design.md`

## Global Constraints

- Do not modify Phase 1 Runtime state or Event Normalizer semantics.
- Do not add rollout records to the app-server additive stream.
- Never add token snapshots across source lanes.
- Historical-only records do not drive the live tree.
- Preserve source timestamps separately from observed timestamps.

---

### Task 1: Canonical Source Snapshot

**Files:**
- Modify: `src-tauri/src/shared/global_sources_core/source_registry.rs`
- Modify: `src-tauri/src/shared/global_sources_core/rollout_watcher.rs`
- Modify: `src-tauri/src/global_sources/app_server_live.rs`
- Test: `src-tauri/src/shared/global_sources_core/tests.rs`

**Interfaces:**
- Produces: `SourceAuthorityRegistry::snapshot() -> CanonicalSourceSnapshot` with canonical threads, turns, parent relation, resolved fields, provenance, and lane counts.

- [ ] Add failing tests for rollout-only identity, parent/turn evidence, paired LIVE authority, and non-additive tokens.
- [ ] Run focused Rust tests and confirm the snapshot API is missing.
- [ ] Implement metadata retention and immutable serialization.
- [ ] Run focused Rust tests until green.

### Task 2: Tauri Snapshot Command and Update Event

**Files:**
- Create: `src-tauri/src/global_sources/snapshot.rs`
- Modify: `src-tauri/src/global_sources/runtime.rs`
- Modify: `src-tauri/src/global_sources/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: Tauri command `global_source_snapshot` and event `global-source-snapshot-updated` carrying `GlobalSourceSnapshot`.

- [ ] Add failing runtime tests for revisioning, immutable reads, and unchanged-snapshot suppression.
- [ ] Implement the cached snapshot publisher and command.
- [ ] Publish after watcher reconciliation/live ingest without exposing the mutable registry.
- [ ] Run focused Rust tests until green.

### Task 3: Global Source View Store

**Files:**
- Modify: `src/services/tauri.ts`
- Modify: `src/services/tauri.test.ts`
- Modify: `src/services/events.ts`
- Modify: `src/services/events.test.ts`
- Create: `src/features/agent-monitor/global-source/types.ts`
- Create: `src/features/agent-monitor/hooks/useGlobalSourceViewStore.ts`
- Create: `src/features/agent-monitor/hooks/useGlobalSourceViewStore.test.tsx`

**Interfaces:**
- Produces: one MainApp-scoped store that fetches the initial snapshot, subscribes once, and accepts only newer revisions.

- [ ] Add failing IPC, event-hub, and revision/race tests.
- [ ] Implement TypeScript contracts, wrappers, and the store.
- [ ] Run focused frontend tests until green.

### Task 4: Unified Agent Monitor Projection

**Files:**
- Modify: `src/features/agent-monitor/types.ts`
- Create: `src/features/agent-monitor/utils/globalSourceSelector.ts`
- Create: `src/features/agent-monitor/utils/globalSourceSelector.test.ts`
- Modify: `src/features/agent-monitor/utils/agentRuntimeSelector.ts`
- Modify: `src/features/agent-monitor/utils/agentRuntimeSelector.test.ts`

**Interfaces:**
- Produces: `selectUnifiedAgentMonitorView(runtime, snapshot, now, filter)` with canonical-key dedup, LIVE precedence, rollout-only NEAR LIVE nodes, and source filtering.

- [ ] Add failing tests for external roots/sub-agents, paired dedup, Current isolation, filters, freshness, and non-additive tokens.
- [ ] Implement the minimal projection and merge.
- [ ] Run focused selector tests until green.

### Task 5: Thin Agent Monitor UI Integration

**Files:**
- Modify: `src/features/app/components/MainApp.tsx`
- Modify: `src/features/agent-monitor/components/AgentMonitorPage.tsx`
- Modify: `src/features/agent-monitor/components/AgentMonitorPage.test.tsx`
- Modify: `src/features/agent-monitor/components/AgentTreeNode.tsx`
- Modify: `src/features/agent-monitor/components/AgentTreeNode.test.tsx`
- Modify: `src/styles/agent-monitor.css`

**Interfaces:**
- Consumes: the global store and unified selector.
- Produces: source filter plus source/freshness badges in full and split layouts.

- [ ] Add failing UI tests for CLI option labeling, source filters, freshness, and no false Current marker.
- [ ] Wire the store once in MainApp and pass the snapshot to both Agent Monitor variants.
- [ ] Add compact source/freshness presentation without restructuring the page.
- [ ] Run focused UI tests until green.

### Task 6: Verification

**Files:**
- Verify only; no planned production changes.

- [ ] Run focused frontend and Rust tests.
- [ ] Run `npm run typecheck`.
- [ ] Run `npm run lint`.
- [ ] Run `npm run test`.
- [ ] Run `cargo check` from `src-tauri`.
- [ ] Run `git diff --check` and inspect the owned-file diff.
- [ ] Report automated evidence and the three real A/B/C acceptance procedures without performing them automatically.


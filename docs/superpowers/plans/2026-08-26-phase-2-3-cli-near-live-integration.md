# Phase 2.3 CLI Near-Live Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run one backend-owned rollout watcher for all known Codex homes and prove external CLI observations, checkpoint recovery, lag, and LIVE/NEAR_LIVE authority behavior end to end.

**Architecture:** A desktop-only Tauri adapter owns one watch-service task for the application lifetime. Shared Global Source Core remains responsible for discovery, byte cursors, checkpoints, parsing, health, and authority; confirmed app-server notifications are copied through a separate LIVE channel into that registry without touching Phase 1 Runtime. A redacted app-data diagnostic journal is the only temporary integration evidence surface.

**Tech Stack:** Rust, Tokio, Tauri 2, notify, serde/serde_json, existing Phase 2.1/2.2 Global Source Core.

**Spec:** User-approved Phase 2.3 request in the current task.

## Global Constraints

- Do not change Phase 1 Runtime State semantics or Agent Monitor UI.
- Do not add Desktop, Historical Scanner, multi-window Runtime, mobile, router, installer, or release work.
- Filesystem notifications remain wake-up signals; every read uses the existing reconcile and delta-reader path.
- Rollout observations remain `NEAR_LIVE`; app-server observations remain `LIVE`; tokens never add across lanes.
- Diagnostics exclude prompts, assistant text, tool arguments, and other conversation content.

---

### Task 1: Runtime configuration and CODEX_HOME discovery

**Files:**
- Create: `src-tauri/src/shared/global_sources_core/runtime_config.rs`
- Modify: `src-tauri/src/shared/global_sources_core.rs`
- Test: `src-tauri/src/shared/global_sources_core/runtime_config.rs`

**Interfaces:**
- Consumes: default Codex home plus workspace-resolved Codex homes and Tauri app-data root.
- Produces: `discover_runtime_codex_homes(...) -> Vec<CodexHomeSource>` and stable checkpoint/diagnostic paths.

- [ ] Write tests proving path-normalized deduplication, more than one home, stable redacted identity, and app-data file placement.
- [ ] Run focused Rust tests and observe missing API failures.
- [ ] Implement the smallest discovery/config API using the existing home resolver and literal app-data filenames.
- [ ] Re-run focused tests.

### Task 2: Unique backend lifecycle

**Files:**
- Create: `src-tauri/src/global_sources/mod.rs`
- Create: `src-tauri/src/global_sources/runtime.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/global_sources/runtime.rs`

**Interfaces:**
- Consumes: `RolloutWatchService`, app state workspaces, app-data paths, Tokio shutdown channel.
- Produces: `GlobalRolloutRuntime::start(...)`, `GlobalRolloutRuntime::shutdown()`, and a synchronous LIVE event sender.

- [ ] Write tests proving a second start is rejected/no-op, shutdown joins the only task, and shutdown before start is safe.
- [ ] Run focused tests and observe missing lifecycle behavior.
- [ ] Add the desktop-only managed state, start it once after `AppState` is managed, and await shutdown from the existing exit cleanup path.
- [ ] Re-run focused tests and cargo check.

### Task 3: Confirmed app-server LIVE adapter and authority diagnostics

**Files:**
- Create: `src-tauri/src/global_sources/app_server_live.rs`
- Create: `src-tauri/src/global_sources/diagnostics.rs`
- Modify: `src-tauri/src/event_sink.rs`
- Modify: `src-tauri/src/shared/global_sources_core/rollout_watch_service.rs`
- Test: `src-tauri/src/global_sources/app_server_live.rs`
- Test: `src-tauri/src/global_sources/diagnostics.rs`

**Interfaces:**
- Consumes: real app-server notification envelope, workspace-to-home map, source registry, reconcile reports.
- Produces: zero or one confirmed `SourceLaneUpdate`, a service command channel, and redacted JSONL observations containing only identity/model/token/lifecycle/time/provenance.

- [ ] Write fixture-driven tests for settings model, turn start/complete, token snapshot, unknown event rejection, and absence of message content.
- [ ] Run focused tests and observe missing normalization/journal failures.
- [ ] Implement only fixture-confirmed schemas and feed resulting LIVE updates into the watch service's existing authority registry.
- [ ] Write authority tests proving one canonical Thread, fresh LIVE precedence, non-additive tokens, and non-regressing fallback.
- [ ] Re-run focused and all Global Source tests.

### Task 4: Real CLI evidence run

**Files:**
- Runtime artifacts only: CodexMonitor app-data checkpoint and diagnostic journal.
- Do not add raw unsanitized rollout files to the repository.

**Interfaces:**
- Consumes: independently launched `codex exec`/resume processes and the running CodexMonitor backend.
- Produces: measured diagnostic rows for session creation, task start, model, token, sub-agent creation, and completion.

- [ ] Start CodexMonitor and confirm exactly one watcher-start diagnostic.
- [ ] Run a new single-agent CLI task from an independent shell process.
- [ ] Resume the same session for a second turn.
- [ ] Run a read-only Main + 2–3 Sub-Agent task with an explicit model.
- [ ] Restart Monitor and verify committed offsets advance without replay while any pending partial tail remains uncommitted.
- [ ] Exercise missed-notification reconciliation and a transient sharing/lock failure, recording whether recovery was observed.
- [ ] Pair one Monitor-owned thread's LIVE notifications with its rollout lane and inspect the canonical authority decision.

### Task 5: Verification and acceptance report

**Files:**
- Modify only the plan checkbox state if needed; no status document is required.

**Interfaces:**
- Consumes: test output, diagnostic journal, checkpoint before/after snapshots, and Git diff.
- Produces: the requested Phase 2.3 report with automated, real-environment, and unverified evidence separated.

- [ ] Calculate count/min/p50/p95/max lag by confirmed record kind from `sourceTimestamp` to `observedTimestamp`.
- [ ] Run focused Rust tests, `cargo check`, frontend typecheck where shared contracts require it, formatting, and `git diff --check`.
- [ ] Inspect the final diff for out-of-scope files, accidental content capture, placeholders, and Phase 1 semantic changes.
- [ ] Report lifecycle, discovery, checkpoint path/recovery, real CLI results, lag, authority, failure recovery, and Phase 2.4 readiness without overstating unverified cases.

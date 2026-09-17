# Phase 3.5.4a Projection Freshness Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one shared, read-only, generation-aware projection freshness authority with App/daemon schema parity and no recovery, UI, event-envelope, persistence, or mutation behavior.

**Architecture:** A shared Rust runtime stores coverage-scoped freshness evidence and selects current-generation direct evidence ahead of historical timestamps. Existing authoritative `thread/list`, exact `thread/read`, and observation-query boundaries record evidence through shared helpers; App and daemon expose the same read-only query schema through thin adapters.

**Tech Stack:** Rust, Tokio, Serde/JSON-RPC, Tauri IPC, TypeScript contract declarations, sanitized JSON fixtures.

**Spec:** User-approved Phase 3.5.4 forensic / contract freeze and Phase 3.5.4a authorization dated 2026-09-17.

## Global Constraints

- Reuse `DaemonProcessGeneration`, `RemoteTransportGeneration`, `WorkspaceSessionGeneration`, and `AppServerConnectionGeneration`; add no new generation type.
- Coverage is independent across `workspace_catalog`, `thread_catalog`, `thread_detail`, and `observation_snapshot`.
- Status is limited to `not_hydrated`, `hydrating`, `current`, `stale`, `unavailable`, and `unknown`.
- No event envelope, recovery orchestration, UI behavior, persistence, RemoteClientIdentity, owner, lease, replay, or mutation.
- Preserve the dirty main-worktree files and stage only Phase 3.5.4a paths.

---

### Task 1: Shared freshness schema and authority

**Files:**
- Create: `src-tauri/src/shared/projection_freshness.rs`
- Create: `src-tauri/src/shared/projection_freshness_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`

**Interfaces:**
- Produces: `ProjectionFreshnessRuntime`, `ProjectionFreshnessSnapshot`, coverage/status/source enums, generation vector, scope key, and read-only snapshot selection.
- Consumes: existing generation identifiers and `CodexThreadKey`.

- [ ] Write the required model, coverage, generation-invalidation, partial-hydration, timestamp-precedence, and forbidden-semantics tests against the desired API.
- [ ] Run `cargo test projection_freshness --lib -- --nocapture` and preserve the expected missing-authority RED.
- [ ] Implement the minimal shared runtime and schema.
- [ ] Re-run the focused tests and require zero failures.

### Task 2: Authoritative read instrumentation

**Files:**
- Modify: `src-tauri/src/shared/codex_core.rs`
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify focused shared tests as required by constructor fields.

**Interfaces:**
- Produces: shared wrappers that mark only matching coverage current after successful `thread/list`, exact `thread/read`, or observation query evidence.
- Consumes: `ProjectionFreshnessRuntime` and current WorkspaceSession/app-server generations.

- [ ] Add failing tests proving list/detail/observation coverage independence and zero mutations.
- [ ] Run the focused tests and verify they fail because instrumentation is absent.
- [ ] Add the runtime to `WorkspaceSession` and instrument only the three authorized read boundaries.
- [ ] Re-run focused read and observation regressions.

### Task 3: App/daemon read-only parity

**Files:**
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`
- Modify: `src/services/tauri.ts`
- Modify: `src/types.ts`

**Interfaces:**
- Produces: `get_projection_freshness` with identical App and daemon serialization.
- Consumes: the current WorkspaceSession runtime; missing workspace/session remains distinct from not-hydrated coverage.

- [ ] Add failing parity and read-only adapter tests.
- [ ] Run them and preserve the expected missing-route RED.
- [ ] Add thin App/daemon adapters and the minimal TypeScript contract/wrapper.
- [ ] Re-run parity, daemon RPC, typecheck, and mutation-count tests.

### Task 4: Fixtures, authority docs, verification, and Git closeout

**Files:**
- Create: `docs/fixtures/projection-freshness/*`
- Create: `docs/phase-3-5-4-projection-recovery-telemetry.md`
- Create: `docs/evidence/phase-3-5-4a/projection-freshness-authority.md`
- Modify: `README.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-0-remote-mobile-forensics.md`

**Interfaces:**
- Produces: sanitized fixture coverage and the canonical Phase 3.5.4a authority record.

- [ ] Add the ten sanitized fixtures and a fixture-contract test that fails until every required scenario is represented.
- [ ] Document generation precedence, partial coverage, unavailable/unknown non-absence, read-only behavior, and forbidden semantics.
- [ ] Run all requested focused and full Rust/TypeScript verification plus `git diff --check`.
- [ ] Review the scoped diff, explicitly stage only Phase 3.5.4a files, commit, ff-only merge to `main`, push `origin/main`, and verify local/remote alignment.

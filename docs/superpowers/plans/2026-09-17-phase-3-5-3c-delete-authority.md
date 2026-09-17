# Phase 3.5.3c Delete Authority and Exact-ID Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind every App-local and Remote `thread/delete` mutation to one exact canonical Thread and current host/session generations, observe only direct upstream outcomes, and reconcile tombstones only after authoritative confirmation.

**Architecture:** Add a shared `DeleteMutationObservationRuntime` beside the existing writer/subscription/approval runtimes. `WorkspaceSession` owns the runtime and one observed `thread/delete` dispatch path; App and daemon adapters supply the host authority and Remote transport correlation while a shared confirmed-only reconciliation gate protects the existing tombstone path. Exact `thread/deleted` notifications may confirm the current exact attempt but never infer deletion from runtime or projection absence.

**Tech Stack:** Rust, Tokio, serde/serde_json, UUID v4, existing fake app-server and daemon transport fixtures, Markdown authority docs.

**Spec:** User-approved Phase 3.5.3c contract in the 2026-09-17 task request and `docs/phase-3-5-3-approval-delete-authority.md`.

## Global Constraints

- All mutation tests use fake app-server sessions, sanitized fixtures, temporary state, and fake Thread identities.
- Never delete a real Codex Thread, project Thread, frozen A3 target, or real rollout.
- Exact identity is `(RemoteHostIdentity, CodexThreadKey, WorkspaceSessionGeneration, AppServerConnectionGeneration, DeleteAttemptId)`.
- Automatic retry and replay remain zero; no takeover, owner, lease, RemoteClientIdentity, fuzzy matching, or UI-absence inference is added.
- `thread/delete` success `{}` and exact current-generation `thread/deleted` are the only direct confirmation evidence.
- Phase 3.5.3d concurrency, reconnect-race, restart recovery, duplicate ordering, and real E2E remain out of scope.

---

### Task 1: Shared delete observation state machine

**Files:**
- Create: `src-tauri/src/shared/codex_core/delete_mutation_observation.rs`
- Create: `src-tauri/src/shared/codex_core/delete_mutation_observation_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`

**Interfaces:**
- Consumes: `RemoteHostIdentity`, `CodexThreadKey`, `WorkspaceSessionGeneration`, `AppServerConnectionGeneration`.
- Produces: `DeleteAttemptId`, `DeleteMutationObservationState`, exact scope/snapshot types, begin/confirm/reject/unknown/session-end transitions.

- [ ] Add tests named by the approved contract for exact identity, unique attempts, generation rejection, success/event confirmation, idempotency, active-writer rejection, unknown/session-end outcomes, and forbidden fields.
- [ ] Run `cargo test delete_mutation_observation --lib -- --nocapture` and retain the expected missing-module/API RED.
- [ ] Implement the smallest state machine that makes the tests pass without dispatch, retry, ownership, or projection inference.
- [ ] Re-run the focused test and confirm GREEN.

### Task 2: Observed WorkspaceSession delete dispatch

**Files:**
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`
- Create: `src-tauri/src/shared/codex_core/delete_mutation_instrumentation_tests.rs`

**Interfaces:**
- Consumes: current session generations, resolved `CodexThreadKey`, caller-supplied current `RemoteHostIdentity`, optional `RemoteRequestDispatchContext`.
- Produces: one exact upstream `thread/delete { threadId }` dispatch and a direct-observation snapshot/outcome.

- [ ] Add fake-session tests for full-ID enforcement, zero dispatch for invalid/stale scope, success/rejection/response-loss/cancellation outcomes, session-end transition, and zero retry/replay.
- [ ] Run the focused instrumentation tests and retain RED.
- [ ] Add a delete dispatch guard around `DispatchBoundary`; bind Remote provenance to the already-created `DeleteAttemptId`; classify exact `{}` success, typed upstream rejection, malformed/ambiguous results, and post-dispatch errors.
- [ ] Feed exact current-generation `thread/deleted` notifications into this runtime; leave `thread/closed`, `notLoaded`, read failure, transport disconnect, and projection removal non-transitioning.
- [ ] Re-run focused tests and confirm GREEN.

### Task 3: App/daemon exact authority and confirmed-only reconciliation parity

**Files:**
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`
- Modify: `src-tauri/src/shared/remote_request_provenance.rs`
- Modify: `src-tauri/src/shared/global_sources_core/deletion_tombstone.rs`
- Add or modify focused App/daemon/reconciliation tests.

**Interfaces:**
- Consumes: confirmed shared delete outcome and exact key.
- Produces: one shared confirmed-delete reconciliation request accepted by both App-local and Remote adapters; non-confirmed outcomes produce no tombstone.

- [ ] Add RED tests for App/daemon use of the same core, Remote provenance binding, exact tombstone creation, pending/unknown/rejected non-reconciliation, and duplicate direct-evidence idempotency.
- [ ] Wire daemon delete through the Remote context and actual daemon `RemoteHostIdentity`; wire App-local delete through a valid local host authority without changing the RPC schema.
- [ ] Refactor the current reconciliation helper so only `DeleteConfirmed` can persist/reconcile the exact key and descendants.
- [ ] Re-run parity, daemon RPC, tombstone, and transport-provenance tests.

### Task 4: Sanitized protocol fixtures and contract tests

**Files:**
- Create: `docs/fixtures/app-server/delete-mutation-observation/*.json`
- Create: `src-tauri/src/shared/codex_core/delete_mutation_protocol_fixture_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`

**Interfaces:**
- Consumes: sanitized bundled 0.153.4 request/response/event shapes.
- Produces: stable serialized observation and fixture compatibility checks.

- [ ] Add exact request, `{}` success, `-32600` active-writer rejection, exact `thread/deleted`, response-loss unknown, and session-end unresolved fixtures using only synthetic UUIDs and paths.
- [ ] Run fixture tests for RED before registering/implementing fixture serialization.
- [ ] Implement stable snake/camel-case serialization and run fixture tests GREEN.

### Task 5: Authority docs, full verification, and scoped Git closeout

**Files:**
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-3-approval-delete-authority.md`
- Create: `docs/evidence/phase-3-5-3c/delete-authority-freeze.md`
- Modify other existing Phase 3.5.3 roadmap/current-state docs only when directly required.

**Interfaces:**
- Consumes: verified production/test behavior.
- Produces: canonical distinction among mutation authority, runtime unload, direct deletion evidence, tombstone reconciliation, and projection-only disappearance.

- [ ] Document `thread/delete`, `thread/deleted`, `thread/closed`, tombstone, and sidebar/catalog boundaries plus the bundled 0.153.4 evidence source.
- [ ] Run focused delete, approval a/b, writer/subscription/runtime, transport/provenance, daemon RPC tests.
- [ ] Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`.
- [ ] Inspect `git status` and `git diff`; explicitly stage only Phase 3.5.3c files, commit, ff-only merge to `main`, push `origin main`, and verify local/remote refs.

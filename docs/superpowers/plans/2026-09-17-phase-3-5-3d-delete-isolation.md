# Phase 3.5.3d Unknown / Stale / Multi-client Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve the Phase 3.5.3c exact-ID delete authority while making concurrent Remote delete intents, transport loss, stale generations, and session replacement deterministic and auditable without any retry, replay, ownership, or real Thread deletion.

**Architecture:** Keep `DeleteMutationObservationRuntime` as the WorkspaceSession-owned shared authority. Add an exact `(WorkspaceSessionGeneration, AppServerConnectionGeneration, CodexThreadKey)` active gate and stable current-attempt index inside that runtime, preserve local-versus-upstream rejection provenance on each attempt, and couple Remote transport loss to the real write boundary through the existing `RemoteRequestProvenance` correlation. Direct app-server evidence remains stronger than transport uncertainty; confirmed-only reconciliation remains the sole tombstone path.

**Tech Stack:** Rust, Tokio, serde/serde_json, UUID v4, existing fake app-server sessions and fake daemon transport fixtures, Markdown/JSON sanitized fixtures.

**Spec:** User-approved Phase 3.5.3d contract in the 2026-09-17 task request and `docs/phase-3-5-3-approval-delete-authority.md`.

## Global Constraints

- All mutation tests use fake app-server sessions, deterministic transport fixtures, synthetic IDs, and temporary state. No real `thread/delete` is sent.
- Canonical delete states remain exactly `not_observed`, `delete_pending`, `delete_confirmed`, `delete_rejected`, `delete_outcome_unknown`, and `session_ended_outcome_unknown`.
- Retry, replay, force takeover, owner/client/lease semantics, fuzzy identity, and projection-based confirmation remain absent.
- Same-thread concurrency is locally limited to one admitted attempt; different exact Thread keys remain independent.
- Phase 3.5.3e is out of scope.

---

### Task 1: RED fixtures and shared active-gate contract

**Files:**
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_observation_tests.rs`
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_instrumentation_tests.rs`
- Create: `docs/fixtures/app-server/delete-mutation-isolation/*.json`
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_protocol_fixture_tests.rs`

- [ ] Add the approved same-thread, different-thread, stale-generation, pre/post-dispatch, direct-evidence precedence, session-end, reconnect/restart, new-intent-after-unknown, tombstone, projection, forbidden-field, and zero-retry/replay tests.
- [ ] Add sanitized fixture compatibility tests for the ten approved lifecycle cases.
- [ ] Run the focused delete tests and retain the genuine compile/assertion RED before production changes.

### Task 2: Shared active-delete gate and rejection provenance

**Files:**
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_observation.rs`
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_observation_tests.rs`

- [ ] Replace the attempts-only map with one synchronized runtime state containing attempts, active attempt by exact Thread key, and stable newest-intent lookup.
- [ ] Admit at most one pending attempt per exact key and record a distinct terminal attempt for a duplicate active intent.
- [ ] Add minimal serialized rejection source/reason fields distinguishing local pre-dispatch rejection from upstream rejection.
- [ ] Release the active gate on every terminal shared outcome while retaining immutable attempt history and allowing a later explicit intent after unknown.
- [ ] Ensure late direct evidence may upgrade its exact dispatched attempt but cannot downgrade confirmed evidence or rewrite a different attempt's history.
- [ ] Run focused state-machine tests GREEN.

### Task 3: Atomic pre/post-dispatch transport isolation

**Files:**
- Modify: `src-tauri/src/shared/codex_core/creation_coordination.rs`
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify: `src-tauri/src/shared/remote_request_provenance.rs` only if required by the existing correlation contract
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_instrumentation_tests.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/remote_dispatch_correlation_tests.rs`

- [ ] Add an atomic cancel-before-write state to `DispatchBoundary`; a cancelled boundary must prevent the write, while loss after boundary crossing must not cancel the shared task.
- [ ] Bind the existing transport provenance to the already-created `DeleteAttemptId` and use the same boundary in the transport-loss hook.
- [ ] Record pre-dispatch loss as local rejection with zero dispatch; record post-dispatch loss as unknown only until direct evidence arrives.
- [ ] Verify duplicate same-transport requests create no second task/attempt and identical request IDs across transport generations remain independent.
- [ ] Run fake-session and fake-daemon transport tests GREEN.

### Task 4: Generation/session replacement and confirmed-only reconciliation regression

**Files:**
- Modify focused lifecycle/restart/reconciliation tests only as needed.
- Modify: `src-tauri/src/shared/codex_core/delete_mutation_protocol_fixture_tests.rs`

- [ ] Prove stale WorkspaceSession/app-server evidence cannot change the current runtime.
- [ ] Prove real session end converts dispatched unresolved attempts to `session_ended_outcome_unknown`, and new sessions inherit neither active gates nor pending attempts.
- [ ] Prove reconnect/restart never replays delete and same `RemoteHostIdentity` does not imply attempt continuity.
- [ ] Prove confirmed tombstones are exact/idempotent and stale projection evidence cannot confirm or resurrect a deleted Thread.

### Task 5: Authority docs, full verification, and scoped Git closeout

**Files:**
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-3-approval-delete-authority.md`
- Modify: `docs/fixtures/app-server/README.md`
- Create: `docs/evidence/phase-3-5-3d/delete-isolation-freeze.md`
- Modify: `docs/evidence/README.md`

- [ ] Freeze active-gate authority, generation hierarchy, local/upstream rejection provenance, transport uncertainty, direct-evidence precedence, zero retry/replay, and fake-only verification limitations.
- [ ] Run focused delete, provenance, approval, writer/subscription/runtime, reconnect/restart, and tombstone regressions.
- [ ] Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`.
- [ ] Audit status/diff and forbidden semantics; explicitly stage only Phase 3.5.3d files, commit, ff-only merge, push, and verify local/remote refs without touching main-worktree user changes.

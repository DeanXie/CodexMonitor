# Phase 3.5.4b Generation-Tagged Event Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver app-server events through one generation-tagged envelope and reject any event that cannot be proven current for the frontend projection, without adding replay, buffering, recovery, or mutation behavior.

**Architecture:** Shared app-server truth carries WorkspaceSession and app-server connection generations. The daemon binds its process generation and the actual authenticated Remote transport generation only at outbound delivery. The frontend records authoritative projection generation context from successful read evidence and applies a fail-closed gate before any event fanout or reducer mutation.

**Tech Stack:** Rust, Tokio, Serde/JSON-RPC, Tauri events, TypeScript, Vitest, sanitized JSON fixtures.

**Spec:** User-approved Phase 3.5.4b Generation-tagged Event Delivery contract dated 2026-09-17.

## Global Constraints

- Reuse only `DaemonProcessGeneration`, `RemoteTransportGeneration`, `WorkspaceSessionGeneration`, and `AppServerConnectionGeneration`.
- Local App events never invent daemon or Remote transport provenance.
- Remote transport provenance is bound during actual daemon delivery, not stored in shared WorkspaceSession event truth.
- Missing or mismatched required generations fail closed before frontend event fanout.
- Events cannot promote not-hydrated, stale, unavailable, or unknown coverage to current.
- Preserve the existing stale transport delivery gate and all writer/subscription/runtime/approval/delete authorities.
- No replay, retry, buffering, sequence/gap model, UI, recovery orchestration, or new mutation.
- Preserve the dirty main-worktree files and explicitly stage only Phase 3.5.4b paths.

---

### Task 1: RED contract tests and fixtures

**Files:**
- Modify: `src-tauri/src/backend/events.rs`
- Modify focused Rust event/transport tests
- Modify: `src/services/events.test.ts`
- Create: `docs/fixtures/generation-tagged-events/*`
- Add focused fixture/schema tests in the established compatibility test locations

- [x] Add the required generation-envelope, local/Remote parity, stale/missing generation, freshness interaction, reload, multi-client, and non-mutation tests.
- [x] Add all eleven sanitized fixtures from the approved contract.
- [x] Run focused Rust and frontend tests and preserve genuine failures caused by the missing generation envelope/gate.

### Task 2: Shared and adapter event envelope

**Files:**
- Modify: `src-tauri/src/backend/events.rs`
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/src/event_sink.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/transport.rs`
- Modify: `src-tauri/src/remote_backend/transport.rs`

- [x] Add required WorkspaceSession/app-server generations to shared app-server events.
- [x] Ensure local App delivery leaves daemon/Remote transport provenance absent.
- [x] Bind daemon process and actual authenticated transport generations at daemon outbound delivery.
- [x] Keep and extend the existing stale transport-generation delivery gate without changing reconnect/session truth.
- [x] Re-run focused Rust tests until GREEN.

### Task 3: Frontend fail-closed delivery authority

**Files:**
- Modify: `src/types.ts`
- Modify: `src/services/tauri.ts`
- Modify: `src/services/events.ts`
- Modify focused frontend tests

- [x] Add the stable TypeScript envelope schema matching Rust/daemon serialization.
- [x] Capture current generation context only from authoritative successful read/freshness evidence.
- [x] Reject missing, stale, or contextless events before any listener/reducer receives them.
- [x] Allow a matching event to incrementally update an already-current projection without completing broader hydration.
- [x] Require current `thread_catalog` authority so another current coverage cannot bypass a not-hydrated catalog.
- [x] Prove rejected delivery causes zero reducer/shared-authority mutations and no buffering/replay.
- [x] Re-run focused frontend/type tests until GREEN.

### Task 4: Compatibility docs, fresh verification, and Git closeout

**Files:**
- Modify: `README.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/app-server-events.md`
- Modify: `docs/phase-3-5-4-projection-recovery-telemetry.md`
- Create/update: `docs/evidence/phase-3-5-4b/*`
- Modify established compatibility fixture/index docs as required

- [x] Document the generation hierarchy, local/Remote binding boundary, fail-closed frontend gate, freshness limits, reconnect/multi-client semantics, and absent sequence/replay guarantees.
- [x] Run focused generation-event/frontend/freshness/transport/session/multi-client and observation/delete regressions.
- [x] Run Rust all-targets, `cargo check --all-targets`, `cargo fmt --check`, frontend focused/full tests, `npm run typecheck`, forbidden-semantics scan, and `git diff --check`.
- [x] Inspect the scoped diff, explicitly stage only Phase 3.5.4b files, commit, ff-only merge to `main`, push `origin/main`, and verify local/remote alignment.

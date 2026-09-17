# Phase 3.5.4d Offline / Stale UI & Multi-client Isolation Implementation Plan

Status: **COMPLETE**.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Project existing freshness, availability, generation, hydration, and event-delivery evidence into a concise user-visible Remote status without creating new canonical authority or mutation behavior.

**Architecture:** Keep `ProjectionFreshness` as the only freshness authority and derive a pure frontend view model per coverage. Add a transport-local, ephemeral daemon broadcast-lag marker so only the affected frontend derives stale coverage and requests the existing single-flight authoritative hydration. Keep availability diagnostics, approval/delete action safety, and legacy live-delivery mode separate from freshness.

**Tech Stack:** Rust/Tauri, TypeScript, React, Vitest, Cargo tests.

**Spec:** User-approved Phase 3.5.4d request plus `docs/phase-3-5-4-projection-recovery-telemetry.md`.

## Global Constraints

- Do not add a second UI/client freshness authority; view-model mapping is pure and gap evidence is ephemeral diagnostic input.
- Do not add durable sequence, replay, telemetry persistence, RemoteClientIdentity, owner, lease, takeover, automatic mutation retry, or automatic mutation replay.
- Do not mutate canonical Thread, writer, subscription/runtime, approval, or delete authority from a delivery gap.
- Preserve coverage independence for workspace catalog, thread catalog, thread detail, and observation snapshot.
- Reuse the Phase 3.5.4c per-workspace single-flight recovery coordinator.
- Preserve the four user-local main-checkout changes and explicitly stage only Phase 3.5.4d files.

---

### Task 1: Establish RED view-model, safety, and fixture contracts

**Files:**
- Create: `src/features/app/orchestration/projectionStatusModel.test.ts`
- Create: `src/features/app/components/ProjectionStatusIndicator.test.tsx`
- Create: sanitized fixtures under `src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/`

**Interfaces:**
- Consumes: frozen `ProjectionFreshnessQuerySnapshot`, Remote availability layers, authoritative observation snapshot fields.
- Produces: wished-for pure `buildProjectionStatusModel`, `isApprovalProjectionActionable`, and confirmed-delete projection helpers.

- [x] Add the required named RED cases for current/hydrating/stale/unavailable/unknown, mixed coverage, reconnect/restart, gap, multi-client isolation/convergence, duplicate/out-of-order, approval/delete safety, read-only recovery, mutation counts, and forbidden semantics.
- [x] Add all seventeen sanitized scenario fixtures with literal expected UI/evidence values.
- [x] Run only the new focused tests and preserve the expected missing-module/component failures as RED evidence.

### Task 2: Implement the pure projection status model

**Files:**
- Create: `src/features/app/orchestration/projectionStatusModel.ts`
- Modify: `src/types.ts`

**Interfaces:**
- Consumes: `ProjectionFreshnessQuerySnapshot`, availability snapshot, optional local delivery-gap evidence, observation values.
- Produces: `ProjectionStatusModel` with primary Current/Hydrating/Stale/Unavailable/Unknown, coverage rows, separate availability diagnosis, and safe action predicates.

- [x] Map `ProjectionFreshness` statuses without inventing absence or truth; map `not_hydrated` to user-visible Unknown.
- [x] Derive coverage-specific mixed state and availability diagnostics independently.
- [x] Downgrade retained current cache to Stale during disconnect/reconnect/daemon-generation mismatch or a known affected gap, without mutating shared authority.
- [x] Gate approvals on current observation freshness plus exact current pending evidence; preserve unknown delete outcomes and suppress confirmed-delete resurrection.
- [x] Run the focused model tests to GREEN.

### Task 3: Add per-transport ephemeral broadcast-gap evidence

**Files:**
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`
- Modify: `src-tauri/src/remote_backend/transport.rs`
- Modify: `src/types.ts`
- Modify: `src/services/events.ts`
- Modify: focused Rust/TypeScript event tests.

**Interfaces:**
- Consumes: Tokio broadcast `Lagged(skipped)` and current daemon/transport generations.
- Produces: `app-server-event-gap` diagnostic notification and per-frontend in-memory gap evidence/subscription.

- [x] Add RED tests proving a daemon lag emits an ephemeral marker, the transport gate admits it only for the current generation, and frontend A gap state cannot affect frontend B.
- [x] Implement a minimal gap marker with skipped count plus daemon/transport generation; do not add sequence or persistence.
- [x] On frontend receipt, mark only that frontend's tracked affected coverages stale in the pure view and notify recovery listeners; authoritative hydration clears the diagnostic.
- [x] Verify duplicate/out-of-order unresolved evidence falls back to stale/unknown and never timestamp authority.

### Task 4: Wire concise UI status and safe read-only recovery

**Files:**
- Create: `src/features/app/hooks/useRemoteProjectionStatus.ts`
- Create: `src/features/app/components/ProjectionStatusIndicator.tsx`
- Modify: `src/features/app/hooks/useMainAppWorkspaceLifecycle.ts`
- Modify: `src/features/app/hooks/useMainAppShellProps.tsx`
- Modify: `src/features/app/components/MainApp.tsx`
- Modify: `src/features/app/components/ApprovalToasts.tsx`
- Modify: focused component/hook tests and existing CSS as required.

**Interfaces:**
- Consumes: read-only freshness/availability snapshots, local gap evidence, existing recovery coordinator, active Workspace/Thread.
- Produces: primary status badge, compact coverage detail, secondary transport/daemon/workspace diagnostic, and non-actionable stale approval UI.

- [x] Add RED integration tests proving legacy Live/Polling/Disconnected is secondary delivery information only.
- [x] Subscribe to backend freshness snapshots and local gap evidence; refresh read-only status without polling.
- [x] Trigger only the existing single-flight authoritative recovery on a known gap; never call resume, approval decision, delete, or upstream unsubscribe.
- [x] Disable approval actions unless exact current pending evidence and observation freshness are current.
- [x] Render mixed coverage details in the status title/details while keeping the primary label concise.
- [x] Run focused UI/hook tests to GREEN.

### Task 5: Regression, authority docs, and closeout

**Files:**
- Modify: `README.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-4-projection-recovery-telemetry.md`
- Add: Phase 3.5.4d evidence/index documentation.

**Interfaces:**
- Consumes: completed implementation and fresh test evidence.
- Produces: frozen compatibility contract and scoped Git commit.

- [x] Document user-visible state mapping, availability separation, mixed coverage, local-only gap invalidation, multi-client divergence/convergence, safety boundaries, and zero retry/replay.
- [x] Run focused offline/stale UI, event-gap, multi-client, freshness, generation-event, recovery, approval/delete safety, and availability tests.
- [x] Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, frontend focused/full tests, and `git diff --check`; apply only the existing six zh-CN waiver if reproduced unchanged.
- [x] Inspect the scoped diff and forbidden semantics, then explicitly stage only Phase 3.5.4d files and commit.
- [x] Fast-forward merge to `main`, push `origin/main`, verify local/remote equality, and stop before Phase 3.5.4e. If dirty-main safety rejects the merge, stop without touching the user's files.

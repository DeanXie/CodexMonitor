# Phase 3.5.3a Approval Request Observation Implementation Plan

Status: **COMPLETE**.

**Goal:** Add a generation-scoped, shared, observation-only authority for bundled app-server approval requests and reconcile resolved requests out of the frontend actionable projection.

**Architecture:** A crate-private `ApprovalObservationRuntime` lives on each `WorkspaceSession`, ingests supported app-server messages before App/daemon event projection, and retains current pending plus historical terminal evidence. Frontend handling remains a projection and only removes exact resolved request IDs; no decision or delete path changes.

**Tech Stack:** Rust, Tokio, Serde/serde_json, React/TypeScript, Vitest, sanitized JSON fixtures.

**Spec:** `docs/phase-3-5-3-approval-delete-authority.md`

## Global Constraints

- Supported request kinds are command execution, file change, and permissions only.
- Identity includes WorkspaceSession generation, app-server connection generation, requestId, threadId, turnId, and itemId.
- `approvalId` is optional subordinate evidence only.
- `serverRequest/resolved` requires exact current-generation requestId/threadId matching.
- `item/completed` requires exact current-generation requestId/threadId/turnId/itemId matching.
- Session-ended state requires direct WorkspaceSession/app-server generation-end evidence.
- No approval decision mutation, retry, delete mutation, RemoteClientIdentity, owner, or lease is added.

---

### Task 1: Shared approval observation model

**Files:**
- Create: `src-tauri/src/shared/codex_core/approval_observation.rs`
- Create: `src-tauri/src/shared/codex_core/approval_observation_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`

**Interfaces:**
- Consumes: existing `WorkspaceSessionGeneration` and `AppServerConnectionGeneration` value types.
- Produces: `ApprovalObservationRuntime`, exact identity/request-kind types, pending/history snapshots, and generation-checked ingestion methods.

- [x] **Step 1: Write failing state/identity/request-kind tests**

Add literal tests for command, file-change, and permissions requests; generation isolation; exact resolved matching; exact item completion matching; auto-review annotations; and session-end transitions.

- [x] **Step 2: Run focused Rust tests and verify RED**

Run: `cargo test --lib approval_observation -- --nocapture`

Expected: compilation/test failures because the shared observation API does not exist.

- [x] **Step 3: Implement the minimal shared runtime**

Use separate pending and history storage. Normalize JSON-RPC IDs as string-or-integer values, sanitize request evidence by kind, reject stale generation transitions, and expose no decision methods.

- [x] **Step 4: Run focused Rust tests and verify GREEN**

Run: `cargo test --lib approval_observation -- --nocapture`

Expected: all approval observation model tests pass.

### Task 2: WorkspaceSession ingestion and lifecycle integration

**Files:**
- Modify: `src-tauri/src/backend/app_server.rs`
- Test: `src-tauri/src/backend/app_server.rs`

**Interfaces:**
- Consumes: `ApprovalObservationRuntime::ingest_server_message` and direct generation-end transition.
- Produces: shared observation updates before existing `AppServerEvent` emission.

- [x] **Step 1: Write failing ingestion/parity tests**

Cover ingestion-before-projection, exact `serverRequest/resolved`, exact `item/completed`, stale connection generation, direct process/session end, and Remote transport non-transition.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test --lib approval_observation -- --nocapture`

Expected: failures because `WorkspaceSession` does not own or invoke the runtime.

- [x] **Step 3: Add the runtime to WorkspaceSession and ingest before projection**

Construct it from the same WorkspaceSession/app-server generations as the existing lifecycle runtimes. Call it before daemon/frontend event emission. Wire only direct app-server generation-end paths to `session_ended_unresolved`.

- [x] **Step 4: Run tests and verify GREEN**

Run: `cargo test --lib approval_observation -- --nocapture`

Expected: model and integration tests pass without changing outgoing response/delete paths.

### Task 3: Frontend resolved projection reconciliation

**Files:**
- Modify: `src/features/app/hooks/useAppServerEvents.ts`
- Modify: `src/features/app/hooks/useAppServerEvents.test.tsx`
- Modify: `src/features/threads/hooks/useThreadsReducer.ts`
- Modify: `src/features/threads/hooks/threadReducer/threadQueueSlice.ts`
- Test: existing focused frontend approval tests.

**Interfaces:**
- Consumes: raw `serverRequest/resolved {threadId, requestId}` notification.
- Produces: exact workspace/request removal from the actionable approval projection.

- [x] **Step 1: Write a failing frontend test**

Assert that a prior actionable approval is removed only by a resolved event with the exact workspace/request ID, while unrelated request IDs remain.

- [x] **Step 2: Run focused frontend tests and verify RED**

Run: `npm run test -- src/features/app/hooks/useAppServerEvents.test.tsx src/features/threads/hooks/useThreadApprovalEvents.test.tsx`

Expected: resolved events do not currently reconcile the queue.

- [x] **Step 3: Add the minimal read-only projection hook**

Route `serverRequest/resolved` to the existing `removeApproval` reducer action without adding any decision capability.

- [x] **Step 4: Run focused frontend tests and verify GREEN**

Run the same command and expect all tests to pass.

### Task 4: Sanitized protocol fixtures and authoritative documentation

**Files:**
- Create: `docs/fixtures/app-server/approval-observation/*.json`
- Modify: `docs/fixtures/app-server/README.md`
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-0-remote-mobile-forensics.md`
- Modify: `docs/phase-3-5-3-approval-delete-authority.md`

**Interfaces:**
- Consumes: bundled codex-cli 0.153.4 request/notification schemas.
- Produces: sanitized compatibility evidence with no command text, token, owner, or lease fields.

- [x] **Step 1: Add fixture-driven tests that fail without fixture support**

Fixtures cover the three request kinds, resolved, item completed, auto-review annotation, generation isolation, and session end.

- [x] **Step 2: Verify fixture RED**

Run: `cargo test --lib approval_observation_protocol_fixture -- --nocapture`

- [x] **Step 3: Add fixtures, parser assertions, and current canonical docs**

Document the supported request families, explicit MCP elicitation exclusion, authority separation, generation scope, and no-decision/no-delete boundary.

- [x] **Step 4: Verify fixture GREEN**

Run the focused fixture tests and expect all cases to pass.

### Task 5: Fresh verification and scoped Git closeout

**Files:**
- Verify all scoped source, tests, fixtures, plans, and docs only.

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: one scoped Phase 3.5.3a commit merged and pushed to `origin/main`.

- [x] **Step 1: Run focused and non-regression tests**

Run focused approval observation, generation isolation, lifecycle, App/daemon parity, writer/subscription/runtime, and frontend tests.

- [x] **Step 2: Run full verification**

Run `cargo test --all-targets`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, applicable frontend tests, and `git diff --check`.

- [x] **Step 3: Inspect scoped diff and forbidden semantics**

Confirm no decision/delete path changed and no owner, lease, RemoteClientIdentity, retry, accepted, declined, or auto-approved state was introduced.

- [x] **Step 4: Explicitly stage and commit scoped files**

Use explicit paths only; do not use `git add -A`.

- [x] **Step 5: Fast-forward main, push, and verify refs**

Preserve the dirty main-checkout files, perform an ff-only merge, push `origin/main`, and verify local main and origin/main point to the scoped commit.

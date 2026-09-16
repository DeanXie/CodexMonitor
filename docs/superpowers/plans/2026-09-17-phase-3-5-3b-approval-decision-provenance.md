# Phase 3.5.3b Approval Decision Provenance Implementation Plan

> **Status:** approved for implementation. This plan does not authorize Phase 3.5.3c or any Thread delete change.

**Goal:** Bind Remote approval decision dispatch to one exact, current approval observation; record transport/session provenance and dispatch ambiguity without changing upstream authority or retrying a decision.

**Architecture:** Extend the shared approval authority with an atomic decision-attempt gate so pending validation, generation validation, and single-dispatch admission happen under one lock. The daemon passes its existing `RemoteRequestDispatchContext` into the shared response boundary. A transport-loss hook observes whether the app-server response write boundary was crossed and records either `decision_not_dispatched` or `decision_outcome_unknown`. App-local non-Remote responses keep their existing path.

**Protocol contract:** Bundled codex-cli 0.153.4 response shapes are validated by approval kind: command execution accepts the six bundled decision forms, file change accepts its four string decisions, and permissions accepts the typed permissions/scope payload. Persisted provenance records only the response kind, never raw policy amendment, permission path, token, client identity, owner, or lease data.

---

## Task 1: RED — shared decision state, schemas, and atomic gate

**Files:**
- Create: `src-tauri/src/shared/codex_core/approval_decision_provenance.rs`
- Create: `src-tauri/src/shared/codex_core/approval_decision_provenance_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`
- Modify: `src-tauri/src/shared/codex_core/approval_observation.rs`

Add failing tests for current pending admission, exact identity/generation binding, resolved/session-ended rejection, one admitted attempt under concurrency, second-attempt rejection, auto-review resolution races, typed command/file/permissions schemas, wrong-schema rejection, state serialization, and forbidden ownership/lease semantics.

Run `cargo test --lib approval_decision_provenance -- --nocapture` and preserve the compile/test failure as RED evidence.

## Task 2: GREEN — shared decision gate and dispatch boundary

**Files:**
- Modify: `src-tauri/src/shared/codex_core/approval_decision_provenance.rs`
- Modify: `src-tauri/src/shared/codex_core/approval_observation.rs`
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`

Implement unique `ApprovalDecisionAttemptId`, session-generation/exact-identity binding, response-kind-only evidence, and states `decision_pending`, `decision_dispatched`, `decision_not_dispatched`, `decision_outcome_unknown`, and `decision_stale_rejected`. Add a cancellation guard around the real response write. Keep zero retry/replay.

## Task 3: RED/GREEN — Remote transport correlation and disconnect semantics

**Files:**
- Modify: `src-tauri/src/shared/remote_request_provenance.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`
- Create/modify scoped daemon correlation tests

First add failing tests for Remote transport/request correlation, stale old transport rejection, reconnect with a new transport against the same pending approval, disconnect before the write boundary, disconnect after the write boundary, and simultaneous multi-transport attempts. Then pass the existing Remote context into the shared approval decision boundary and bind it to the already-created decision attempt. A transport generation remains audit provenance only.

## Task 4: Bundled compatibility fixtures and docs

**Files:**
- Create: `docs/fixtures/app-server/approval-decision-provenance/*`
- Modify: `docs/fixtures/app-server/README.md`
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-3-approval-delete-authority.md`
- Create: `docs/evidence/phase-3-5-3b/approval-decision-provenance-freeze.md`
- Modify: `docs/evidence/README.md`
- Modify: `docs/phase-3-5-0-remote-mobile-forensics.md`

Freeze sanitized command, file-change, and permissions response shapes plus stale/duplicate/disconnect expectations. Keep docs canonical and current-state only.

## Task 5: Fresh verification and scoped Git closeout

Run focused decision, approval observation, generation, multi-client, auto-review/resolved, and daemon transport tests; relevant all-target Rust tests; `cargo check --all-targets`; `cargo fmt --all -- --check`; `npm run typecheck`; and `git diff --check`. Explicitly stage only Phase 3.5.3b files, commit, ff-only merge, push `origin/main`, and verify local/main/origin alignment. Do not start Phase 3.5.3c.

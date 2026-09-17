# Phase 3.5.3e Compatibility / Fixtures / Docs Closeout Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Freeze the completed Phase 3.5.3 approval and delete authority contracts as stable compatibility fixtures/tests/docs, run fresh non-mutating verification, and close the whole phase without adding product behavior.

**Architecture:** Reuse the existing shared approval/delete runtimes as the only behavioral authority. Add a compatibility fixture manifest plus focused Rust tests that serialize and exercise the real shared types, verify generation and multi-client isolation, and scan only schema/state identifiers for forbidden semantics. Documentation records the already-implemented authority boundaries and remaining upstream facts that are still not proven.

**Tech Stack:** Rust, serde/serde_json, Tokio test fixtures, sanitized JSON fixtures, Markdown.

**Spec:** User-approved Phase 3.5.3e contract in the 2026-09-17 task request and `docs/phase-3-5-3-approval-delete-authority.md`.

## Global Constraints

- No production behavior expansion and no real approval decision, Thread deletion, rollout deletion, or writer mutation.
- Approval/delete state machines, generation authority, exact-ID rules, retry/replay counts, and confirmed-only tombstone gate remain unchanged.
- Forbidden semantics remain absent: no RemoteClientIdentity, owner/lease/primary-deleter, FREE/AVAILABLE/RELEASED, force takeover, automatic retry, or automatic replay.
- Tests must exercise real shared types/cores with fake or deterministic inputs; documentation-only assertions are insufficient.
- Phase 3.5.4 is out of scope.

---

### Task 1: Compatibility fixture and focused test RED

**Files:**
- Create: `src-tauri/src/shared/codex_core/phase_3_5_3_compatibility_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`
- Create: `docs/fixtures/app-server/phase-3-5-3-compatibility/authority-contract.json`
- Create: `docs/fixtures/app-server/phase-3-5-3-compatibility/fixture-family-manifest.json`

- [x] Register tests that cover approval schema/provenance/parity/generation/multi-client contracts, delete schema/attempt/parity/exact-ID/concurrency/rejection/direct-evidence/tombstone contracts, generation hierarchy, zero retry/replay, and forbidden semantics.
- [x] First run the focused test module with the compatibility fixtures absent and retain the genuine missing-contract RED.
- [x] Add only the sanitized compatibility fixtures required to satisfy the frozen contracts.
- [x] Run the focused test module GREEN.

### Task 2: Cross-slice compatibility freeze and non-regression

**Files:**
- Modify the new focused test module and fixtures only as required by real shared interfaces.

- [x] Validate all required Phase 3.5.3 fixture families resolve to existing sanitized fixtures.
- [x] Prove same request ID across generations is isolated, shared clients observe one shared session/app-server truth, same-thread mutation dispatch remains at most one, and different exact Thread keys remain independent.
- [x] Prove direct upstream evidence outranks stale transport uncertainty and only confirmed exact-ID delete evidence opens the tombstone gate.
- [x] Prove retry/replay counters remain zero and forbidden schema/state identifiers are absent.

### Task 3: Authoritative documentation and evidence closeout

**Files:**
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-3-approval-delete-authority.md`
- Modify: `docs/fixtures/app-server/README.md`
- Create: `docs/evidence/phase-3-5-3e/compatibility-closeout.md`
- Modify: `docs/evidence/README.md`
- Modify: `docs/phase-3-5-0-remote-mobile-forensics.md`

- [x] Freeze approval observation/decision and delete exact-ID/unknown/rejection/tombstone authority without adding semantics.
- [x] Freeze the RemoteHost/daemon/transport/workspace/app-server generation hierarchy and multi-client scoping.
- [x] Record stale/unknown/direct-evidence precedence, zero retry/replay, and canonical-versus-projection boundaries.
- [x] Preserve exact upstream duplicate/late approval behavior and concurrent duplicate-delete ordering as NOT PROVEN.
- [x] Mark Phase 3.5.3a-e and the whole Phase 3.5.3 PASS / COMPLETE / FROZEN; keep Phase 3.5.4 NOT STARTED.

### Task 4: Fresh verification and scoped Git closeout

**Files:**
- Stage only files owned by Tasks 1-3.

- [x] Run focused Phase 3.5.3 compatibility, approval, delete, provenance, writer/subscription/runtime, reconnect, and daemon-restart tests.
- [x] Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`.
- [x] Inspect the scoped diff and main-worktree local changes; do not stash/reset/restore/clean or stage unrelated files.
- [ ] Explicitly stage only Phase 3.5.3e files, commit, ff-only merge to main, push `origin/main`, and verify local/remote refs.

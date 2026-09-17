# Phase 3.5.4c Authoritative Recovery / Hydration Implementation Plan

> Execute in `F:\AI\CodexMonitor\.worktrees\phase-3-5-4c-authoritative-hydration` on branch `codex/phase-3-5-4c-authoritative-hydration`. Preserve the four user-local changes in the main checkout. Never replay a mutation.

## Goal

Build a generation-safe, read-authoritative recovery path for reload, reconnect, WorkspaceSession replacement, and daemon restart. Each projection coverage advances independently. Stale in-flight reads are rejected before frontend projection application. Writer, subscription, runtime, approval, approval-decision, and delete authorities remain separate inside one read-only snapshot.

## Task 1: Establish RED contracts and sanitized fixtures

Files:

- Add `src/features/app/orchestration/authoritativeRecovery.test.ts`
- Add `src-tauri/src/shared/codex_core/authoritative_recovery_tests.rs`
- Add sanitized fixtures under `src-tauri/tests/fixtures/phase-3-5-4c-authoritative-hydration/`

Steps:

1. Add the eleven required scenario fixtures (reload, same-daemon reconnect, session replacement, daemon restart, partial hydration, selected read, observation snapshot, stale response, event race, read failure, multi-client convergence).
2. Add frontend RED tests for canonical read ordering, coverage independence, single-flight, mutation non-replay, and generation compare-at-apply.
3. Add Rust RED tests for aggregate observation projection, generation-safe completion, independent freshness transitions, and absence/non-delete semantics.
4. Run only the new focused tests and preserve their real failures as RED evidence.

## Task 2: Add the shared read-only observation snapshot

Files:

- Modify `src-tauri/src/shared/codex_core.rs`
- Modify observation modules under `src-tauri/src/shared/codex_core/`
- Modify `src-tauri/src/codex/mod.rs`
- Modify `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`
- Modify `src-tauri/src/lib.rs`
- Modify `src/types.ts`
- Modify `src/services/tauri.ts`

Steps:

1. Define a serializable aggregate snapshot that contains separate writer, subscription, runtime, approval, decision-provenance, and delete fields plus exact generation identity.
2. Implement one shared query core used by App and daemon; it must not dispatch any mutation or establish a session.
3. Instrument only `observation_snapshot` freshness, mark HYDRATING before the query, and mark CURRENT only if the same WorkspaceSession/AppServer generations are still current at completion.
4. Expose the stable read-only command/RPC and TypeScript contract.
5. Keep existing writer-only query behavior compatible.

## Task 3: Make authoritative read completion generation-safe

Files:

- Modify `src-tauri/src/shared/codex_core.rs`
- Modify `src-tauri/src/shared/projection_freshness.rs`
- Modify `src/services/tauri.ts`
- Modify `src/features/threads/hooks/useThreadActions.ts`

Steps:

1. Re-check current session generations after thread/list, thread/read, and observation queries before recording CURRENT.
2. Classify authoritative read failures as UNAVAILABLE or UNKNOWN from direct evidence instead of flattening every error.
3. Have frontend list/read wrappers require a CURRENT matching freshness snapshot before returning a payload eligible for reducer application.
4. Preserve existing list/detail data and anchors on stale/unavailable outcomes; never synthesize ABSENT or tombstones.

## Task 4: Add canonical recovery orchestration

Files:

- Add `src/features/app/orchestration/authoritativeRecovery.ts`
- Modify `src/features/workspaces/hooks/useWorkspaceRestore.ts`
- Modify `src/features/app/hooks/useMainAppWorkspaceLifecycle.ts`
- Modify focused hook tests as required

Steps:

1. Implement the fixed chain: workspace list, connect only if unavailable, thread list, selected exact read, observation snapshot, optional existing safe live attach.
2. Deduplicate by workspace/current-generation with a process-local single-flight map; do not create owner/lease/client identity concepts.
3. Use the chain for reload/restore. Keep established focus/reconnect reads generation-gated and hydrate the selected observation after the exact read.
4. Never call resume, approval decision, delete, upstream unsubscribe, takeover, or any generic retry/replay path.

## Task 5: GREEN verification and regression

Files:

- Update tests and fixtures only as required by the minimal implementation

Steps:

1. Run new frontend/Rust focused tests and make all RED cases GREEN.
2. Run ProjectionFreshness, event-generation, WorkspaceSession/app-server lifecycle, observation parity, reconnect/restart, multi-client, approval/delete no-replay, and writer/subscription/runtime regressions.
3. Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, frontend focused/full tests, and `git diff --check`.
4. Record the known six zh-CN baseline failures only if they reproduce unchanged.

## Task 6: Authority docs and closeout

Files:

- Update `README.md`
- Update `docs/codebase-map.md`
- Update current Phase 3.5.4 authority/evidence/roadmap docs found in the repository

Steps:

1. Document coverage-by-coverage recovery, generation-safe apply, read-only observation aggregation, single-flight, and zero mutation replay.
2. Explicitly freeze authority precedence, missing-data non-inference, multi-client convergence, and 3.5.4d exclusions.
3. Review scoped diff and forbidden terms/fields.
4. Explicitly stage only 3.5.4c files, commit, ff-only merge to main, push `origin/main`, and verify local/main/remote equality.
5. If dirty-main safety blocks the ff-only merge, stop without touching the user-local files and request authorization.

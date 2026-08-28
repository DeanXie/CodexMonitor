# Phase 2.4.2 Cross-View Deletion Reconciliation Implementation Plan

> Formal design: `docs/superpowers/specs/2026-08-28-phase-2-4-2-cross-view-deletion-reconciliation-design.md` (`dacbb34`).

**Goal:** Make a confirmed official `thread/delete` persist a crash-recovery tombstone, retire the exact canonical root and confirmed descendants from Registry/Watcher/checkpoint state, publish the clean snapshot, and clear only CodexMonitor-owned view caches.

**Frozen boundary:** Phase 2.1–2.4.1b behavior is preserved. This plan does not modify Phase 1 Runtime semantics, Desktop private state, SourceEnvelope/token authority, or Phase 2.5 code.

**Commit policy:** The user requires one final Phase 2.4.2 commit. Do not make intermediate commits; retain RED/GREEN verification output in the task record and inspect the complete staged diff before the single commit.

---

## Task 1: Persisted tombstone contract and Registry retirement

**Files:**
- Create: `src-tauri/src/shared/global_sources_core/deletion_tombstone.rs`
- Modify: `src-tauri/src/shared/global_sources_core/mod.rs`
- Modify: `src-tauri/src/shared/global_sources_core/source_registry.rs`

1. Add failing shared-core tests for versioned JSON round-trip, atomic save/reload, UUID idempotency, exact root/descendant identity, and Desktop exact-ID state transitions.
2. Add failing Registry tests proving retirement removes canonical state and rejects LIVE, NEAR LIVE, HISTORICAL, identity observation, and batch replay while preserving a different fullThreadId.
3. Implement the spec-exact `DeletionTombstone` document and atomic store (temporary file, file `sync_all`, rename) plus idempotent evidence merge.
4. Add a tombstoned-key set to `SourceAuthorityRegistry`; remove thread state and its observation keys on retirement and return `Ok(false)` for every later ingest of that exact key.
5. Run the focused Rust tests and `cargo fmt --check`.

## Task 2: Watcher retirement, checkpoint rewrite, and crash recovery

**Files:**
- Modify: `src-tauri/src/shared/global_sources_core/rollout_watcher.rs`
- Modify: `src-tauri/src/shared/global_sources_core/rollout_watch_service.rs`
- Modify: `src-tauri/src/shared/global_sources_core/rollout_checkpoint.rs` only if a test-only/read helper is required
- Modify: `src-tauri/src/shared/global_sources_core/runtime_config.rs`
- Modify: `src-tauri/src/global_sources/mod.rs`
- Modify: `src-tauri/src/global_sources/diagnostics.rs`

1. Add failing tests for active source retirement, restored checkpoint filtering, discovered/replayed owner rejection, stale filesystem signal rejection, checkpoint cleanup, unrelated-source preservation, pending-tombstone startup recovery, duplicate notification idempotency, and no new missing-file (`os error 2`) failure after retirement.
2. Load tombstones before checkpoints. Seed Registry tombstoned identities and reject retired source identities before discovery/reads.
3. Add a service command carrying the exact deletion operation and a one-shot result. Persist `pending` before any mutation, retire Registry/source state, rewrite the checkpoint, mark `completed`/`refreshPending`, and publish through the existing service event callback.
4. Keep failed checkpoint reconciliation pending and replay it on startup/next reconciliation. Never classify unrelated I/O failures as deletion.
5. Record only the deletion metadata permitted by the spec; add the tombstone path beside the existing checkpoint path.
6. Run focused watcher/service/diagnostic tests.

## Task 3: Official delete orchestration and app/daemon contract parity

**Files:**
- Modify: `src-tauri/src/codex/mod.rs`
- Modify: `src-tauri/src/shared/codex_core.rs` only to preserve/test the official request boundary as needed
- Modify: `src-tauri/src/backend/app_server.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Modify: `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`
- Modify: associated Rust tests

1. Add failing tests proving official delete failure never submits/persists a tombstone and successful deletion submits the frontend UUID plus exact confirmed descendants.
2. Extend the delete command/RPC payload with `descendantThreadIds` and `monitorDeleteOperationId`; keep the upstream app-server `thread/delete` payload unchanged.
3. After official success, resolve the workspace CODEX_HOME identity and await the unique rollout service reconciliation result. Return a reconciliation error if persistence/local cleanup is not accepted.
4. Forward `thread/deleted` exact-ID notifications as independent evidence without inventing an operation or descendant closure.
5. Keep local/remote RPC payload shapes aligned. If the daemon has no Global Source service, use the same persisted shared reconciliation boundary in its owned data directory rather than mutating Desktop-private state.
6. Run focused command/RPC/app-server tests.

## Task 4: Frontend request identity and Monitor-owned cache cleanup

**Files:**
- Modify: `src/services/tauri.ts`
- Modify: `src/services/tauri.test.ts`
- Modify: `src/features/threads/hooks/useDeleteThreadPrompt.ts`
- Modify: `src/features/threads/hooks/useDeleteThreadPrompt.test.tsx`
- Modify: `src/features/threads/utils/threadStorage.ts`
- Modify: `src/features/threads/hooks/useThreadStorage.ts`
- Modify: `src/features/threads/hooks/useThreadCodexParams.ts`
- Modify: `src/features/threads/hooks/useDetachedReviewTracking.ts`
- Modify: `src/features/threads/hooks/useThreads.ts`
- Modify: focused hook/integration tests

1. Add failing tests that confirmation generates one UUID and submits root plus sorted confirmed descendants, while failure leaves reducer and storage unchanged.
2. Add failing storage/integration tests proving Main and descendants clear activity, pins, custom names, Codex params, and detached-review links while unrelated workspace/thread entries remain. Preserve Archive behavior.
3. Implement scoped cleanup APIs that update both persistent storage and live hook refs. Call them only after backend acceptance, through `forgetThreads`.
4. Preserve the current draft/image cleanup, active Current Session removal, Phase 1 exclusion set, normal thread-list refresh, and canonical snapshot-driven Agent Monitor removal.
5. Run focused Vitest tests and typecheck.

## Task 5: Full verification, safety audit, and single commit

**Files:**
- Modify: `docs/app-server-events.md` only if the locally supported deletion-evidence routing documentation changes
- Modify: this plan only for canonical live-state corrections if implementation diverges materially

1. Run `cargo fmt --check`, focused Rust suites, `cargo test --lib`, and `cargo check`.
2. Run focused frontend suites, `npm run typecheck`, and `npm run test`. Compare any failures with the recorded frozen baseline (1078 pass / 6 known locale failures) and do not claim new failures as PASS.
3. Run a focused temporary-directory diagnostic: delete/retire a watched rollout, reconcile multiple times plus a stale signal, and assert the path's `os error 2` count does not increase after retirement.
4. Inspect `git status`, `git diff --check`, `git diff --stat`, and the complete diff. Confirm no write path targets `session_index.jsonl`, `.codex-global-state.json`, `state_5.sqlite`, Desktop caches, or Phase 2.5.
5. Stage every file with explicit paths, inspect `git diff --cached --check`, `--stat`, and `--name-status`, rerun fresh verification, then create exactly one Phase 2.4.2 commit.
6. Stop after the commit and report the nine requested outcomes; do not begin Phase 2.5.

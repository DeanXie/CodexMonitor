# Phase 3.5.4a — Projection Freshness Authority evidence

Status: **PASS / COMPLETE / FROZEN**.

## TDD evidence

- Model RED: the new focused contract suite first failed because the shared authority was absent; the behavioral stub run executed 22 tests with 15 failures and 7 passes.
- Daemon parity RED: the real daemon router returned `unknown method: get_projection_freshness`.
- Fixture RED: the fixture contract failed on the missing `not-hydrated.json` path.
- GREEN: 31 focused projection tests pass, including the real `thread/list`, exact `thread/read`, and observation-query boundaries; the shared App/daemon schema, read-only query, generation invalidation, source/coverage rules, and all ten sanitized compatibility fixtures pass.

## Frozen implementation

- Shared model: `src-tauri/src/shared/projection_freshness.rs`.
- Shared query and authoritative read instrumentation: `src-tauri/src/shared/codex_core.rs`.
- App adapter: `src-tauri/src/codex/mod.rs` and Tauri registration.
- Daemon adapter: `src-tauri/src/bin/codex_monitor_daemon.rs` and `rpc/codex.rs`.
- Frontend typed read wrapper: `src/services/tauri.ts` and `src/types.ts`.
- Fixtures: `docs/fixtures/projection-freshness/`.

The query is read-only. The implementation does not connect a workspace, resume/start a Thread or Turn, subscribe/unsubscribe, answer approvals, delete, retry, replay, recover, persist freshness, or manufacture canonical Thread truth.

## Fresh verification

Fresh closeout results:

- focused projection freshness: 31 passed, 0 failed;
- generation hierarchy: 60 passed, 0 failed;
- App/daemon parity: 10 passed, 0 failed;
- thread-read filters: 16 passed, 0 failed;
- writer/subscription/runtime: 110 passed, 0 failed;
- approval/delete: 134 passed, 0 failed;
- Remote request/transport: 33 passed, 0 failed;
- Rust all targets: 997 library + 907 daemon + 29 daemonctl + 1 config passed, 0 failed (7 ignored); auxiliary targets contained zero tests;
- `cargo check --all-targets`: passed;
- `cargo fmt --all -- --check`: passed;
- `npm run typecheck`: passed;
- `git diff --check`: passed.

# P4.1d-3 Runtime Validation Handshake Implementation Plan

**Goal:** Separate durable target commit from per-process runtime validation, then make App, daemon, and daemonctl enforce the corrected startup gate without touching real user data.

**Baseline:** `b3e6d69f8e2465bbb0f1cba4190ca88b09de08b1`, `0.7.68 · Build 4 · development`.

**Global constraints:** Tests use temporary roots, fake identities, synthetic settings/workspaces, and controlled child processes. No real migration, identity retirement, daemon operation, credentials, A3 target, P4.1e closeout, or P4.2 work. The existing P4.1e worktree remains untouched.

## Task 1: Durable commit and runtime-validation authority

**Files:**
- `src-tauri/src/shared/activation_foundation.rs`
- `src-tauri/src/shared/startup_activation.rs`
- `src-tauri/src/shared/activation_foundation_tests.rs`
- `src-tauri/src/shared/startup_activation_tests.rs`

1. Preserve the proven RED showing commit incorrectly writes `runtime_validated`.
2. Add RED coverage for fresh/migration commit, committed-profile validation, old runtime-validated revalidation, binding changes, journal-write failure, and no migration replay.
3. Make commit stop at `target_committed`.
4. Split static committed-profile validation from current-process readiness.
5. Add an internal runtime-validation completion operation that revalidates root/transaction under the existing short commit lock and writes `runtime_validated` only after the caller's required initialization succeeds.
6. Add a process-local `BLOCKED / VALIDATING / READY / FAILED` gate with one validation attempt at a time.

## Task 2: App startup handshake

**Files:**
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/lib.rs`
- `src/features/app/bootstrap/BootstrapBoundary.tsx`
- related Rust/frontend tests

1. Add RED coverage proving committed profiles enter restricted validation, not corrupt/recovery/normal access.
2. Build a candidate `AppState` from strict committed-profile metadata.
3. Complete persistent validation only after candidate construction and binding recheck.
4. Publish process READY and manage business state only after persistence succeeds.
5. Start global-source scanning and Remote/daemon callbacks only after READY.
6. Keep failed validation in the minimal bootstrap surface; direct normal business state remains unavailable.

## Task 3: Daemon and daemonctl handshake

**Files:**
- `src-tauri/src/bin/codex_monitor_daemon.rs`
- `src-tauri/src/bin/codex_monitor_daemonctl.rs`
- related production-entry tests

1. Add RED coverage for committed-profile daemon validation and daemonctl start readiness.
2. Daemon sequence: static validation → service lock → candidate state → isolated listener bind → binding recheck/persistent completion → READY → accept/dispatch.
3. Ensure failures before READY drop candidate resources and expose zero business RPC dispatch.
4. Make daemonctl `start` accept committed profiles but wait for authenticated daemon readiness; spawn alone is not success.
5. Keep `status`/`command-preview` read-only and prevent them from advancing the journal.

## Task 4: Contract, version, regression, and integration

**Files:**
- P4.1d authority/evidence docs
- `VERSION.json` and canonical projections (only through version tooling)
- scoped tests/fixtures

1. Document required App/daemon initialization and optional external dependencies.
2. Append the proven defect and correction without rewriting d-1/d-2 history.
3. Run focused RED→GREEN verification, P4.1a/b/c/d regressions, Rust all-targets/check/fmt, frontend focused/full, typecheck, production build, version check, and diff check.
4. Bump Build exactly once to Build 5 through canonical tooling because this is a production fix.
5. Explicitly stage only d-3 files; commit, ff-only merge, and push after all fresh gates pass.

## Review focus

- No path can turn file/schema validation alone into current-process READY.
- `runtime_validated` on disk never bypasses fresh per-process initialization.
- No App command/RPC, daemon accept loop, global scan, WorkspaceSession, or Remote autoconnect is exposed before current-process READY.
- App success is not daemon success; daemonctl never signs validation on the daemon's behalf.
- Journal/binding failures preserve committed data and keep business access closed.

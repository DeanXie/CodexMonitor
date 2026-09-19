# P4.1d-4b Controlled Legacy Migration Entry / Stop Evidence Implementation Plan

> **Execution note:** Implement this plan in the existing isolated worktree
> `F:\AI\CodexMonitor\.worktrees\phase-4-1d-4b-legacy-migration-entry` on
> `codex/phase-4-1d-4b-legacy-migration-entry`. Do not touch the paused P4.1e
> worktree.

**Goal:** Add the controlled local legacy-migration product entry and prove
native stop evidence at the protected identity-retirement boundary, without
changing the d-3 runtime-validation handshake or executing a real user cutover.

**Architecture:** A shared migration-entry coordinator owns opaque preview
records and calls the existing migration/activation cores. The Tauri bootstrap
adapter exposes sanitized preview and explicit-confirmation commands. A
Windows-native provider creates a backend-private stop guard that is consumed
inside the existing activation lock/identity-protection critical section. The
frontend remains a restricted bootstrap view until restart and d-3 validation.

**Tech stack:** Rust/Tauri, React/TypeScript, Windows `windows-sys`, Vitest,
Rust unit/integration tests, existing version and build-storage tooling.

**Approved design:**
`docs/superpowers/specs/2026-09-19-p4-1d-4b-controlled-legacy-migration-entry-design.md`

---

## Task 1: Freeze the RED contract at the shared-core boundary

**Files:**
- Modify: `src-tauri/src/shared/activation_foundation.rs`
- Modify: `src-tauri/src/shared/activation_foundation_tests.rs`
- Create: `src-tauri/src/shared/legacy_migration_entry.rs`
- Create: `src-tauri/src/shared/legacy_migration_entry_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`

1. Add failing tests proving an untrusted `ConfirmedStopped` value can
   currently reach `prepare_migration_activation` and that no product
   coordinator binds preview, source snapshot, and one-time intent.
2. Define `LegacyMigrationPreviewId`, a sanitized serializable preview DTO, a
   backend-private bound preview record, and a one-time in-memory preview
   registry keyed by the opaque ID.
3. Define an injected `LegacyStopEvidenceProvider` interface whose production
   result is a non-serializable guard/context with only `Running`, `Unknown`,
   or `VerifiedQuiescentWithinSupportedScope` outcomes.
4. Move the preparation/commit orchestration behind a coordinator method that
   accepts the preview ID and exact intent, re-runs startup and migration
   inspection, verifies all preview bindings, and rejects expired/reused or
   changed evidence before staging.
5. Remove the ordinary production ability to pass
   `LegacyProcessStopEvidence::ConfirmedStopped` directly. Keep deterministic
   fake-provider injection only under shared-core tests.
6. Run the focused shared-core tests through `npm run rust:test -- <filter>`
   and retain the initial failing assertions as RED evidence before GREEN.

## Task 2: Implement native Windows stop-evidence acquisition

**Files:**
- Create: `src-tauri/src/shared/legacy_process_stop.rs`
- Create: `src-tauri/src/shared/legacy_process_stop_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

1. Add Windows-only RED tests with controlled child processes for live legacy
   process, cached identity with closed file, cleanly stopped process, access
   denial/ambiguity, PID reuse protection via creation time, and executable or
   root mismatch.
2. Extend only the required `windows-sys` feature set for native process
   enumeration, `OpenProcess`, executable-image lookup, process creation-time
   lookup, waiting/liveness checks, and handle lifetime management.
3. Implement the supported-scope provider without logging full command lines.
   Bind candidate evidence to executable identity, process creation time, and
   legacy-root relation. Any incomplete enumeration or handle query returns
   `Unknown`, never quiescent.
4. Make the verified guard short-lived and non-cloneable. Before retirement it
   must recheck every bound process handle plus a fresh supported-scope
   enumeration; churn or mismatch invalidates it.
5. Provide a non-Windows implementation that reports unsupported/unknown and
   therefore cannot retire a legacy identity.
6. Run the focused native tests on Windows. If the supported success path
   cannot produce verified quiescence or the cached-identity process is not
   blocked, stop implementation and report the exact native limitation.

## Task 3: Bind the guard to the protected retirement critical section

**Files:**
- Modify: `src-tauri/src/shared/activation_foundation.rs`
- Modify: `src-tauri/src/shared/activation_foundation_tests.rs`
- Modify: `src-tauri/src/shared/legacy_migration_entry.rs`
- Modify: `src-tauri/src/shared/legacy_migration_entry_tests.rs`

1. Add RED tests for lock order, stopped-then-restarted process, identity
   mutation after preview, source mutation after staging, target/staging
   replacement, evidence reuse, and concurrent confirmations.
2. Split migration preparation into a pre-retirement phase that builds and
   protects recovery material, and a retirement/commit phase that runs only
   with the activation commit lock, `ProtectedLegacyIdentity`, and verified
   native guard held together.
3. Revalidate preview root/schema/policy/snapshot and journal/transaction
   bindings after acquiring the commit lock, then acquire identity protection,
   revalidate identity file identity, acquire/revalidate native stop evidence,
   and only then call `replace_with`.
4. Keep the journal transitions exactly `prepared → legacy_identity_retired →
   target_committed`; never write `runtime_validated` in this path.
5. Prove failed/unknown/running evidence causes zero legacy write and zero
   target commit; prove failure after protected recovery material preserves it
   under d-1/d-4a cleanup rules.
6. Prove `retired_v2` with the exact transaction resumes through d-4a recovery
   without requesting first-retirement stop evidence or replacing identity a
   second time.
7. Prove successful confirmation commits once, returns restart-required, and
   requires the existing current-process runtime validation after restart.

## Task 4: Add the Tauri bootstrap adapter and restricted UI flow

**Files:**
- Modify: `src-tauri/src/bootstrap.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/services/tauri.ts`
- Modify: `src/services/tauri.test.ts`
- Modify: `src/features/app/bootstrap/BootstrapBoundary.tsx`
- Modify: `src/features/app/bootstrap/BootstrapBoundary.test.tsx`

1. Add frontend and Rust RED tests for absent migration entry, preview secrecy,
   explicit confirmation, cancellation, stale/reused preview, no trusted
   `confirmedStopped` input, and business IPC remaining blocked.
2. Extend `BootstrapState` with the shared coordinator and production native
   provider. Add bootstrap-only commands `preview_legacy_migration` and
   `confirm_legacy_migration` using current naming conventions.
3. Register only those commands as bootstrap invocations. Do not add daemon
   RPC methods or normal-business bypasses.
4. Add TypeScript wrappers that send no source path, process ID, stop boolean,
   identity, token, or unrestricted configuration. Confirmation sends only the
   preview ID and fixed explicit intent.
5. Render the sanitized migration categories, exclusions, warnings, and
   conflicts in `BootstrapBoundary`; require a deliberate preview-then-confirm
   interaction and retain an explicit cancel/no-op path.
6. On success, show the existing restart-required state. Do not initialize
   normal `MainApp`, global sources, Remote connections, WorkspaceSessions, or
   business IPC in the current process.
7. Add adapter tests proving native provider failures remain precise
   `running`/`unknown` blocks and never become generic fresh/recovery success.

## Task 5: Production-path and recovery integration tests

**Files:**
- Modify: `src-tauri/src/bootstrap.rs` test module or extract focused bootstrap tests
- Modify: `src-tauri/src/shared/startup_activation_tests.rs`
- Modify: `src-tauri/src/shared/activation_foundation_tests.rs`
- Create: `docs/fixtures/phase-4-1d-4b-legacy-migration-entry/contract.json`

1. Run a full temporary-root chain through production entrypoints: legacy
   startup inspection → sanitized preview → explicit confirmation → controlled
   child quiescence → protected retirement → target commit → restart-required
   → new-process runtime validation → business READY.
2. Cover migration-core preview conflict, unsupported schema, corrupt source,
   missing/retired/replaced identity, Remote token/pin exclusion, and no
   CODEX_HOME/canonical data copying.
3. Cover cancel, running process, unknown/access-denied process, process restart
   inside the critical window, source/target/staging mutation, duplicate
   confirmation, and concurrent confirmation; assert zero retirement/commit
   where required.
4. Cover interruption before retirement, after identity replacement, and after
   target move. Verify the existing d-4a recovery adapter reaches the correct
   disposition without a second retirement, UUID change, or ordinary cleanup
   of identity recovery material.
5. Assert no real user paths, services, credentials, daemon processes, or
   Remote/thread mutations are accessed by the fixture.

## Task 6: Freeze authority docs, version, and evidence

**Files:**
- Create: `docs/phase-4-1d-4b-controlled-legacy-migration-entry.md`
- Create: `docs/evidence/phase-4-1d-4b/README.md`
- Modify: `README.md`
- Modify: `docs/phase-4-0-truth-release-boundary.md`
- Modify: `docs/phase-4-1-closeout.md`
- Modify: `docs/phase-4-1d-2-startup-cutover.md`
- Modify: `docs/phase-4-1d-4a-recovery-root-authority.md`
- Modify: `docs/evidence/README.md`
- Modify: `docs/evidence/phase-4-1e/README.md`
- Modify: `docs/fixtures/phase-4-1-closeout/contract.json`
- Modify via canonical tooling: `VERSION.json` and generated version projections

1. Record R01/R03 as resolved only after the product-entry and native-provider
   tests pass; retain R04/R06/R11 as open and P4.1e as paused.
2. Document the supported Windows legacy-process scope and explicitly list
   unverified historical binaries/process shapes as outside the proof.
3. Record that real user migration, installed-package validation, and real
   identity retirement were not executed.
4. Add the sanitized fixture contract and RED/GREEN evidence references.
5. After production changes and before final full verification, run
   `npm run version:bump -- build` exactly once and `npm run version:sync`.
   Confirm `0.7.68 · Build 7 · development` with `npm run version:check`.

## Task 7: Fresh verification, review, and Git closeout

**Files:** No new behavior beyond scoped fixes required by failing gates.

1. Run focused d-4b shared/native/bootstrap/frontend tests.
2. Run d-4a, d-3, d-1 foundation, P4.1a/b/c, updater/Sentry, version, and
   migration compatibility regressions.
3. Run `npm run rust:test`, `npm run rust:check`, `cargo fmt --all -- --check`
   through the repository-approved wrapper where applicable, `npm run
   typecheck`, the focused/full frontend suites, `npm run tauri:build`, `npm
   run version:check`, `npm run storage:report`, and `git diff --check`.
4. Confirm only the six named historical frontend baseline failures, if still
   present, qualify for the existing waiver; no new failure is waived.
5. Inspect the complete scoped diff and run `npm run storage:closeout --
   --accepted`, then `npm run storage:closeout -- --apply --accepted` only after
   the candidate is accepted.
6. Explicitly stage only P4.1d-4b files; never use `git add -A` or `git add .`.
7. Commit, fast-forward merge into `main`, push `origin/main`, and verify local
   main, origin/main, and GitHub main agree. Stop on divergence, conflict,
   security denial, or native-proof failure; never stash/reset/restore/clean or
   force push.

## Review focus

- The frontend cannot supply or forge stop evidence.
- Stop evidence is fresh, native, scoped, backend-private, and consumed inside
  the protected retirement section after binding revalidation.
- A live legacy process that cached identity still blocks migration.
- Preview data is sanitized and stale previews cannot commit.
- The implementation reuses migration/activation authority and does not add a
  daemon migration RPC, auto-stop, retry, new owner/lease identity, or Remote
  mutation.
- Successful migration stops at `target_committed`; d-3 alone publishes READY.
- P4.1e work and the main checkout's four user-local modifications remain
  untouched.

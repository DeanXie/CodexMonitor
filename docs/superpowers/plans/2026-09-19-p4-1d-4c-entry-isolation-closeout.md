# P4.1d-4c Entry Gate Evidence / Restart UX / Isolation Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve R04, R06, and the executed-test scope of R11 without real cutover or P4.1e/P4.2 work.

**Architecture:** Reuse `BootstrapState` as the sole process-readiness authority. Add one small native-action gate shared by menu and current tray callbacks, one setup-effect decision seam, and a test-only process environment harness. Freeze the already-selected restart-required policy with production-path tests and authority documents.

**Tech Stack:** Rust/Tauri 2, React/TypeScript/Vitest, Node contract tests, repository storage wrappers.

**Spec:** `docs/superpowers/specs/2026-09-19-p4-1d-4c-entry-isolation-closeout-design.md`

## Global Constraints

- Use only temporary roots, fake identities, fake tokens/endpoints, and controlled children.
- Never read or migrate real user data, retire a real identity, stop a real service, launch WebView, or install/launch a bundle.
- Keep d-3 current-process runtime validation as the only READY authority.
- Keep P4.1e paused and P4.2 not started.
- Use worktree storage wrappers; do not create an in-tree Cargo target.
- Bump Build once only if production behavior changes.

## Review Focus

- Missing BootstrapState must fail closed for business native actions while shell actions remain usable.
- A stale restart-required response must not allow BootstrapBoundary children to render.
- A controlled child must not inherit CODEX_HOME, app-data roots, temp/cache roots, or daemon token.
- Relative explicit data roots and changed cwd must not escape the sandbox.
- Existing macOS tray behavior must be gated without creating a Windows tray implementation.

---

### Task 1: Native menu, tray, and startup business gate

**Files:**
- Modify: `src-tauri/src/bootstrap.rs`
- Modify: `src-tauri/src/menu.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: colocated Rust tests in the same modules

**Interfaces:**
- Consumes: `BootstrapState::business_access_allowed()` and `RuntimeProcessState`.
- Produces: one shared native action authorization helper and one startup-effect plan used by production handlers/setup.

- [ ] Add production-entry tests that invoke menu/tray dispatch seams and observe blocked side-effect counts; run them and verify RED because business events currently bypass readiness.
- [ ] Add startup decision tests for BLOCKED, VALIDATING, FAILED, and READY; verify the missing seam is RED.
- [ ] Implement the minimal shared gate and wire actual menu/tray handlers and setup decisions.
- [ ] Run focused Rust tests and verify GREEN with shell actions still allowed and business side effects zero until READY.

### Task 2: Restart-required contract freeze

**Files:**
- Modify: `src-tauri/src/bootstrap.rs`
- Modify: `src/features/app/bootstrap/BootstrapBoundary.test.tsx`
- Modify only if required by RED: `src/features/app/bootstrap/BootstrapBoundary.tsx`

**Interfaces:**
- Consumes: activation/recovery results and `BootstrapStatus.restartRequired`.
- Produces: frozen fresh/migration/recovery current-process blocking and next-process validation behavior.

- [ ] Add missing fresh, migration, recovery, and UI contract tests; verify the missing coverage is RED without breaking already-correct behavior.
- [ ] Make only the minimal correction required by the failing production-path test.
- [ ] Verify commit stops at `target_committed`, current process remains blocked, restart is explicit, and a clean process can validate once without remigration or UUID replacement.

### Task 3: Controlled child-process isolation harness

**Files:**
- Create: `src-tauri/tests/support/mod.rs`
- Create: `src-tauri/tests/support/isolated_process_environment.rs`
- Modify: `src-tauri/tests/runtime_validation_handshake.rs`
- Create: `src-tauri/tests/p4_1_process_isolation.rs`

**Interfaces:**
- Produces: `IsolatedProcessEnvironment` which applies exact sandbox roots and removes inherited daemon credentials from `std::process::Command`.
- Consumes: actual daemon/daemonctl binaries supplied by Cargo integration tests.

- [ ] Add a real child test showing the current command setup inherits ambient roots/token; verify RED without reading their values.
- [ ] Implement the test-only harness and apply it to P4.1 production-entry child commands.
- [ ] Prove sandbox app-data/CODEX_HOME/temp/cache, cwd independence, absolute explicit data-dir containment, synthetic token/endpoint behavior, and inherited-token removal.
- [ ] Run focused integration tests and verify GREEN.

### Task 4: Authority, version, regression, and closeout

**Files:**
- Modify: `docs/phase-4-1-closeout.md`
- Create: `docs/phase-4-1d-4c-entry-isolation-closeout.md`
- Create: `docs/evidence/phase-4-1d-4c/README.md`
- Create: `docs/fixtures/phase-4-1d-4c-entry-isolation-closeout/contract.json`
- Modify: `docs/fixtures/phase-4-1-closeout/contract.json`
- Modify: `scripts/phase-4-1-closeout-compatibility.test.mjs`
- Modify via canonical tooling if production changed: `VERSION.json` and projections

**Interfaces:**
- Consumes: Tasks 1-3 behavior and evidence.
- Produces: R01-R12 compliance authority, d-4c evidence, Build 8 when required, and Git-closeout candidate.

- [ ] Add/adjust contract tests to expect R04/R06/R11 resolved; verify RED against the old closeout authority.
- [ ] Update canonical docs/fixtures and bump/sync Build exactly once if Task 1 changed production behavior.
- [ ] Run focused d-4c, d-4b, d-4a, d-3, P4.1 compatibility, version, Rust all-targets/check/fmt, frontend focused/full, typecheck, production Tauri build, and diff checks.
- [ ] Confirm only the six named zh-CN/date baseline failures, zero new waiver, zero real operations, no WebView/installer launch, and explicit scoped diff.
- [ ] Commit, final-review the whole branch, then ff-only merge and push only if main has not advanced incompatibly.

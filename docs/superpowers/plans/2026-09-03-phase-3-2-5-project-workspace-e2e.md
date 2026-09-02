# Phase 3.2.5 Project / Workspace Interoperability E2E Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove with executable contract fixtures and sanitized real-environment evidence that the frozen Phase 3.2.1–3.2.4 Workspace, scoped relation, runtime reconciliation, and Desktop Project projection contracts work together.

**Architecture:** Add no new identity or resolver model. A table-driven fixture suite drives the existing `workspace_interop_core` types and reducers through Gates A–L; bounded real probes use isolated test Threads and the existing read-only Desktop metadata adapter, then persist only sanitized evidence and explicit `UNKNOWN`, `NOT OBSERVED`, `NOT TESTED`, or `NOT RECOVERABLE BY CURRENT CONTRACT` outcomes.

**Tech Stack:** Rust unit/integration-style tests, serde JSON fixtures, Codex app-server/CLI test Threads, read-only Desktop JSON/SQLite inspection, Markdown/JSON evidence.

**Spec:** User-approved Phase 3.2.5 Contract Fixtures / Focused E2E request dated 2026-09-03.

## Global Constraints

- `CodexThreadKey`, `WorkspaceKey`, `NormalizedRootLocator`, `ExecutionEnvironmentKey`, `ThreadWorkspaceRelation`, `ORIGIN`, `TURN_EXECUTION`, `DesktopProjectProjection`, and `ASSIGNED / AMBIGUOUS / UNASSIGNED / UNKNOWN` are frozen.
- `Thread identity != Workspace identity != Desktop Project identity`.
- `Workspace = execution environment + normalized root`; Desktop Project is a Desktop Surface organization projection.
- Never infer Desktop Project assignment from cwd, configured Project roots, sidebar visibility, catalog membership, display name, or Monitor `WorkspaceEntry.id`.
- Do not write Desktop Project assignment, `.codex-global-state.json`, `state_5.sqlite`, `codex-dev.db`, `local_thread_catalog`, or sidebar state.
- Use only new isolated test Threads for mutating real probes; existing Phase 3.1 A2 evidence may be read without modifying that Thread.
- Do not start Phase 3.3, Phase 3.4, Remote/Mobile, UI work, or Phase 2 Global Source Core refactoring.
- If a Gate exposes a Phase 3.2 implementation defect, first add a focused failing regression, verify RED, make the minimal production change, and rerun the affected Gate before continuing.

---

### Task 1: Add executable Gate A–L contract fixtures

**Files:**
- Create: `docs/fixtures/project-workspace-interop/contract-scenarios.json`
- Create: `docs/fixtures/project-workspace-interop/README.md`
- Create: `src-tauri/src/shared/workspace_interop_core/contract_fixture_tests.rs`
- Modify: `src-tauri/src/shared/workspace_interop_core.rs`

**Interfaces:**
- Consumes: `resolve_workspace_root`, `RuntimeWorkspaceReconciler`, `ThreadWorkspaceRelationStore`, `resolve_desktop_project_projection`, and `DesktopMetadataSnapshot`.
- Produces: one executable acceptance suite whose test names and fixture IDs map directly to Gates A–L.

- [ ] **Step 1: Write literal fixture records**

Store Windows path literals, execution environment keys, configured Workspace records, direct legacy/app-server Project assignments, aliases, migration state, expected canonical candidates, expected relation basis, and expected states. Include unique root, nested root, duplicate normalized root, equal-longest distinct candidates, overlapping Desktop Projects, explicit assignment, origin A / Turn B, child direct/fallback, CLI/Monitor creator labels, absent/invalid/no-match cwd, conflict, and schema drift.

- [ ] **Step 2: Write table-driven acceptance tests against real shared-core code**

Each assertion uses hand-derived literal expectations. Tests must prove:

```text
A unique root                    -> Workspace ASSIGNED, one candidate, stable Thread key
B nested roots                  -> longest component-boundary root
C duplicate normalized roots    -> one canonical WorkspaceKey, ASSIGNED
D distinct equal-longest keys   -> AMBIGUOUS, deterministic candidates, no workspace route
E overlapping Project roots     -> Workspace ASSIGNED; Project UNASSIGNED or UNKNOWN without direct assignment
F explicit Desktop assignment   -> ASSIGNED; aliases dedup; SQLite NULL cannot negate legacy evidence
G origin A / later Turn B       -> independent relations under one fullThreadId
H child direct/fallback         -> direct wins; confirmed ASSIGNED parent may infer; ambiguous/unknown parent cannot
I external CLI semantics        -> rollout cwd affects Workspace only; Project and producer remain independent
J Monitor-created semantics     -> thread/start cwd establishes ORIGIN; no synthetic Project assignment
K missing/invalid/no-match cwd  -> UNKNOWN / UNKNOWN / UNASSIGNED respectively
L Project conflict/schema drift -> AMBIGUOUS / UNKNOWN with diagnostics; Workspace relation unaffected
```

- [ ] **Step 3: Run the focused suite**

Run: `cargo test --lib phase_3_2_5_contract -- --nocapture`

Expected: all fixture Gates pass. A failure is evidence of a contract defect and triggers Task 2.

### Task 2: Correct only proven contract defects

**Files:**
- Test: the narrow test file that exposed the failing Gate.
- Modify: only the existing Phase 3.2.1–3.2.4 implementation file responsible for the wrong result.

**Interfaces:**
- Consumes: an observed Gate failure with Expected, Actual, Evidence, and Root Cause.
- Produces: minimal GREEN without changing frozen architecture.

- [ ] **Step 1: Preserve a minimal failing regression**

Name the exact wrong branch or side effect the regression catches and run it alone. Confirm it fails for the observed reason.

- [ ] **Step 2: Make the smallest production correction**

Do not add new identity types, resolvers, persistence, readers, UI, or write paths.

- [ ] **Step 3: Verify GREEN and affected regressions**

Run the failing test, the complete Phase 3.2.5 fixture suite, `workspace_interop_core`, app-server runtime tests, and Phase 3.1 exact-ID admission/resume tests.

### Task 3: Run bounded real-environment probes

**Files:**
- Create: `docs/evidence/phase-3-2-5/environment.json`
- Create: `docs/evidence/phase-3-2-5/gate-matrix.json`
- Create: `docs/evidence/phase-3-2-5/restart-reconstruction.json`

**Interfaces:**
- Consumes: current read-only Desktop metadata, current Monitor Workspace configuration, one new CLI Thread, one new Monitor-created Thread, and read-only Phase 3.1 A2 rollout evidence.
- Produces: sanitized Gate evidence containing Thread IDs, Workspace keys, locators/roots needed for the contract, Project ID aliases, basis, state, confidence/provenance, and no prompt/response/account/token/cookie/private file content.

- [ ] **Step 1: Capture pre-probe environment and Desktop private-state hashes**

Record current commit, CLI/runtime versions, relevant root availability, migration flags, assignment-source readability, and hashes for existing Desktop global-state/SQLite/catalog files. Do not persist existing unrelated Thread titles or IDs.

- [ ] **Step 2: Create one isolated CLI Thread**

Run the installed Codex CLI in `F:\AI\CodexMonitor` with the test instruction `Phase 3.2.5 isolated CLI E2E. Return only PHASE_3_2_5_CLI_OK.` Capture its new fullThreadId and rollout cwd. Do not resume or modify any existing user Thread.

- [ ] **Step 3: Create one isolated Monitor/app-server Thread**

Use the CodexMonitor saved project with local environment so `thread/start.cwd` is `F:\AI\CodexMonitor`. The test instruction is `Phase 3.2.5 isolated Monitor E2E. Return only PHASE_3_2_5_MONITOR_OK.` Capture only the new fullThreadId, Turn ID, cwd basis, and observable Project projection state.

- [ ] **Step 4: Read Phase 3.1 A2 and Desktop assignment evidence**

Read only the `session_meta.cwd` and `turn_context.cwd` fields for A2 Thread `01a05dd5-648d-7432-a4ad-e32d60474763`. Select one existing explicit legacy assignment read-only, resolve its alias/migration state through `DesktopMetadataReader`, and persist only allowed IDs/states/provenance.

- [ ] **Step 5: Prove read-only behavior**

Recompute Desktop private-state hashes. Every pre/post hash must match. Any missing source remains explicit rather than synthesized.

### Task 4: Run restart/reconstruction Gate

**Files:**
- Test: `src-tauri/src/shared/workspace_interop_core/contract_fixture_tests.rs`
- Evidence: `docs/evidence/phase-3-2-5/restart-reconstruction.json`

**Interfaces:**
- Consumes: isolated real Thread evidence and `RuntimeWorkspaceReconciler` reconstruction inputs.
- Produces: deterministic pre/post reconstruction comparison and an explicit recoverability classification for each relation scope.

- [ ] **Step 1: Reconstruct twice in opposite observation orders**

Assert identical WorkspaceKey, effective ORIGIN, Project projection state, and canonical candidate order.

- [ ] **Step 2: Classify Turn recovery honestly**

If current persisted/thread-list evidence cannot reconstruct historical Turn relations after process restart, record `NOT RECOVERABLE BY CURRENT CONTRACT`; do not infer or backfill them from ORIGIN.

- [ ] **Step 3: Verify no order dependence**

Assert no result depends on HashMap iteration, fixture record order, or relation append order.

### Task 5: Publish the Phase 3.2 matrix and validate evidence

**Files:**
- Create: `docs/phase-3-2-project-workspace-matrix.md`
- Modify: `docs/evidence/README.md`

**Interfaces:**
- Consumes: executable fixture results and sanitized real probe results.
- Produces: Gate A–L table with creator, Workspace state, Desktop Project state, basis, result, and explicit unknown/not-observed fields.

- [ ] **Step 1: Write the matrix**

Record every Gate as PASS, FAIL, UNKNOWN, NOT OBSERVED, NOT TESTED, or NOT RECOVERABLE BY CURRENT CONTRACT. Separate fixture proof from real-environment observation.

- [ ] **Step 2: Scan evidence for sensitive content**

Reject prompt bodies, response bodies, reasoning, credentials, tokens/cookies, account data, raw database rows, private file contents, and unrelated existing Thread identifiers/titles.

- [ ] **Step 3: Validate JSON and canonical state vocabulary**

Run JSON parsing, exact Gate ID coverage, result vocabulary checks, and `git diff --check`.

### Task 6: Run final regressions and close Phase 3.2 only if evidence permits

**Files:**
- Modify only after all Gates are evaluated: `CodexMonitor_四阶段开发路线图.md`
- Update only after all Gates are evaluated: CodexMonitor files in the cross-project memory repository.

**Interfaces:**
- Consumes: complete Gate A–L results, restart classification, and full regression outputs.
- Produces: either `Phase 3.2 = PASS / COMPLETE; Phase 3.3 = GO / NOT STARTED` or an exact bounded blocker without a false PASS.

- [ ] **Step 1: Run focused and frozen regressions**

Run Phase 3.2.5 focused tests, `workspace_interop_core`, Desktop projection, app-server/runtime, Phase 3.1 admission/resume, and evidence validation.

- [ ] **Step 2: Run full verification**

Run `cargo test --lib`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`.

- [ ] **Step 3: Audit scope and private-state immutability**

Review status/diff, pre/post Desktop hashes, placeholder scans, and protected user modifications. Confirm no Phase 3.3 implementation or Desktop private-state write occurred.

- [ ] **Step 4: Commit by bounded concern**

Commit fixture/evidence implementation separately from project docs and cross-project memory. Do not stage the main checkout's `src-tauri/src/main.rs`, `启动CodexMonitor.bat`, `停止CodexMonitor.bat`, or unrelated memory changes.


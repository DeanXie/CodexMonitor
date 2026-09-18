# Worktree Build Storage Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bound ephemeral Worktree Cargo storage with isolated external targets, automatic preparation, closeout and orphan cleanup eligibility, and pre-build disk guards.

**Architecture:** A dependency-free Node core resolves Git topology, target identity, policy, classification, scanning, guards, and deletion safety. A thin CLI exposes prepare/report/guard/build/closeout/clean commands and injects Cargo environment only for linked Worktrees; main and release behavior remain unchanged.

**Tech Stack:** Node.js ESM and `node:test`, Git CLI, Cargo/Tauri CLI, JSON policy.

**Spec:** `docs/superpowers/specs/2026-09-19-worktree-build-storage-governance-design.md`

## Global Constraints

- Never delete current real Worktrees or current real build artifacts during implementation or acceptance.
- External targets are unique per Worktree; no shared agents target.
- Ephemeral builds use `CARGO_INCREMENTAL=0`; main keeps current behavior.
- Orphans are immediately visible but require at least seven days before apply eligibility.
- Cleanup is dry-run by default and must reject roots, `.git`, source roots, symlinks, junctions, and reparse escapes.
- Disk guard runs before every formal heavy ephemeral build.
- Do not change `VERSION.json`, release authority, release semantics, `src-tauri/src/main.rs`, root `.gitignore`, or current user batch files.

---

### Task 1: Policy, topology, and target identity

**Files:**
- Create: `config/storage-governance.json`
- Create: `scripts/storage-governance-core.mjs`
- Test: `scripts/storage-governance.test.mjs`

**Interfaces:**
- Produces `loadPolicy`, `resolveTopology`, `resolveBuildRoot`, `resolveAgentTarget`, and `evaluateBudget`.

- [ ] Write failing tests proving main/linked classification, two Worktrees receiving different targets, configured root precedence, and all warning/block thresholds.
- [ ] Run `node --test scripts/storage-governance.test.mjs` and confirm missing exports fail.
- [ ] Implement the minimal pure functions and policy loader.
- [ ] Re-run the focused suite and keep it green.

### Task 2: Safe manifests, classification, and single-pass reports

**Files:**
- Modify: `scripts/storage-governance-core.mjs`
- Modify: `scripts/storage-governance.test.mjs`

**Interfaces:**
- Produces `ensureAgentTarget`, `scanDirectoryOnce`, `classifyTargets`, and `buildStorageReport`.

- [ ] Add failing fixture tests for `ACTIVE`, `CLOSEOUT-ELIGIBLE`, `ORPHAN`, `LEGACY-IN-TREE`, `MAIN`, and `RELEASE`.
- [ ] Add a failing assertion that each directory is traversed once and scan duration is returned.
- [ ] Implement schema-versioned manifests and one-pass aggregation.
- [ ] Re-run the focused suite.

### Task 3: Fail-closed closeout and orphan cleanup

**Files:**
- Modify: `scripts/storage-governance-core.mjs`
- Modify: `scripts/storage-governance.test.mjs`

**Interfaces:**
- Produces `planCloseout`, `planAgentCleanup`, and `applyCleanupPlan`.

- [ ] Add failing tests for dry-run immutability, accepted/clean requirements, seven-day orphan TTL, exact candidate deletion, and rejection of root, `.git`, Worktree root, dirty source, and symlink escape.
- [ ] Implement canonical boundary validation and exact-target deletion.
- [ ] Add Windows-only ACL fixture behavior that either repairs only the candidate or returns a scoped fail-closed result.
- [ ] Re-run the focused suite.

### Task 4: CLI and build wrappers

**Files:**
- Create: `scripts/storage-governance.mjs`
- Modify: `scripts/storage-governance.test.mjs`
- Modify: `package.json`

**Interfaces:**
- Commands: `prepare`, `report`, `guard`, `cargo`, `tauri`, `closeout`, and `clean-agents`.

- [ ] Add failing CLI integration tests for prepare, report, guard rejection/override, Cargo environment injection, dry-run closeout, and apply closeout.
- [ ] Implement the CLI as a thin adapter over the core.
- [ ] Add `storage:*`, `rust:check`, `rust:test`, and Tauri wrapper scripts with minimal package.json edits.
- [ ] Re-run CLI tests and `npm run typecheck`.

### Task 5: Agent lifecycle and target-path consumers

**Files:**
- Modify: `AGENTS.md`
- Modify: `scripts/phase-3-5-final-acceptance.mjs`
- Modify: `scripts/phase-3-5-final-acceptance.test.mjs`

**Interfaces:**
- Agents use formal wrapper commands and `storage:closeout`; acceptance code consumes the resolved Cargo target path.

- [ ] Add a failing acceptance test using a non-default target root.
- [ ] Update acceptance target resolution without changing release paths.
- [ ] Document the no-bare-Cargo and closeout requirements in `AGENTS.md`.
- [ ] Run focused acceptance tests.

### Task 6: Operations documentation

**Files:**
- Create: `docs/build-storage-governance.md`
- Modify: `docs/codebase-map.md`
- Modify: `README.md`

**Interfaces:**
- Documents target locations, classifications, commands, policy edits, rollback, and ACL behavior.

- [ ] Write the concise operational guide and link it from existing navigation.
- [ ] Scan for stale Windows-hardcoded paths in new files.

### Task 7: Real Worktree acceptance and final verification

**Files:**
- No production files beyond prior tasks; temporary fixtures only.

**Interfaces:**
- Produces fresh verification evidence and the final storage baseline.

- [ ] Create two temporary Worktrees and confirm different resolved external targets.
- [ ] Run real ephemeral `cargo check` and `cargo test` through wrappers and confirm no Worktree-local target growth.
- [ ] Run two builds concurrently and confirm no shared target or lock contention.
- [ ] Run closeout apply on a fixture and prove target deletion while Worktree and branch remain.
- [ ] Remove one fixture from the Git registry, prove immediate `ORPHAN`, and prove under-TTL apply refusal.
- [ ] Inject low-space stats and prove guard failure.
- [ ] Confirm main Cargo metadata and incremental behavior remain unchanged.
- [ ] Run focused tests, typecheck, `npm run version:check`, release authority tests, `git diff --check`, `npm run storage:report`, and final status/diff review.

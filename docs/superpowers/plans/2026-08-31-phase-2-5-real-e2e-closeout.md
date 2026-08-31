# Phase 2.5 Real E2E Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve sanitized Gate B evidence, record the A/B/C/D Real E2E PASS conclusion, and move Phase 2.5 to final UI / closeout without starting Phase 3.

**Architecture:** Keep the already reviewed A/C/D source fixes in their existing standalone commit and fast-forward that commit onto `main`. Create a second closeout commit containing only durable evidence, canonical project documentation, and cross-project memory updates; do not modify Global Source Core behavior.

**Tech Stack:** Git, JSON evidence summaries, Markdown project documentation, Node.js evidence validation.

**Spec:** User-approved Phase 2.5 Real E2E closeout request dated 2026-08-31.

## Global Constraints

- `Phase 2.5 Desktop forensics = COMPLETED`.
- `Slice 1 = PASS`.
- `Slice 2 = PASS`.
- `Real E2E A/B/C/D = PASS`.
- `Phase 2.5 = READY FOR FINAL UI / CLOSEOUT`.
- Do not modify the passed Global Source Core in the closeout commit.
- Do not start Phase 3.
- Preserve `src-tauri/src/main.rs` and the two untracked local `.bat` files.

---

### Task 1: Establish the commit boundary

**Files:**
- Existing standalone commit: `d617b7f fix: close phase 2.5 real e2e gaps`
- Preserve: `src-tauri/src/main.rs`
- Preserve: `启动CodexMonitor.bat`
- Preserve: `停止CodexMonitor.bat`

**Interfaces:**
- Consumes: reviewed A/C/D fixes and evidence already committed at `d617b7f`.
- Produces: `main` containing the standalone source commit before closeout docs are staged.

- [ ] **Step 1: Verify ancestry and exact source commit scope**

Run: `git merge-base --is-ancestor HEAD d617b7f` and `git diff --name-status HEAD..d617b7f`.

Expected: `d617b7f` is a direct descendant and contains only the previously reviewed four fixes, tests, and Gate A/C/D evidence.

- [ ] **Step 2: Clear content-identical index noise and fast-forward**

Run: `git add --refresh -- src-tauri/src/main.rs` and `git merge --ff-only d617b7f`.

Expected: no content change to `src-tauri/src/main.rs`; `main` advances to `d617b7f` without creating a mixed commit.

### Task 2: Save Gate B and A/B/C/D evidence

**Files:**
- Create: `docs/evidence/phase-2-5-gate-b-main.json`
- Create: `docs/evidence/phase-2-5-gate-b-child.json`
- Create: `docs/evidence/phase-2-5-real-e2e-summary.md`
- Modify: `docs/evidence/README.md`

**Interfaces:**
- Consumes: sanitized diagnostic journal facts and the accepted Gate B task result.
- Produces: durable identity, source, hierarchy, model, token, lifecycle, workspace, startup, discovery, catch-up, tail, and completion evidence without prompt or reasoning content.

- [ ] **Step 1: Write Main and Child evidence summaries**

Use the existing evidence schema. Put process/service start, first canonical observation, catch-up range, continued-tail range, and completion latency in the allow-listed `notes` field while keeping structured identity/model/token/lifecycle/workspace fields.

- [ ] **Step 2: Write the four-gate summary**

Record all four accepted outcomes and the exact conclusion `Desktop Near-Live Real E2E = PASS`.

- [ ] **Step 3: Validate evidence**

Run the existing Node validator against all Phase 2.5 evidence JSON files and scan the new files for prompt/reasoning/message content.

Expected: every JSON validates; the sensitive-content scan finds no retained content.

### Task 3: Update canonical project state

**Files:**
- Modify: `CodexMonitor_四阶段开发路线图.md`
- Modify: `docs/desktop-near-live-forensics.md`

**Interfaces:**
- Consumes: Gate A/B/C/D PASS evidence.
- Produces: canonical Phase 2.5 completion state and a bounded final-UI work list.

- [ ] **Step 1: Replace obsolete Real E2E GO / not-started status**

Record Desktop forensics COMPLETED, Slice 1 PASS, Slice 2 PASS, Real E2E A/B/C/D PASS, and READY FOR FINAL UI / CLOSEOUT.

- [ ] **Step 2: Record remaining UI scope**

Limit the next work to Desktop producer-surface filter/label presentation, canonical hierarchy/status/model/token/freshness rendering, projection-only exclusion checks, and focused UI tests. Explicitly keep Phase 3 blocked.

### Task 4: Update cross-project memory

**Files:**
- Modify: `F:/ObsidianVault/Codex跨项目记忆库/02_项目/CodexMonitor Agent Monitor/当前状态.md`
- Modify: `F:/ObsidianVault/Codex跨项目记忆库/02_项目/CodexMonitor Agent Monitor/已完成事项.md`
- Modify: `F:/ObsidianVault/Codex跨项目记忆库/02_项目/CodexMonitor Agent Monitor/重要决策.md`

**Interfaces:**
- Consumes: the canonical project closeout facts.
- Produces: live cross-project status, completion history, and the decision to freeze backend Phase 2.5 behavior while final UI remains.

- [ ] **Step 1: Update current state and completed work**

Mirror the exact five-line Phase 2.5 status and link the durable evidence paths.

- [ ] **Step 2: Update decisions**

Record that A/B/C/D PASS closes the backend Real E2E gate, no second watcher or private Desktop writes are allowed, and the only remaining Phase 2.5 work is final UI / closeout.

### Task 5: Verify and commit the closeout

**Files:**
- Stage only the evidence, plan, project docs, and three cross-project memory files listed above.

**Interfaces:**
- Consumes: all closeout edits.
- Produces: one documentation/memory commit separate from `d617b7f`.

- [ ] **Step 1: Run documentation-appropriate verification**

Run: evidence schema validation, sensitive-content scan, exact status phrase scan, `git diff --check`, `npm run typecheck`, and the focused evidence test.

Expected: all closeout-specific checks pass. Any unrelated baseline test issue is reported separately and is not called PASS.

- [ ] **Step 2: Review scope and placeholders**

Run: `git status --short`, `git diff --name-status`, and scans for `TODO`, `TBD`, `FIXME`, obsolete `REAL E2E GO`, and accidental Phase 3 activation.

Expected: no source file is part of the closeout diff; local `.bat` files remain untracked and untouched.

- [ ] **Step 3: Create the documentation/memory commit**

Stage exact paths and commit with: `docs: close phase 2.5 real e2e`.

- [ ] **Step 4: Verify final repository state**

Run: `git log -2 --oneline`, `git status --short --branch`, and `git show --name-status --oneline HEAD`.

Expected: `d617b7f` remains the standalone source commit, the closeout commit contains no source, and only the preserved local `.bat` files or content-identical stat noise remain outside commits.

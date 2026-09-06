# Phase 3.3 Final Standard Session Acceptance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prepare and execute a fail-closed, privacy-safe real acceptance run for one Monitor-created Standard Session across Monitor, Desktop, and CLI.

**Architecture:** A Node standard-library collector snapshots rollout inventory, locks one authoritative new `session_meta.id`, captures exact-ID Turn deltas at explicit user-confirmed boundaries, and records app-server read/resume outcomes. Raw state stays in an ignored run directory; only sanitized evidence and canonical acceptance documentation enter Git.

**Tech Stack:** Node.js ESM and `node:test`, Codex app-server JSON-RPC, existing Codex rollout JSONL, Cargo/npm regression gates.

**Spec:** `docs/superpowers/specs/2026-09-06-phase-3-3-final-standard-session-acceptance-design.md`

## Global Constraints

- Use a new Final Acceptance Thread only.
- Never record prompt, assistant output, reasoning, credentials, cookies, tokens, or private file contents.
- Never write Desktop private databases or metadata.
- Correlate only by authoritative fullThreadId/fullTurnId and explicit stage checkpoints.
- Normal cross-Surface continuation occurs only after the previous writer releases.
- Keep the active-writer protection Gate separate from idle continuation.
- Do not start Phase 3.4.

---

### Task 1: Collector contract and parser

**Files:**
- Create: `scripts/phase-3-3-final-acceptance.test.mjs`
- Create: `scripts/phase-3-3-final-acceptance.mjs`

**Interfaces:**
- Produces: `inventoryRollouts()`, `parseSanitizedRollout()`, `discoverSingleNewThread()`, `captureSingleTurnDelta()`, and the `prepare`, `capture`, `probe`, `status`, `finalize` CLI commands.

- [x] Write Node tests proving sensitive fields are omitted, invalid/multiple new Threads fail closed, exact IDs are retained, a stage requires one new Turn, repeated capture is idempotent, and incomplete finalization fails.
- [x] Run `node --test scripts/phase-3-3-final-acceptance.test.mjs` and confirm RED because the collector module does not exist.
- [x] Implement the smallest standard-library collector satisfying those tests; do not add a production IPC or runtime surface.
- [x] Rerun the focused Node tests and confirm GREEN.

### Task 2: Prepare isolated runtime evidence state

**Files:**
- Runtime only: `.codexmonitor/phase-3-3-final-acceptance/<runId>/run.json`

**Interfaces:**
- Consumes: collector `prepare` command.
- Produces: a baseline rollout inventory, exact workspace root, CODEX_HOME location, CLI version, and `PREPARED` stage without touching an existing Thread.

- [x] Verify the worktree is clean except owned spec/plan/collector files.
- [x] Run the focused collector tests and `npm run typecheck`.
- [x] Run `prepare` against the active CODEX_HOME and this isolated worktree.
- [x] Run `status` and confirm no Thread is locked and the next action is Monitor creation.

### Task 3: Guided real Surface sequence

**Files:**
- Runtime only: ignored run manifest and stage snapshots.

**Interfaces:**
- Consumes: one explicit user action at a time.
- Produces: exact Monitor, Desktop, CLI, occupancy, and restart observations.

- [x] Ask the user to create and finish one new Monitor Turn; capture and lock the sole authoritative new Thread/Turn.
- [x] Confirm idle state, then ask for one Desktop exact-ID continuation; capture exactly one new Turn.
- [x] Confirm idle state, then ask for one CLI exact-ID continuation; capture exactly one new Turn.
- [x] Run the separate active-writer Gate and record `BLOCKED_BY_ACTIVE_WRITER` without classifying it as failure.
- [x] After release, perform bounded exact-ID read/resume and restart/list reconstruction checks.

### Task 4: Finalize evidence and determine Phase status

**Files:**
- Create: `docs/evidence/phase-3-3-final/standard-session.json`
- Create or modify: canonical Phase 3.3 acceptance documentation.

**Interfaces:**
- Consumes: completed ignored run state.
- Produces: sanitized evidence, deterministic validation, and either `Phase 3.3 = PASS / COMPLETE` or an exact blocker.

- [x] Run `finalize`; reject any forbidden key or missing required gate.
- [x] Add fixture-backed validation for the sanitized evidence.
- [x] Run Phase 3.3 focused regressions, Phase 3.2 workspace regressions, Phase 3.1 exact-ID regressions, `cargo test --lib`, `cargo check --all-targets`, `cargo fmt --check`, `npm run typecheck`, and `git diff --check`.
- [x] Verify Desktop private state was never written by the harness and no existing Thread was operated on.
- [x] Report observed PASS/FAIL/UNKNOWN boundaries and stop before Phase 3.4.

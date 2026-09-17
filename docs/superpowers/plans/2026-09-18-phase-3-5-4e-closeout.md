# Phase 3.5.4e Telemetry / Compatibility / Docs Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Freeze the completed Phase 3.5.4 projection, event-delivery, recovery, offline/stale UI, telemetry, and multi-client contracts as stable compatibility fixtures/tests/docs, then close the whole phase without adding product behavior.

**Architecture:** Reuse the existing Phase 3.5.4a-d Rust and TypeScript authorities. Add one aggregate sanitized fixture manifest and authority contract, exercise those contracts through real Rust serialization and frontend projection/recovery functions, and update the canonical documentation and evidence index. Production code changes are permitted only if the new compatibility suite exposes a small existing contract defect.

**Tech Stack:** Rust, serde/serde_json, TypeScript, Vitest, sanitized JSON fixtures, Markdown.

**Spec:** User-approved Phase 3.5.4e request dated 2026-09-18 and `docs/phase-3-5-4-projection-recovery-telemetry.md`.

## Global Constraints

- No new subsystem, authority, recovery redesign, event ledger, persistence layer, UI redesign, or Phase 4 work.
- No real resume, approval decision, Thread delete, upstream unsubscribe, writer takeover, or other mutation.
- Preserve current generation, freshness, coverage, recovery, approval/delete UI, and multi-client semantics from Phase 3.5.4a-d.
- Preserve zero automatic retry/replay and absence of RemoteClientIdentity, owner, lease, `FREE`, `AVAILABLE`, `RELEASED`, force takeover, telemetry authority, and new generation concepts.
- Preserve the four dirty main-worktree files and explicitly stage only Phase 3.5.4e paths.
- The six existing zh-CN frontend baseline failures retain their existing waiver; no new failure is allowed.

---

### Task 1: Aggregate fixture contract and genuine RED

**Files:**
- Create: `docs/fixtures/phase-3-5-4-compatibility/authority-contract.json`
- Create: `docs/fixtures/phase-3-5-4-compatibility/fixture-family-manifest.json`
- Create: `docs/fixtures/phase-3-5-4-compatibility/current-actionable-approval.json`
- Create: `src-tauri/src/shared/phase_3_5_4_compatibility_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`
- Create: `src/features/app/orchestration/phase354Compatibility.test.ts`

**Interfaces:**
- Consumes: existing `ProjectionFreshness*`, `AppServerEvent`, projection status model, authoritative recovery coordinator, approval/delete projection gates, and Phase 3.5.4a-d sanitized fixture families.
- Produces: one aggregate compatibility schema plus Rust/frontend tests named for the frozen contracts.

- [x] Register the Rust and TypeScript compatibility suites while the aggregate fixtures are absent.
- [x] Run the focused suites and retain the expected missing-fixture RED.
- [x] Add the minimal sanitized authority contract, fixture-family manifest, and current-actionable approval fixture.
- [x] Run the same suites GREEN without changing product behavior.

### Task 2: Cross-slice contract and forbidden-inference freeze

**Files:**
- Modify: `src-tauri/src/shared/phase_3_5_4_compatibility_tests.rs`
- Modify: `src/features/app/orchestration/phase354Compatibility.test.ts`
- Modify aggregate fixtures only as required by real authority schemas.

**Interfaces:**
- Consumes: exact statuses, coverages, sources, generation envelope, recovery chain, UI projection model, gap evidence, approval/delete gates, and zero-mutation intent.
- Produces: compatibility evidence for all user-named tests and fixture families.

- [x] Freeze projection schema, coverage/source vocabularies, local/Remote event parity, fail-closed stale/missing-generation delivery, and generation hierarchy.
- [x] Freeze authoritative recovery order, coverage-by-coverage hydration, observation parity, event-gap diagnostics, and mixed coverage UI behavior.
- [x] Freeze availability/freshness separation, multi-client divergence/convergence, stale approval safety, delete unknown/confirmed safety, and zero mutation retry/replay.
- [x] Scan serialized schema/state keys and contract statements for forbidden inference and ownership semantics without treating deliberate documentation warnings as product fields.

### Task 3: Authoritative documentation and evidence closeout

**Files:**
- Modify: `docs/app-server-events.md`
- Modify: `docs/codebase-map.md`
- Modify: `docs/phase-3-5-4-projection-recovery-telemetry.md`
- Modify: `docs/fixtures/projection-freshness/README.md`
- Create: `docs/fixtures/phase-3-5-4-compatibility/README.md`
- Modify: `docs/evidence/README.md`
- Create: `docs/evidence/phase-3-5-4e/compatibility-closeout.md`
- Modify: `docs/phase-3-5-0-remote-mobile-forensics.md`

**Interfaces:**
- Consumes: passing aggregate compatibility evidence.
- Produces: canonical Phase 3.5.4 authority, telemetry classification/persistence policy, authority precedence, remaining NOT PROVEN boundary, and final phase status.

- [x] Record `ProjectionFreshness`, generation-tagged delivery, recovery, UI, gap, multi-client, approval/delete, and no-replay contracts without inventing authority.
- [x] Classify authoritative state, diagnostic telemetry, historical evidence, and UI projection; freeze no telemetry database/freshness persistence/gap ledger.
- [x] Preserve daemon broadcast/event stream completeness as NOT PROVEN and document known-gap recovery as the safe mitigation.
- [x] Mark Phase 3.5.4a-e and the whole Phase 3.5.4 PASS / COMPLETE / FROZEN only after fresh verification succeeds.

### Task 4: Fresh verification and scoped Git closeout

**Files:**
- Stage only files owned by Tasks 1-3 and this plan.

**Interfaces:**
- Consumes: the final scoped worktree.
- Produces: fresh verification evidence, one scoped commit, ff-only main integration, and verified origin alignment.

- [x] Run Phase 3.5.4a-d regressions, the new compatibility suites, Phase 3.5.3 approval/delete non-regression, transport/provenance, writer/subscription/runtime, and daemon-restart regressions.
- [x] Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, focused/full frontend tests, and `git diff --check`.
- [x] Review the scoped diff, forbidden scans, fixture sanitization, placeholders, and the untouched dirty main-worktree files.
- [x] Prepare the explicit Phase 3.5.4e stage list and ff-only closeout sequence; stop before Phase 4.

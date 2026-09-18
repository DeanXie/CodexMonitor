# Phase 3.5 Final Integration Acceptance Plan

> **Status:** completed and frozen. This plan closed Phase 3.5 only and did not authorize Phase 4.

**Goal:** Run a fresh, evidence-backed final acceptance across Phase 3.5.1-3.5.4, including one isolated non-destructive real Remote E2E, and freeze the result in canonical fixtures, tests, evidence, and roadmap documentation.

**Architecture:** Reuse the existing Phase 3.5 shared authorities and fixture families. Add an aggregate acceptance contract plus an isolated host-side E2E harness. The real lane uses a disposable workspace, disposable `CODEX_HOME`, disposable daemon data directory, random loopback port, random token, and an existing exact disposable Thread without creating a Turn. It performs only authentication, workspace connect, exact list/read, freshness queries, reconnect, and authoritative rehydration. Stale-generation rejection, daemon restart, multi-client isolation, mutation no-replay, approval, delete, writer/subscription/runtime, event-gap, and UI honesty remain deterministic integrations over the production authorities. No real approval decision, delete, upstream unsubscribe, resume, takeover, or user Thread is permitted.

**Spec:** User-approved Phase 3.5 Final Integration Acceptance / E2E / Closeout Gate dated 2026-09-18.

## Safety boundaries

- Work only in `codex/phase-3-5-final-acceptance` at the dedicated acceptance worktree.
- Preserve `.gitignore`, `src-tauri/src/main.rs`, `启动CodexMonitor.bat`, and `停止CodexMonitor.bat` in the main checkout.
- Do not use stash, reset, restore, clean, `git add -A`, or `git add .`.
- Do not touch A3 evidence, user Codex homes, user Threads, or production project data.
- Never persist or print the daemon token or private Thread contents.
- If acceptance exposes more than a small existing contract bug, stop without expanding scope.

## Task 1: Aggregate Phase 3.5 acceptance contract — RED then GREEN

**Files:**
- Add: `docs/fixtures/phase-3-5-final-acceptance/authority-contract.json`
- Add: `docs/fixtures/phase-3-5-final-acceptance/fixture-family-manifest.json`
- Add: `src-tauri/src/shared/phase_3_5_final_acceptance_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`
- Add: `src/features/app/orchestration/phase35FinalAcceptance.test.ts`

1. Add RED tests requiring the aggregate fixture families, authority hierarchy, no-replay counts, forbidden-inference boundaries, forbidden-semantics exclusions, generation-scoped writer/subscription/runtime state, exact approval/delete constraints, projection hydration, event-gap isolation, and UI honesty.
2. Run the focused Rust and frontend tests and preserve the initial missing-fixture failures as RED evidence.
3. Add the minimum sanitized manifests/contracts and turn the focused suites GREEN.

## Task 2: Isolated non-destructive real Remote E2E — RED then GREEN

**Files:**
- Add: `scripts/phase-3-5-final-acceptance.mjs`
- Add: `scripts/phase-3-5-final-acceptance.test.mjs`
- Modify: `package.json`

1. Add unit/contract tests for fail-closed isolation, secret redaction, exact-ID enforcement, allowed RPC list, forbidden mutation count zero, generation/reconnect assertions, and evidence sanitization; run them RED before the harness exists.
2. Implement the bounded harness:
   - create unique ignored run root under `src-tauri/target/phase-3-5-final-acceptance/`;
   - create disposable workspace, `CODEX_HOME`, daemon data-dir, random loopback port, and random token;
   - reuse an existing disposable exact Thread without calling `thread/start` or `turn/start`;
   - launch the locally built daemon with the isolated data-dir and inherited isolated `CODEX_HOME`;
   - authenticate, validate identity/protocol, list/connect workspace, exact-list/exact-read, query freshness, observe generation-tagged delivery, reconnect to a new transport generation, verify stale old-generation rejection through the production gate regression, then re-list/read/query until current;
   - stop only harness-owned processes and retain sanitized evidence without token or Thread contents.
3. Run the harness tests GREEN, build the daemon, run exactly one real E2E, and preserve the sanitized result.

## Task 3: Final deterministic integration matrix

**Files:** existing Phase 3.5 test modules and fixture suites only unless a small contract defect is proven.

1. Run focused Phase 3.5.1 foundation/identity regressions.
2. Run Phase 3.5.2 admission, lifecycle, transport, reconnect, restart, multi-client, and no-replay regressions.
3. Run Phase 3.5.3 approval/delete exact-ID, stale/unknown, dispatch-correlation, and non-destructive fake-authority regressions.
4. Run Phase 3.5.4 freshness, hydration, generation-event, event-gap, UI honesty, multi-client, and compatibility regressions.
5. Confirm the two frozen upstream NOT PROVEN items remain documented and non-blocking.

## Task 4: Evidence and roadmap closeout

**Files:**
- Add: `docs/evidence/phase-3-5-final/final-acceptance.json`
- Add: `docs/evidence/phase-3-5-final/README.md`
- Add: `docs/phase-3-5-final-integration-acceptance.md`
- Modify: `README.md`
- Modify: `docs/codebase-map.md`
- Modify: existing current-state/roadmap/evidence index files discovered in the repository.

1. Record the tested commit, environment, bundled Codex version, real versus deterministic coverage, sanitized host identity, observed generations, E2E steps, assertions, test counts, waiver, and remaining NOT PROVEN items.
2. Record that real mutation counts are zero; no Thread/Turn creation, approval decision, delete, unsubscribe, resume, or takeover occurs.
3. Mark Phase 3.5.0 complete and Phase 3.5.1-3.5.4 plus Phase 3.5 overall PASS / COMPLETE / FROZEN only after all fresh gates pass.
4. Keep Phase 4 not started.

## Task 5: Fresh verification and scoped Git closeout

1. Run the final integration suite and the real non-destructive E2E.
2. Run fresh focused Phase 3.5.1-3.5.4 regressions.
3. Run `cargo test --all-targets --no-fail-fast`, `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, the frontend full suite, and `git diff --check`.
4. Treat only the already frozen six zh-CN/date-label failures as waived; add no waiver.
5. Review the scoped diff and explicitly stage only acceptance tests, fixtures, evidence, docs, and any proven minimal bugfix.
6. Commit, fast-forward merge to `main`, push `origin/main`, verify local/remote/GitHub refs, and stop before Phase 4. If dirty-main safety blocks the merge, stop and request explicit user direction.

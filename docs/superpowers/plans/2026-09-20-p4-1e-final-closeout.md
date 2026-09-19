# P4.1e Final Closeout Resume Plan

**Goal:** Reconcile current P4.1 truth, freeze the complete failure/evidence
matrix, run fresh verification on current main, and close P4.1 without changing
production behavior.

**Base:** `9579a3fb459313818da7318b40f7dcb2f0f98161` / `0.7.68 · Build 8 · development`.

## Constraints

- Preserve the old `phase-4-1e-closeout` worktree unchanged as historical material.
- Production source changes are forbidden; a newly discovered production bug stops closeout.
- Real user cutover, installed-package execution, credentials, HostIdentity, and real services are out of scope.
- Build remains 8 because this slice changes only tests, fixtures, evidence, and docs.
- P4.2 remains not started.

## Task 1 — Truth and aggregate RED

- Add final closeout assertions for Build 8, R01–R12, status consistency, and
  the three missing d-4c failure cases.
- Run the focused Node suite and observe RED against stale authority.

## Task 2 — Minimal contract/docs GREEN

- Extend the failure matrix with evidence type and d-4c cases.
- Reconcile README, P4.0 contract, P4.1 closeout, evidence index/evidence,
  and the root roadmap while preserving historical Build evidence.
- Keep final completion pending until fresh verification succeeds.

## Task 3 — Fresh verification and final freeze

- Run all named P4.1 focused suites, Rust all-targets/check/fmt, frontend
  focused/full, typecheck, production Windows build, version and diff checks.
- Accept only the six frozen zh-CN/date frontend failures.
- After all gates pass, update the final statuses, rerun affected contract
  checks on the final tree, explicitly stage, commit, ff-only merge, and push.

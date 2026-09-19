# P4.1d-4a Recovery / Root Authority Evidence

Status: **PASS / COMPLETE / FROZEN**.

The original compliance RED is retained as historical evidence: startup
inspection made a valid target-moved/journal-lagged state unreachable from the
recovery adapter, `ValidateCommittedTarget` did not converge the journal, and
explicit relative data roots were resolved from the process current directory.

Fresh GREEN authority is provided by:

- `shared::startup_activation_tests` for read-only classification and strict
  absolute-root validation;
- `bootstrap::tests` for the real recovery adapter, fresh and migrated target
  convergence, stop-evidence refusal, tamper recheck, identity continuity, and
  business-gate closure;
- `shared::activation_foundation_tests` for every journal interruption boundary,
  Windows identity retirement, recovery-material preservation, and concurrent
  commit locking;
- `tests/runtime_validation_handshake.rs` for daemon/daemonctl argument parsing,
  current-directory independence, unavailable default-root failure, and d-3
  runtime readiness.

The machine-readable contract is
`../../fixtures/phase-4-1d-4a-recovery-root-authority/contract.json`.
Git closeout records the final fresh command counts and commit/ref identity.
Real user data operations and real service operations remain zero.

Final fresh verification at `v0.7.68 · Build 6 · development` recorded:

- Rust: `1141/1145` passed with 4 ignored, daemon `1038/1041` passed with
  3 ignored, daemonctl `29/29`, runtime handshake `10/10`, and Tauri config
  `3/3`; zero failures;
- focused frontend/bootstrap/release safety: `47/47`;
- version authority: `29/29`;
- full frontend: `1235/1241`, with only the six previously approved
  zh-CN/date baseline failures and no new waiver;
- all-target check, format check, typecheck, version check, diff check, and the
  production MSI/NSIS build passed.

One intermediate all-target rerun observed the existing Windows temporary-file
lock test fail once with `Access denied`. The exact test then passed three
isolated repetitions, and the final complete Rust rerun passed with zero
failures; no waiver or production change was introduced for that transient.

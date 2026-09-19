# P4.1d-4c Evidence

Status: **PASS / COMPLETE / FROZEN** for R04, R06, and R11.

Authority:

- `docs/phase-4-1d-4c-entry-isolation-closeout.md`
- `docs/fixtures/phase-4-1d-4c-entry-isolation-closeout/contract.json`
- native menu/tray/startup gate tests in the Rust library
- bootstrap restart-boundary tests in Rust and frontend
- `runtime_validation_handshake` tests using the confined child-process harness

RED evidence:

- native menu actions had no shared bootstrap gate;
- production-entry process tests had no reusable full environment isolation;
- the restart requirement lacked a fresh-activation/next-process identity
  regression.

GREEN evidence proves zero native business dispatch before current-process
`READY`, a restart-only activation UX across fresh/migration/recovery flows, and
confined daemon/daemonctl execution roots.

Fresh verification on 2026-09-20:

- Rust all-targets: 2,254 passed, 0 failed, 9 ignored;
- runtime-validation process isolation: 11 passed;
- native startup/menu/tray gates: 14 passed;
- frontend bootstrap boundary: 4 passed;
- P4.1 compatibility contract: 10 passed;
- frontend full suite: 1,237 passed out of 1,243, with only the frozen six
  zh-CN/date expectation waivers failing (`useTraySessionUsage`: five;
  `Home`: one); no new waiver was added;
- Rust all-targets check, Rust format check, TypeScript typecheck, version
  projection check, and Git diff check: passed;
- Windows production build: passed; both MSI and NSIS bundles were generated
  in the worktree's external Cargo target. They were not installed or run.

Real user data operations: **0**. Real service operations: **0**. Installed app
or WebView launches: **0**.

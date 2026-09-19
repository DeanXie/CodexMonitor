# P4.1d-4b Evidence

Status: **PASS / COMPLETE / FROZEN** for the controlled legacy-migration entry
and supported Windows native stop-evidence boundary.

Authority:

- `docs/phase-4-1d-4b-controlled-legacy-migration-entry.md`
- `docs/fixtures/phase-4-1d-4b-legacy-migration-entry/contract.json`
- shared tests in `legacy_migration_entry_tests.rs`,
  `legacy_process_stop_tests.rs`, and `activation_foundation_tests.rs`
- production adapter tests in `bootstrap.rs`
- frontend IPC/bootstrap tests in `tauri.test.ts` and
  `BootstrapBoundary.test.tsx`

RED evidence established that the product lacked a migration preview/confirm
entry, caller-supplied stopped evidence could reach activation, and no guarded
retirement path was wired. The frontend RED reported missing wrappers and UI.
GREEN evidence covers sanitized one-time preview, exact confirmation intent,
RUNNING/UNKNOWN fail-closed behavior, the cached-identity live-child case,
protected-handle validation, in-critical-section revalidation, successful
quiescent commit, prepared recovery, duplicate confirmation, and the
`target_committed` runtime boundary.

Fresh closeout verification:

- Rust: 1,155 + 1,049 + 29 + 10 + 3 passed, 0 failed; 5 + 4 ignored helpers;
- frontend focused: 78 passed, 0 failed;
- frontend full: 1,237 passed; only the six frozen zh-CN/date baseline cases
  failed, with no new waiver;
- `rust:check`, TypeScript typecheck, Rust format check, version projection
  check, production MSI/NSIS build, and `git diff --check`: passed;
- version authority: `0.7.68 · Build 7 · development`.

No real user path, identity, credential, App, daemon, or migration was read,
stopped, retired, or changed. P4.1e remains paused; R04, R06, and R11 remain
open.

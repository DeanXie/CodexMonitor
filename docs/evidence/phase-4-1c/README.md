# P4.1c Controlled Whitelist Migration Evidence

Status: **PASS / COMPLETE / FROZEN**.

## Evidence scope

- Production core: `src-tauri/src/shared/migration_core.rs`
- Focused contract tests: `src-tauri/src/shared/migration_core_tests.rs`
- Sanitized fixtures: `docs/fixtures/phase-4-1c-whitelist-migration/`
- Authority: `release-identity.json.configSchemaVersion`
- Software/build authority: `VERSION.json` (independent from config schema)

All migration tests use disposable temporary roots and fake data. Real user
data access count is zero. Real migration/activation count is zero.

The RED evidence was a real focused Rust compile failure because
`shared::migration_core` did not exist. The initial GREEN suite passed 38/38;
the final expanded focused suite passed 45/45, covering source immutability,
field allowlisting, secret
exclusion, explicit auth/CODEX_HOME/canonical-data exclusion, workspace
identity, target conflicts, schema authority, sanitized preview, staging,
validation, five interruption points, owned rollback, and inactive legacy
runtime identity.

## Fresh closeout verification

- P4.1c focused Rust: 45 passed, 0 failed.
- Rust all-targets: 1,026 passed, 0 failed, 3 ignored.
- `cargo check --all-targets`: PASS.
- `cargo fmt --all -- --check`: PASS.
- P4.1a focused frontend: 3 passed, 0 failed.
- P4.1b version authority: 29 passed, 0 failed.
- P4.0 authority: 3 passed, 0 failed.
- Frontend full suite: 1,231 passed. The only six failures are the frozen
  zh-CN/date-label baseline in `useTraySessionUsage.test.tsx` and
  `Home.test.tsx`; no new waiver was introduced.
- `npm run typecheck`, production `npm run build`, and `git diff --check`: PASS.
- Canonical version authority: `0.7.68`, Build `2`, `development`.

All outputs were produced from synthetic temporary roots. Secret sentinel
occurrences in staging, backup, report, state, and generated evidence: zero.
Real user-data reads, real migration runs, and activation runs: zero.

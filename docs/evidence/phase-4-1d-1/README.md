# P4.1d-1 Migration / Activation Safety Foundation Evidence

Status: **PASS / COMPLETE / FROZEN**. Parent P4.1d remains **IN PROGRESS**.

## Scope and RED

- Production foundation: `src-tauri/src/shared/activation_foundation.rs`,
  `remote_host_identity_activation.rs`, and the hardened P4.1c migration core.
- Compatibility fixture: frozen v1 loader semantics in
  `legacy_remote_host_identity_loader_fixture.rs`.
- Sanitized contract: `docs/fixtures/phase-4-1d-1-migration-activation-foundation/contract.json`.
- Startup wiring: absent by design.

The real RED was a Rust compile failure for missing
`activation_foundation`/`remote_host_identity_activation` modules. No already
existing P4.1c RED was recreated.

## Native Windows evidence

The focused suite uses real temporary files, a child `powershell.exe`, Windows
share modes, and `ReplaceFileW`:

- legacy read while protected: denied;
- protected replace: succeeds and the exact path never becomes absent;
- frozen v1 loader: rejects v2 active/retired before and after guard release;
- blocked replacement sharing: replace fails and original v1 remains readable;
- service lifetime lock: second in-process and child-process access denied,
  then succeeds after release.

This evidence is Windows-native process/file-system evidence, not a mock. It
does not prove abrupt power-loss behavior or exclusion of old binaries that do
not participate in the new lock.

## Fresh closeout verification

- Activation foundation: 17 passed, 0 failed.
- Migration core/P4.1c regression: 57 passed, 0 failed.
- Rust all-targets: 2,170 passed, 0 failed, 7 ignored across the four target
  suites (1,115 + 1,023 + 29 + 3 passed).
- P4.0 authority: 3 passed, 0 failed.
- P4.1a Settings/update safety: 43 passed, 0 failed.
- P4.1b version authority: 29 passed, 0 failed.
- Frontend full suite: 1,231 passed. The only six failures are the frozen
  zh-CN/date-label baseline: five named cases in
  `useTraySessionUsage.test.tsx` and one in `Home.test.tsx`. No new waiver was
  introduced.
- `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run
  typecheck`, production `npm run build`, `npm run version:check`, and `git
  diff --check`: PASS.
- Canonical version authority: `0.7.68`, Build `3`, `development`.

All migration and identity operations used disposable temporary roots and fake
UUIDs. Real user-data access count: 0. Real migration/retirement count: 0.

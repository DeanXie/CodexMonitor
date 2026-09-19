# P4.1d-2 Startup Cutover Evidence

Status: **PASS / COMPLETE / FROZEN** after fresh verification.

Evidence authority includes shared startup classification, P4.1d-1
activation/recovery, Windows retirement, App bootstrap-boundary, daemon,
daemonctl, migration, version, release-identity, updater, and Sentry tests plus
the full Rust/frontend/build gates.

RED was observed before implementation: three desktop identity assertions
failed against the legacy configuration, and the Rust suite could not compile
because the shared startup authority did not exist. GREEN counts and Git refs
were freshly verified at closeout:

- startup activation gates: App/shared `5/5`, daemon `5/5`;
- migration/activation foundation: App/shared `57/57`, daemon `57/57`;
- frontend startup boundary and IPC: `76/76`;
- version authority: `29/29` at `0.7.68 · Build 4 · development`;
- Rust all-targets: library `1120 passed, 4 ignored`, daemon `1028 passed,
  3 ignored`, daemonctl `29 passed`, config tests `3 passed`, and zero failures;
- frontend full suite: `1235 passed`, with only the six pre-existing named
  locale/date baseline failures (five `useTraySessionUsage` assertions and one
  `Home` date assertion); no new waiver was introduced;
- `cargo check --all-targets`, `cargo fmt --all -- --check`, TypeScript
  typecheck, production frontend build, version projection check, and
  `git diff --check`: PASS.

No real user data, identity, credential, daemon, Thread, or rollout was used.
Real user migration and installed-package acceptance remain **NOT EXECUTED**.

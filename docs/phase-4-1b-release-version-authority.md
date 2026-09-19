# P4.1b — Release Identity Contract / Version Authority

Status: **PASS / COMPLETE / FROZEN**. P4.1c is complete and frozen; P4.1d is
not started.

## Software version authority

Root `VERSION.json` is the only current version authority:

```json
{
  "version": "0.7.68",
  "build": 2,
  "status": "development"
}
```

Build 1 is the first formal CodexMonitor baseline under the unified authority;
no historical Build was inferred. Build increments monotonically for a verified
product-state change and never resets when A, B, or C changes. A.B.C remains
the platform ordering version. Build is traceability metadata, not SemVer
precedence. `status` remains `development` until explicit release authorization.

P4.1c closeout advanced the current authority to Build 2 without changing
SemVer or development status.
P4.1d-1 closeout advances it to Build 3 under the same rule.

`scripts/version-authority.mjs` provides separate `check`, `sync`, `bump`, and
`trace` operations. Bump changes only `VERSION.json`; sync is explicit. Check is
read-only and fails closed on drift. Trace derives the Git commit at invocation
time and never writes it back into the authority.

The projections are `package.json`, root entries in `package-lock.json`, the
CodexMonitor package entries in `src-tauri/Cargo.toml` and `Cargo.lock`, Tauri
`version`, and Apple `CFBundleShortVersionString` plus numeric
`CFBundleVersion=Build`.

## Release identity authority

`release-identity.json` freezes the future distribution target:

- product name: `CodexMonitor DeanX`;
- desktop identifier: `io.github.deanxie.codexmonitor`;
- iOS identifier: `io.github.deanxie.codexmonitor.ios`;
- publisher/repository: `DeanXie/CodexMonitor`;
- migration required: true;
- Windows installer product identity is stable across Builds and inactive until
  P4.1d.

The current Tauri product name and desktop/iOS identifiers remain `Codex
Monitor`, `com.dimillian.codexmonitor`, and
`com.dimillian.codexmonitor.ios`. Target identity is metadata, not runtime
cutover. No data directory, daemonctl lookup, user setting, credential,
RemoteHostIdentity, or installed product is read or migrated in P4.1b.

The identity manifest's `configSchemaVersion` is the P4.1c migration/config
schema authority and is independent of software version and Build. Build
changes never increment configuration schema automatically.

## Build and release gates

Production frontend/Tauri builds, CI Tauri builds, GitHub release jobs, and the
iOS TestFlight script consume the canonical authority. The old release-workflow
post-release patch bump was removed; a developer must explicitly choose build,
patch, minor, or major, then sync and commit the result. Updater artifacts and
third-party Sentry remain disabled from P4.1a.

The sanitized fixture family is
`docs/fixtures/phase-4-1b-release-version-authority/`. It covers version drift,
bump semantics, legacy/target separation, installer stability, and absence of
real user data. P4.1b performs zero real migration and does not activate the
DeanX identifiers.

## Verification evidence

- The initial contract suite was RED with 22 failures because no canonical
  manifest, tooling, target identity, or workflow drift gate existed.
- The final P4.1b suite is GREEN: 29/29 assertions pass, including projection
  drift, monotonic Build bumps, release workflow consumption, inactive target
  identity, and P4.1a non-regression.
- P4.1a updater/Sentry focused regression passes 72/72.
- Rust all-targets, `cargo check --all-targets`, `cargo fmt --all -- --check`,
  TypeScript typecheck, and the release-like production frontend build pass.
- The frontend full suite keeps exactly the six pre-existing locale/date
  baseline failures (1231 pass, 6 waived); P4.1b adds no waiver.

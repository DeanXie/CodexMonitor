# P4.1a — Release Safety Switches

Status: **PASS / COMPLETE / FROZEN**.

P4.1a establishes the safety boundary for the first DeanX daily-use
distribution. It does not select the future release identifier, migrate user
data, generate signing keys, or publish a release.

## Updater boundary

The inherited in-app updater is unavailable at every executable boundary:

- startup and manual frontend operations use an inert compatibility surface;
- persisted `automaticAppUpdateChecksEnabled` values cannot enable updates;
- the app menu exposes no update command or update event;
- the frontend updater SDK is not a dependency;
- the Rust updater plugin is not registered or linked;
- desktop capabilities expose no updater permission;
- base and Windows Tauri configuration both set
  `createUpdaterArtifacts` to `false` and configure no updater endpoint or
  public key;
- post-update release-note fetching is unavailable;
- the release workflow builds ordinary installers and release notes but does
  not create, validate, sign, or upload an updater manifest or updater
  artifacts.

This freeze preserves ordinary macOS, Linux, and Windows package outputs. A
future updater requires a separately authorized DeanX endpoint, key chain,
rollback contract, and installed-release acceptance.

## Third-party telemetry boundary

The application does not initialize Sentry and has no production Sentry event
calls. A default DSN or `VITE_SENTRY_DSN` value cannot enable telemetry because
the Sentry SDK is not a runtime dependency. React startup, local error toasts,
console diagnostics, and existing local error paths remain available.

P4.5 still owns redacted local logs, diagnostic export, abnormal-exit evidence,
and crash recovery.

## Evidence boundary

The TDD RED baseline was observed before implementation: the release-like
frontend cases invoked Sentry initialization and the updater check, while the
configuration tests found updater artifacts enabled and the inherited
`latest.json` workflow present. The final focused frontend suite passes 72/72
and the release configuration suite passes 3/3.

Focused release-like frontend tests prove that automatic and manual updater
operations remain inert and application startup succeeds with a residual DSN
environment value. Rust configuration tests prove the base/Windows artifact,
capability, installer, release-notes, and workflow boundaries.

Fresh closeout verification records 2,022 Rust tests passed with 7 ignored,
`cargo check --all-targets`, `cargo fmt --all -- --check`, TypeScript typecheck,
the production frontend build, and `git diff --check` passing. The full
frontend suite records 1,231/1,237 passed; the remaining six failures are the
pre-existing approved zh-CN locale/date assertions, with no new waiver.

Installed Windows package validation is **NOT EXECUTED** in P4.1a. P4.2 must
verify that an installed build starts normally, exposes no update entry point,
performs no updater or Sentry network activity, and produces ordinary installer
artifacts without an updater manifest.

## Scope exclusions

P4.1a changes no `identifier`, `productName`, data directory, migration logic,
real user configuration, credentials, or signing keys. P4.1b is not started.

# P4.1d-4c — Entry Gate Evidence / Restart UX / Isolation Closeout

Status: **PASS / COMPLETE / FROZEN** for the scoped R04, R06, and R11
contracts. P4.1e remains **PAUSED / RESUME PENDING** and P4.2 is not started.

## Native entry gate

All native business menu and current macOS tray actions consult the same
`BootstrapState` current-process runtime gate used by normal business IPC.
`blocked`, `validating`, and `failed` emit or dispatch no business action.
`ready` preserves the existing event mapping. Shell-only window/about/quit
actions remain available. This slice does not add a Windows tray.

The desktop setup effect plan starts global sources and daemon management only
for a `ready` current process. The setup path itself creates no
`WorkspaceSession`. Candidate state construction during validation is not
business readiness.

## Restart UX decision

Fresh activation, legacy migration, and activation recovery stop at a committed
profile and return `restartRequired=true`. The current process remains gated and
the bootstrap screen instructs the user to restart. No hot-switch is supported.
The next process strictly inspects and validates the same committed identity;
restart does not remigrate or generate a replacement UUID.

## Executed-test isolation

Production-entry child-process tests run with disposable roots for `APPDATA`,
`LOCALAPPDATA`, `USERPROFILE`, `HOME`, `CODEX_HOME`, `TEMP`, `TMP`,
`XDG_DATA_HOME`, and `XDG_CACHE_HOME`. The daemon token is removed unless a test
explicitly supplies a fake value. Product binaries receive an explicit absolute
temporary data root and relative/cwd escape remains rejected.

No real user data, credentials, identity, App, daemon, installer, or WebView was
read, changed, stopped, or launched. Source-build and temporary-process evidence
does not constitute installed-package acceptance.

## Fresh closeout verification

- Rust all-targets: 2,254 passed, 0 failed, 9 ignored.
- Focused runtime-validation isolation: 11 passed.
- Focused native startup/menu/tray gates: 14 passed.
- Frontend bootstrap boundary: 4 passed.
- P4.1 compatibility contract: 10 passed.
- Frontend full suite: 1,237/1,243 passed. The six failures are exactly the
  frozen zh-CN/date expectation waivers (five `useTraySessionUsage`, one
  `Home`); there are no new failures or waivers.
- Rust check, Rust format check, TypeScript typecheck, version projection
  check, and Git diff check passed.
- Windows production build produced MSI and NSIS bundles in external build
  storage. No installer or built application was executed.

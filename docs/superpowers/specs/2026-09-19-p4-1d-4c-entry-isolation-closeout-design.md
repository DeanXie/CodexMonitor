# P4.1d-4c Entry Gate Evidence / Restart UX / Isolation Closeout Design

Status: approved for implementation from `2e07fea3768a9c8120ff5d13e27b03380dc0fd24`.

## Goal

Close only compliance findings R04, R06, and R11 without executing a real
profile cutover, restoring P4.1e, starting P4.2, or broadening the P4.1 startup
architecture.

## R04 native and startup gate

A single backend helper classifies a native action as shell-only or business.
Shell-only About, quit/exit, close/hide, minimize, maximize, and fullscreen
remain available before READY. Every native action that emits a business menu
event or opens a tray Thread requires the managed `BootstrapState` to report
current-process business access. Missing state, BLOCKED, VALIDATING, and FAILED
all fail closed. The menu and current macOS tray callback use this helper; no
Windows tray is added.

The existing setup order remains authoritative. A small startup decision seam
records that global-source start and daemon auto-start are allowed only for a
READY process. Candidate state construction, validation, and management remain
in `lib.rs`; the seam exists to test real setup decisions without creating a
second startup framework. Exit cleanup remains a safe no-op when AppState is
absent.

## R06 restart-required policy

Fresh activation, legacy migration, and recovery that completes an activation
transaction return `restartRequired = true`, remain business-blocked in the
current process, and stop at `target_committed`. The first release does not
hot-switch AppState, global sources, daemon/background state, or identity
authority. A new isolated process performs d-3 current-process runtime
validation and may then become READY. A runtime-only retry that commits no new
activation transaction is not forced through another restart.

The existing BootstrapBoundary remains the only bootstrap UI. It shows an
explicit restart message and never renders business children while restart is
required.

## R11 executed-test isolation

Production-entry child-process tests use a test-only
`IsolatedProcessEnvironment`. It rewrites APPDATA, LOCALAPPDATA, USERPROFILE,
HOME, CODEX_HOME, TEMP, TMP, XDG_DATA_HOME, and XDG_CACHE_HOME to unique
test-owned roots, removes any inherited `CODEX_MONITOR_DAEMON_TOKEN`, and may
set only synthetic endpoint/token values needed by a test. Explicit data roots
must remain inside the sandbox and relative/cwd escape attempts fail closed.

The harness never reads the real value of a credential or user root. A sentinel
test proves the child sees sandbox content only. Tauri bundle creation does not
launch WebView or installers; installed-package isolation remains P4.2
`NOT_EXECUTED`.

## Evidence and non-goals

RED must come from the production menu/tray/startup entry path or a real
controlled child. GREEN must report zero blocked event emission, state access,
session creation, daemon start, real user data access, real migration, identity
retirement, daemon stop, WebView launch, and installer launch.

No real AppData, settings, credentials, HostIdentity, CODEX_HOME, App, daemon,
or installer is accessed or operated. No new tray feature, RBAC system,
business generation, activation transaction behavior, or P4.1e/P4.2 behavior
is introduced.

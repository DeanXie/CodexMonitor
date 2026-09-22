# P4.2 Windows Installed Daily-use Baseline Design

Status: **DRAFT / FORENSICS COMPLETE / DECISION REQUIRED**.

## Goal and evidence rule

P4.2 proves that an ordinary Windows user can install, launch, bootstrap,
restart, upgrade, and uninstall CodexMonitor without a repository or developer
toolchain. Source/config and installer-static evidence may establish structure,
but cannot substitute for disposable installed-app E2E.

## Observed baseline

The Build 8 clean production build creates MSI and NSIS packages containing
the main executable and daemon in one directory. The packages differ in scope,
elevation, Desktop shortcut control, and uninstall implementation. Both use
automatic WebView2 network bootstrap. Windows-visible versioning omits Build.
Full evidence is in `docs/phase-4-2a-windows-installer-forensics.md`.

## Options

### Option A — NSIS per-user is canonical (recommended)

Use NSIS as the daily-use acceptance and release authority. It matches
ordinary-user scope and already provides the approved interactive Desktop
shortcut opt-out. Correct WebView2 delivery, remove the cwd fallback, and test
its uninstall/upgrade behavior. MSI may remain a non-authoritative artifact or
be removed from releases after an explicit release decision.

Tradeoff: simplest safe path, but enterprises wanting per-machine MSI do not
receive identical semantics in P4.2.

### Option B — MSI per-machine is canonical

Accept elevation/Program Files as the product contract and add a real WiX UI
choice for Desktop shortcut opt-out. Correct WebView2 delivery and validate
per-machine uninstall/data semantics.

Tradeoff: better enterprise shape, higher implementation/test complexity, and
does not meet an ordinary non-admin install expectation.

### Option C — ship MSI and NSIS with full parity

Define matching shortcut, WebView2, data retention, upgrade, and launch
semantics while retaining different scopes.

Tradeoff: widest distribution surface and roughly doubles installed acceptance
work. It is not justified for the first DeanX daily-use baseline.

## Recommended contract

Choose option A; bundle the WebView2 offline installer for deterministic
network-silent installation; require SemVer to advance for every published
Windows installer while Build remains monotonic; retain daemonctl outside the
ordinary GUI installer. Remove the dictation cwd fallback. This recommendation
is not frozen until the user decides the three release questions.

## Installed acceptance architecture

Use a Windows Sandbox or VM restored from a clean snapshot. Do not redirect
the real DeanX profile. Guest registry and Shell folders remain genuine, while
all package, profile, migration, and shortcut state is disposable. Export only
hashes, manifests, logs, screenshots, process/port evidence, and redacted
network traces.

The A–Q fixture defines precondition, action, expected and forbidden results,
evidence class, destructive scope, and cleanup for every case. A case moves
from `NOT_EXECUTED` only after guest evidence exists. Real-user acceptance, if
later requested, is a separate evidence class.

## Safety boundaries

- No use of the real AppData profile, credentials, daemon, or HostIdentity.
- No source-build result is called an installed acceptance result.
- No updater/telemetry request is allowed; normal explicit product networking
  is classified separately.
- Optional Codex/Git/gh failures must not prevent the installed shell from
  starting.
- Wrong/corrupt profiles must fail closed without cwd/default fallbacks.
- P4.2 does not begin launcher/tray P4.3 behavior.

## Completion gate

P4.2 may close only after a selected package contract, packaging corrections,
and all applicable A–Q guest cases pass with zero critical blocker. Until then
P4.2 remains in progress and this contract remains draft.

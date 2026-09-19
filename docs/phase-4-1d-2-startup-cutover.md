# P4.1d-2 — Startup Cutover / RemoteHostIdentity Safe Migration

Status: **PASS / COMPLETE / FROZEN** after the fresh closeout recorded in the
evidence index. P4.1d production wiring is complete; no real user migration or
HostIdentity retirement was executed. P4.1e is not started.

Historical correction: P4.1d-3 later proved that this slice's file commit path
wrote `runtime_validated` before a real runtime handshake. The d-2 acceptance
record remains historical; current startup authority is the corrected
`target_committed` plus current-process validation contract in
`phase-4-1d-3-runtime-validation-handshake.md`.

Canonical software authority is `v0.7.68 · Build 4 · development`.

## Startup authority

Desktop App, daemon, and daemonctl now resolve the approved target profile and
use the same activated-profile validator. Normal business loading requires an
activation manifest, an active v2 RemoteHostIdentity bound to the same
transaction, an external committed activation journal bound to the same
transaction and canonical target root, parseable settings, and an array-shaped
workspace store. P4.1d-3 additionally requires each current App or daemon
process to perform runtime validation before normal business access.

Fresh, legacy, recovery, conflicting, and corrupt roots remain distinct.
Inspection is read-only and never generates settings, workspaces, or an
identity. `--data-dir` selects a candidate root but cannot bypass activation.
Default-root resolution has no current-directory fallback.

The desktop App manages only bootstrap state until the gate reports `ready`.
AppState, global sources, daemon autostart, WorkspaceSession creation, and
Remote auto-connect therefore do not run while activation is pending. The
minimal bootstrap surface can explicitly create a fresh profile and can resume
only recovery steps that do not newly retire a legacy identity. A migration
still before identity retirement remains blocked until separately controlled
old-process stop evidence is available.

The daemon validates the profile and acquires the P4.1d-1 service-lifetime lock
before constructing state or binding a listener. Daemon state uses strict
typed settings/workspace reads and the active v2 identity. Daemonctl validates
the same root before preview, status, stop, or start and passes that exact root
to the daemon.

## Release identity boundary

Desktop product name and identifier are now `CodexMonitor DeanX` and
`io.github.deanxie.codexmonitor`. Base, Windows, Linux, macOS display metadata,
installer authority, App root, daemon default root, and daemonctl default root
use that desktop authority.

The iOS override remains `com.dimillian.codexmonitor.ios`; iOS container
migration remains a separate platform decision. Updater and Sentry remain
disabled. Migration output remains credential-free; missing Remote token or
host pin is not trusted or auto-connected.

## Verification boundary

All activation and migration tests use temporary roots, fake settings, fake
identities, and controlled test processes. Real AppData, real settings,
credentials, canonical Thread/rollout data, and the live daemon were not read,
modified, stopped, or migrated. Source/config cutover and temporary-profile
tests do not constitute a real user cutover or installed-package acceptance.

The v2 retirement mechanism blocks the fixed legacy loader compatibility target
defined by P4.1d-1 after retirement. It does not prove every historical UI
binary cannot open or prevent manually copied identity material elsewhere.

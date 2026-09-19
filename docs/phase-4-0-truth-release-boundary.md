# Phase 4.0 — Truth / Release Boundary Freeze

Status: **PASS / COMPLETE / FROZEN**. Phase 4 is **IN PROGRESS**. P4.1 is
**PASS / COMPLETE / FROZEN**; P4.2 is not started. The approved desktop identity,
version authority, migration preparation, activation protocol, and fail-closed
startup gate are complete. Real-user migration and installed acceptance remain
unexecuted.

The root `CodexMonitor_四阶段开发路线图.md` remains the sole current-state
authority. This document is the Phase 4 product/release contract, not a second
current-state ledger.

## Product objective and delivery order

Phase 4 turns CodexMonitor from an engineering/development system into software
that a user can install, start, operate, diagnose, and maintain every day.

Delivery order is fixed:

1. Windows Daily-use Release;
2. macOS release and acceptance;
3. iOS real target/build/device acceptance.

Windows delivery does not prove macOS or iOS acceptance and does not complete
Phase 4. Adaptive Model Router remains an **Advanced Phase** concern and is not
part of Phase 4 or the Windows milestone.

## User-approved product decisions

These decisions are approved requirements, not open design questions.

1. **Distribution identity.** The DeanX distribution will use an independent
   identity and will not retain `com.dimillian.codexmonitor` as its long-term
   release identity. P4.1 selects the concrete identifier and migration; P4.0
   changes neither.
2. **Updater.** Automatic update is disabled by default for the first Windows
   daily-use release. A later DeanX release endpoint, signing key/public key,
   update acceptance, and rollback acceptance replace the current Dimillian
   chain. P4.0 changes no endpoint or key and generates no key.
3. **Data migration.** Use a controlled one-time whitelist migration: detect old
   data, back it up, migrate allowlisted Monitor data, validate it, and retain
   the old data temporarily. Candidate data is settings, workspaces, and
   necessary Monitor state. Excluded data is `auth.json`, credentials/tokens,
   `CODEX_HOME`, canonical Threads, canonical rollout data, and unknown caches.
   `RemoteHostIdentity` may continue only as part of the same formal service
   migration after the old daemon is confirmed stopped; two live instances
   must never share that formal identity.
4. **Launch path.** CodexMonitor is the sole primary entry and initializes its
   required background components. Opening Codex Desktop is optional. Login
   autostart is opt-in and off by default. Default Desktop process monitoring or
   hijacking is prohibited.
5. **Window close.** On Windows and macOS, closing the desktop window hides it
   to the tray while the app continues. True exit is explicit through
   **Exit CodexMonitor** in the tray.
6. **Daemon lifecycle.** Exit safely stops only a daemon with explicit
   app-owned lifecycle evidence. A separate setting may keep the remote service
   running after UI exit. Bulk or fuzzy process-name killing is prohibited.
7. **Shortcuts.** Start Menu shortcut is created by default. The installer's
   Desktop shortcut option is checked by default and can be deselected.
8. **Monitor Window.** The first release supports the main window plus at most
   one independent Monitor Window. It can move to a second display, close and
   reopen, persist size/position, and recover into a visible area after display
   removal. Both windows share one authority/data source and must not duplicate
   WorkspaceSessions, collectors, or token accounting.
9. **Diagnostics and telemetry.** Third-party Sentry auto-upload is disabled for
   the first release. Use redacted rolling local logs, abnormal-exit evidence,
   and user-triggered redacted diagnostic export with no automatic upload.
   Future remote telemetry requires separate opt-in.
10. **Timeline and history.** Deliver a lightweight Timeline for Turn,
    agent/sub-agent, model-reroute, approval, and important reconnect/runtime
    events, plus filterable Session History with time, Workspace/Project,
    status, open-existing-session, and pagination/progressive loading. Do not
    build full event sourcing, permanent all-event retention, full replay, or
    unlimited history.
11. **Project statistics.** Workspace is the technical
    execution/session/thread boundary; Project is the user-facing aggregation
    boundary. CodexMonitor maintains a lightweight Project-to-Workspace mapping
    and aggregates using canonical identity/evidence deduplication. Blindly
    summing Workspace counters is prohibited.

## Current release truth under the P4.0 contract

- Tauri/package/Cargo version remains `0.7.68`; the monotonic Build is governed
  by root `VERSION.json`;
- desktop product/identifier are `CodexMonitor DeanX` and
  `io.github.deanxie.codexmonitor`;
- iOS remains `com.dimillian.codexmonitor.ios` pending its separate platform
  migration decision and acceptance;
- the original upstream updater endpoint, public key, SDK, plugin, permissions,
  UI/menu entry points, updater artifacts, and manifest workflow are disabled
  by P4.1a;
- third-party Sentry initialization and event calls are disabled by P4.1a;
- P4.5 still owns local logging, diagnostics export, and crash recovery.

P4.1b establishes root `VERSION.json` as the single software version authority;
P4.1d-1 advanced the authority to `v0.7.68 · Build 3 · development`, P4.1d-2
advanced it to Build 4, P4.1d-3 advanced it to Build 5, P4.1d-4a advanced it
to Build 6, and P4.1d-4b advances it to `v0.7.68 · Build 7 · development`
without changing SemVer or status.
Build 1 remains the first formal unified
version-authority baseline. Package/Cargo/Tauri/Apple projections,
drift checks, deterministic bump/sync tooling, and commit trace output all flow
from it. Build remains monotonic and never substitutes for SemVer ordering.
The DeanX desktop identity frozen in `release-identity.json` is now active in
desktop configuration. P4.1c prepares only sanitized, allowlisted staging data
from explicit roots and does not migrate credentials or RemoteHostIdentity.
P4.1d-2 wires the P4.1d-1 foundation into startup, but no real user migration,
HostIdentity retirement, installed application cutover, or iOS migration has
been executed.

The main checkout's local `.gitignore` addition, metadata-only `main.rs` state,
and two development BAT files are not production build or installed-runtime
dependencies. The BAT files remain local development tools and are not release
artifacts.

## Implementation roadmap and completion gates

### P4.0 — Truth / Release Boundary Freeze

Evidence classifications and root current state are corrected; all decisions,
platform order, and this roadmap are frozen; no product behavior changes.

### P4.1 — Release Identity / Version / Migration / Update Safety

DeanX identity contract; one version authority and drift gate; controlled
migration with backup/rollback tests; first-Windows-release updater safely
disabled; `RemoteHostIdentity` migration safety. **CLOSEOUT IN PROGRESS**:
P4.1d-4a resolved R02/R05/R07 and P4.1d-4b resolves R01/R03 for its supported
Windows scope, while R04/R06/R11 remain open and P4.1e is paused. See
`docs/phase-4-1-closeout.md`.

### P4.2 — Windows Installable Daily-use Baseline

A clean checkout builds installable Windows artifacts; ordinary-user install
and launch work without a development environment; Start Menu/Desktop shortcut
contract and installed-app smoke pass; no dependency on the four local states.

### P4.3 — Launcher / Tray / Background Lifecycle

Single CodexMonitor entry, optional Desktop launch and login autostart,
close-to-tray and explicit Exit, evidence-bound app-owned daemon lifecycle,
optional daemon retention, and no bulk process killing.

### P4.4 — Independent Monitor Window

One independent Tauri Monitor Window with second-display, shared-authority,
close/reopen, persisted geometry, display-disconnect recovery, and zero duplicate
collector/session/token-accounting acceptance.

### P4.5 — Logging / Diagnostics / Crash Recovery

Redacted rolling logs, abnormal-exit evidence, safe restart recovery, redacted
diagnostic export, third-party telemetry disabled by default, and no mutation
replay during recovery.

### P4.6 — UI / Timeline / History / Project Statistics Closeout

Reuse existing Compact/page/split/freshness/usage capabilities; finish the
lightweight Timeline, filterable Session History, Project-to-Workspace mapping,
canonical deduplicated Project statistics, and long-data/offline/empty-state
regressions.

### P4.7 — Platform Release Acceptance

Independently record Windows installed acceptance, macOS
package/sign/notarize/install acceptance, iOS target/build/sign/install/device
acceptance, and final update/migration acceptance where applicable.

## Windows Daily-use Milestone

This milestone is not a Phase. It becomes **AVAILABLE** when P4.1 through P4.4
are complete and no critical P4.5 blocker prevents safe daily use. Phase 4 then
remains **IN PROGRESS** through P4.5-P4.7.

## Evidence classification boundary

Phase 3.5 evidence is classified separately as real daemon/Node integration,
production-function regression, deterministic fixture/contract evidence, or
installed-app/manual UI evidence. The final acceptance's
`staleOldGenerationRejected` value came from a deterministic stale-delivery
fixture. Product isolation regression coverage remains PASS; a real delayed old
socket notification scenario is **NOT_EXECUTED**. This correction neither
reopens A3 nor declares the product logic failed.

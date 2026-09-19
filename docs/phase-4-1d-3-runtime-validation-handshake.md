# P4.1d-3 — Runtime Validation Handshake / Startup Gate Correction

Status: **PASS / COMPLETE / FROZEN**.

P4.1d-1 and P4.1d-2 remain historical acceptance records. P4.1d-3 records a
subsequently proven contract defect: the original commit path advanced the
activation journal to `runtime_validated` after file validation alone. This
slice corrects that defect without repeating migration, changing the identity,
or touching a real user profile.

## Three distinct authorities

1. `target_committed` means the complete target file set, manifest, active v2
   identity, transaction binding, root binding, settings JSON, and workspace
   JSON passed static validation.
2. The process-local gate is `blocked`, `validating`, `ready`, or `failed`.
   Persisted history never makes a new process ready.
3. Normal business IPC/RPC is available only when the current process reaches
   `ready` through its internal initialization path.

Fresh and migration commit paths stop at `target_committed`. File copy, rename,
manifest creation, schema validation, recovery inspection, daemonctl status,
and daemonctl command preview cannot write runtime success. A historical
`runtime_validated` journal is accepted as committed history, but App and daemon
still repeat strict current-process validation.

## Required initialization

| Runtime | Required before process READY | Optional or post-READY |
| --- | --- | --- |
| App | strict committed-profile validation; typed settings and workspace load; active v2 identity validation; transaction/root recheck; runtime journal completion; process gate publication | global-source scan, Remote auto-connect, daemon management, WorkspaceSession creation |
| daemon | strict committed-profile validation; auth/listen configuration; service-lifetime lock; typed settings/workspace state; active v2 identity; listener bind; transaction/root recheck; runtime journal completion; process gate publication | WorkspaceSession connections and Remote workspace activity |
| daemonctl | strict committed-profile inspection; for `start`, spawn the child and wait for authenticated `daemon_info` matching service, mode, version, and child PID | `status` and `command-preview` remain read-only; daemonctl never writes runtime success |

Listener binding during daemon validation is resource preparation, not service
readiness. The accept/dispatch loop starts only after the journal update and
process gate both succeed. A failure drops the candidate listener.

## Failure and recovery matrix

| Event | Persisted journal | Process gate | Business calls |
| --- | --- | --- | --- |
| fresh/migration file commit | `target_committed` | `blocked` | 0 |
| candidate App/daemon state failure | unchanged | `failed` | 0 |
| daemon listener bind failure | unchanged | `failed` | 0 |
| required initialization succeeds but runtime journal write fails | `target_committed` | `failed` | 0; candidate resources released |
| root or transaction changes before completion | unchanged | `failed` | 0 |
| initialization and bound journal update succeed | `runtime_validated` | `ready` for that process only | allowed |
| old `runtime_validated` record plus current initialization failure | historical record retained | `failed` | 0 |

`target_committed` recovery validates the existing committed target. It does
not rerun migration, generate a new UUID, retire the legacy identity again, or
delete committed data. Concurrent runtime validators share only the short
journal critical section: one may complete while another fails closed on lock
contention, and later strict validation is idempotent.

## Explicit non-goals and evidence boundary

- No public payload, environment variable, frontend flag, or CLI flag can set
  process readiness.
- Remote connectivity and all WorkspaceSessions being online are not activation
  requirements.
- No new business generation, ownership, lease, or telemetry framework exists.
- Tests use temporary profiles, fake identities, isolated sockets, and
  controlled child processes only.
- Real user migration, real HostIdentity retirement, installed-package
  acceptance, P4.1e, and P4.2 are not executed by this slice.

## Fresh closeout verification

- Shared Rust library: 1,133 passed, 0 failed, 4 ignored.
- Daemon: 1,036 passed, 0 failed, 3 ignored.
- Daemonctl: 29 passed, 0 failed.
- Production-entry handshake integration: 6 passed, 0 failed.
- Frontend full suite: 1,235 passed; only the six previously frozen locale/date
  baseline assertions failed, with no new waiver.
- Rust check, Rust formatting, frontend typecheck, frontend production build,
  version authority, and diff check passed.
- The repository-managed Tauri production build completed and produced both
  MSI and NSIS bundles in the isolated worktree's external Cargo target. The
  bundles were not installed or launched.
- Canonical version authority is `0.7.68` Build `5`, status `development`.
- Real user data reads/writes, migration attempts, HostIdentity retirements,
  formal daemon operations, and business mutations: 0.

P4.1d-3 restores the technical precondition for resuming P4.1e closeout, but
does not execute or complete P4.1e.

# P4.1d-4a — Recovery Reachability / Root Authority Correction

Status: **PASS / COMPLETE / FROZEN**. Parent P4.1d remains **IN PROGRESS**.

Canonical software authority at this correction's closeout is
`v0.7.68 · Build 6 · development`.

P4.1d-4a preserves the P4.1d-1 through d-3 history and corrects the later
compliance audit findings R02, R05, and R07. It does not implement the legacy
migration product entry (R01), production stop-evidence acquisition (R03), the
remaining full business-entry audit (R04), a new restart/continue product
choice (R06), or the full installed-runtime isolation matrix (R11).

## R02 / R07 recovery authority

Startup inspection is read-only and combines manifest, journal, source,
target, staging, identity state, transaction/root binding, schema, profile
artifacts, and migration identity-recovery material. The resulting contract is:

| Actual evidence | Startup result | Business access |
| --- | --- | --- |
| Valid committed profile with committed journal | `runtime_validation_required` | closed |
| Target committed while journal lags at `prepared` or `legacy_identity_retired`, and all evidence agrees | `recovery_required` | closed |
| Historical `runtime_validated` with valid files | `runtime_validation_required` for each new process | closed until that process validates |
| Transaction/root/identity mismatch, unsupported schema, corrupt file, or missing recovery material | `blocked_corrupt` | closed |

The recovery adapter re-inspects after explicit recovery intent and acquires
the existing activation commit lock. `validate_committed_target` convergence
revalidates current files, source retirement, recovery material, and bindings,
then advances only the lagging journal to `target_committed`. It does not move
profile files, retire an identity again, generate a UUID, remove recovery
material, rewrite legacy settings/workspaces, or publish process readiness.
Current-process runtime validation remains the only path to `ready`.

A migration still at `continue_before_retirement` is deliberately not resumed:
the adapter requires new trusted stop evidence. P4.1d-4b supplies that evidence
through its backend-native guarded product flow; d-4a itself does not
manufacture it.

## R05 root authority

`validate_data_root` is the shared App/daemon/daemonctl rule. An explicit root
must already be a platform absolute path before profile reads, file creation,
listener binding, or child launch. Relative, current-drive-relative, and
root-relative Windows inputs are rejected; they are never joined to the
current directory or replaced by the default root. Default root resolution
fails closed when the platform data base is unavailable. Supported absolute
paths, including spaces and non-ASCII names, remain valid and are passed
unchanged into profile inspection and child startup.

Existing canonicalization, alias, reparse, and junction rules remain owned by
the P4.1d-1/P4.1c path-safety core. This slice does not add UNC, device-path, or
network-root support.

## Evidence boundary

The retained RED evidence was: lagging committed targets were classified
`blocked_corrupt`; the recovery adapter performed no journal convergence for
`validate_committed_target`; and daemon/daemonctl accepted relative explicit
roots. GREEN evidence covers shared startup classification, the real bootstrap
recovery adapter, daemon/daemonctl argument entrypoints, d-1 interruption
recovery, and d-3 current-process readiness.

All tests use temporary roots, fake settings/workspaces/identities, isolated
loopback listeners, and controlled children. Real AppData, credentials,
HostIdentity, user profiles, canonical Threads/rollouts, App/daemon processes,
and migration state were not read, changed, stopped, or activated. No real
user cutover or installed-package acceptance was performed.

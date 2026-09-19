# P4.1d-4b — Controlled Legacy Migration Entry / Stop Evidence

Status: **PASS / COMPLETE / FROZEN**. Parent P4.1d remains **IN PROGRESS**;
P4.1e remains **PAUSED / RESUME PENDING** and P4.2 is not started.

This slice resolves compliance findings R01 and R03 for the supported Windows
scope. It adds a restricted bootstrap preview/confirm flow and backend-private
native stop evidence. User confirmation means only consent to migrate; it does
not prove that an old process stopped.

## Product and evidence authority

`preview_legacy_migration` is read-only and returns an opaque preview ID plus
allowlisted categories, exclusions, warnings, conflicts, schema versions, and
restart requirement. It returns no values, credentials, identities, process
details, or unrestricted paths. `confirm_legacy_migration` accepts only that ID
and the fixed `confirm_legacy_migration` intent. The frontend cannot send a PID,
source root, `confirmedStopped`, or `safeToRetire` value.

The Windows provider classifies the exact supported executable scope as
`RUNNING`, `UNKNOWN`, or `VERIFIED_QUIESCENT_WITHIN_SUPPORTED_SCOPE`. An
inaccessible or ambiguous candidate is `UNKNOWN` and fails closed. A controlled
child that read the v1 identity, closed the file, and remained alive was still
observed as `RUNNING`; an identity file guard is not process-stop evidence.

The first retirement lock order is:

```text
activation commit lock
→ protected legacy identity handle
→ identity/recovery binding validation through that handle
→ native stop-evidence revalidation
→ ReplaceFileW retirement
→ target commit
```

The protected handle is not released and the identity path is not reopened for
the decisive validation. A source already at `retired_v2` with the exact journal
transaction is recovery evidence: it does not request first-retirement stop
evidence and is not retired again.

## State and recovery boundary

A fresh confirmation and a prepared pre-retirement recovery both require new
native stop evidence. Preview expiry, reuse, source change, migration-policy or
binding change, target conflict, or staging mismatch fails closed. A successful
migration advances only through `prepared → legacy_identity_retired →
target_committed` and returns restart-required. It never writes
`runtime_validated` and never publishes business READY; P4.1d-3 current-process
runtime validation remains the sole READY authority.

## Proof limits

The supported process scope is the exact executable image used by the current
installation. An inaccessible same-name candidate is treated as UNKNOWN.
Unverified historical binary names, launchers, copied identities, and arbitrary
external directories are outside this proof. All tests used temporary roots,
fake identities, and controlled child processes. Real AppData, settings,
credentials, HostIdentity, App/daemon processes, and user migration were not
read, changed, stopped, or activated. Installed-package cutover remains
NOT_EXECUTED. R04, R06, and R11 remain open.

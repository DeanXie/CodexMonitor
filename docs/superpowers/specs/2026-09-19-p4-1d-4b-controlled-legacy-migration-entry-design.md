# P4.1d-4b Controlled Legacy Migration Entry / Stop Evidence Design

**Status:** Approved design awaiting implementation-plan confirmation.

**Baseline:** `db6212375487586835d8cf7aa21749ed8f368255`, `0.7.68 · Build 6 · development`.

## Scope

P4.1d-4b closes only compliance findings R01 and R03. It adds a real local
bootstrap product entry for legacy-profile migration and replaces the current
caller-supplied `LegacyProcessStopEvidence::ConfirmedStopped` value with
backend-private native evidence acquired and consumed inside the protected
identity-retirement critical section.

P4.1d-4a remains the authority for recovery reachability and data-root
validation. P4.1d-3 remains the authority for the separation between durable
`TARGET_COMMITTED`, current-process runtime validation, and business `READY`.
R04, R06, and R11 remain open. P4.1e remains paused and P4.2 is not started.

## Product flow

The only supported migration flow is:

1. startup inspection reports `legacy_migration_required`;
2. the restricted bootstrap UI requests a read-only, sanitized preview;
3. the backend stores a short-lived preview record bound to the exact source
   root, target root, source schema, migration policy version, source snapshot,
   and legacy identity file identity;
4. the UI displays only the allowlisted preview and requires explicit user
   confirmation;
5. confirmation submits the preview identifier and fixed migration intent,
   never process evidence or a trusted boolean;
6. the backend re-inspects the startup disposition and rebuilds the source
   snapshot before any write;
7. native process inspection classifies the supported legacy-process scope as
   `running`, `unknown`, or `verified_quiescent_within_supported_scope`;
8. only verified quiescence may enter the existing staging and activation
   transaction;
9. the identity-retirement critical section reacquires protection and repeats
   root, transaction, identity, and native stop-evidence checks before the
   first legacy-source write;
10. successful activation stops at `target_committed` and returns the existing
    restart-required bootstrap state;
11. after restart, the existing d-3 runtime-validation handshake is the only
    path to current-process business `READY`.

There is no daemon migration RPC, no automatic stop, no retry, and no remote
mutation.

## Preview authority

The public preview contains only:

- an opaque preview identifier;
- source and target product labels, not unrestricted filesystem details;
- recognized source and target schema versions;
- allowlisted migration categories;
- excluded/deferred categories;
- warnings and blocking conflicts;
- whether explicit restart is required.

It excludes settings values, workspace paths, command/argument/endpoint text,
tokens, pins, identities, credentials, process command lines, environment
values, and arbitrary source filenames.

The backend-private record additionally binds:

- canonical source and target roots;
- source fingerprint and identity-file fingerprint/file identity;
- migration policy version;
- creation time and one-time consumption state.

Preview expiration, source mutation, root rebinding, identity replacement,
target/staging conflict, or a second consumer invalidates confirmation. A
failed confirmation never silently refreshes the preview or starts a new
transaction.

## Native stop-evidence contract

The Windows provider inspects only the explicitly supported legacy executable
and legacy-root relationship. It uses native process handles, executable image
identity, process creation time, and the configured-root relationship needed
to classify relevant legacy processes. It does not persist or log full command
lines.

The result is one of:

- `running`: a supported-scope legacy process is alive, including a process
  that cached the identity and later closed the identity file;
- `unknown`: access denial, enumeration ambiguity, identity mismatch, process
  churn, unsupported path/process shape, or incomplete proof;
- `verified_quiescent_within_supported_scope`: the supported enumeration and
  handle checks completed without a matching live legacy process.

Only the last result permits retirement. PID absence, process name, window
title, port availability, frontend confirmation, or a previous preview never
constitutes stop evidence by themselves.

Stop evidence is represented by a backend-private live guard/context, not a
serializable IPC value. A preliminary native inspection is allowed before the
commit begins, but it is not retirement authority. The decisive inspection is
performed again after the migration commit lock and legacy identity protection
are held, and that result is consumed once by the retirement operation. Process
churn or a changed guard fails closed. The authoritative lock order is fixed:

`activation commit lock → identity protection → native stop guard/recheck → retirement → target commit`.

## Reuse of existing migration and activation authority

The implementation must not create a second migration engine. Preview and
confirmation reuse `migration_core` for schema recognition, the fixed
allowlist, secret exclusion, root/staging validation, source fingerprinting,
and deterministic staging. They reuse `activation_foundation` for recovery
materials, protected identity replacement, journal transitions, and target
commit. The new coordinator only binds the product flow and private stop
evidence to those authorities.

The current public-like `LegacyProcessStopEvidence::ConfirmedStopped` input is
removed from normal production call sites. Tests may use an injected provider
that returns deterministic native-equivalent outcomes, but cannot call the
retirement primitive with an untrusted boolean or enum.

## Failure and recovery

- Cancel before confirmation performs zero writes.
- Running or unknown stop state leaves the preview invalidated or explicitly
  blocked, leaves the legacy identity active, and performs zero retirement.
- Failure before retirement uses ordinary staging cleanup rules.
- Once recovery materials are protected or the legacy identity is retired,
  existing d-1/d-4a recovery rules apply; ordinary cleanup cannot delete the
  identity recovery source.
- A source already in `retired_v2` state with the exact matching transaction
  is interrupted-recovery evidence, not a first retirement. That path must not
  request new stop evidence and must not retire the identity again.
- A target committed with a lagging journal remains recoverable through the
  d-4a production adapter.
- Repeated confirmation of the same preview fails closed and cannot perform a
  second retirement or commit.
- Successful migration returns restart-required; this slice does not change
  the R06 restart/continue product choice.

## Evidence boundary

All implementation and validation use temporary roots, fake configuration,
fake identities, controlled child processes, and isolated application data.
No real AppData, settings, HostIdentity, credential, CODEX_HOME, canonical
Thread/rollout, App, daemon, or user migration is read, changed, stopped, or
activated. The Windows provider is validated with controlled native child
processes; installed-package and real cutover acceptance remain unperformed.

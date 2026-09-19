# P4.1d-1 Migration / Activation Safety Foundation

Status: approved for implementation. Runtime cutover remains P4.1d-2.

## Boundary

This slice adds inactive shared-library primitives only. It does not register a
Tauri command, daemon RPC, startup hook, or CLI entry point. The active product
name, identifiers, data-directory resolution, App/daemon/daemonctl startup, and
legacy profile behavior remain unchanged.

All tests use temporary roots, synthetic settings, and synthetic host
identities. Real AppData, credentials, CODEX_HOME, canonical sessions, user
daemons, launchers, and installed applications are out of scope.

## Migration input contract

Source schema and target schema are independent facts. A source with no profile
manifest is accepted only through an explicit `legacy_v0` rule. A supported
manifest version is accepted explicitly; corrupt or future versions fail
closed.

Migration reads raw JSON into fixed projection DTOs. Only named fields are
copied. Execution-bearing command/argument/script fields, credentials, tokens,
RemoteHostIdentity pins, CODEX_HOME, and canonical Thread/rollout data are not
eligible. Adding a field to `AppSettings` or `WorkspaceEntry` does not extend
the migration input allowlist.

## Path and staging contract

Source, target, and staging roots are validated using filesystem identity, not
string inequality. Existing aliases and ancestor/descendant relationships are
blocked. Reparse-point components are blocked on Windows. Staging ownership is
bound to target root identity plus prepared-content fingerprints. A changed
source, replaced staging tree, or concurrently populated target invalidates the
prepared activation.

Once staging is adopted as identity recovery material it is no longer ordinary
P4.1c cleanup material. Generic rollback must refuse to delete it.

## Bootstrap and activation

Read-only bootstrap inspection classifies a profile as fresh, activated-valid,
legacy-migration-required, activation-recovery-required, target-conflict, or
corrupt. Inspection never creates defaults or identities and never falls back
to the current working directory.

Fresh activation order is fixed:

1. explicit fresh intent;
2. prepare defaults and an active v2 identity in staging;
3. validate all prepared artifacts;
4. commit the activation manifest;
5. allow a future P4.1d-2 normal load.

Legacy activation uses a journal, but recovery always combines journal state
with the legacy identity file, protected recovery material, target manifest,
target files, and transaction/root binding. No multi-file sequence is called a
single atomic transaction.

## Host identity

The candidate v2 store has explicit `active` and `retired` states. The baseline
v1 loader is frozen as a compatibility fixture and must reject both v2 states,
corrupt input, and read failures without rewriting or generating an identity.

On Windows, retirement uses a verified no-gap primitive: a handle that denies
read while permitting delete sharing plus `ReplaceFileW`. The real primitive is
tested with temporary files and a child process. If the platform combination
cannot replace atomically while protected, identity retirement is not enabled.

A service-lifetime lock covers only new processes using the same controlled
profile. It is not writer ownership, Remote-client ownership, a distributed
lease, or a guarantee against legacy binaries or copied identities.

## Remote trust and recovery

Imported remote endpoints remain untrusted. Tokens and host pins are excluded;
no automatic connection, pin learning, unauthenticated fallback, retry, or
identity rebinding is introduced.

Recovery distinguishes interruption before activation, interruption after
identity retirement but before journal advance, target commit before journal
advance, and failure after activation. Once a target has accepted new runtime
data, recovery never deletes it or writes the legacy source back automatically.

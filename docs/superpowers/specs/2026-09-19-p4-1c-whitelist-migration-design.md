# P4.1c Controlled Whitelist Migration Engine Design

Status: approved for implementation. P4.1d activation is out of scope.

## Boundary

The engine is an internal shared Rust library that accepts explicit `sourceRoot`
and `targetRoot` paths. It is not registered as a Tauri command, daemon RPC,
startup hook, or user-facing action. The source remains read-only. The active
target remains untouched. Prepared data is written only to an engine-owned
sibling staging directory.

The current legacy identifier, active data directory, and daemonctl lookup stay
unchanged. The target DeanX identity remains inactive. Real user data is never
used by the P4.1c test suite.

## Authority and data flow

`release-identity.json.configSchemaVersion` is the configuration-schema
authority. It is independent from `VERSION.json.version` and
`VERSION.json.build`.

The deterministic flow is:

1. inspect the explicit roots without mutation;
2. parse `settings.json` and `workspaces.json` with pure read-only readers;
3. rebuild settings and workspaces from their typed, allowlisted schemas;
4. omit credentials before any output serialization;
5. write sanitized migration and backup artifacts to staging;
6. validate the staged schema, identity metadata, paths, and forbidden data;
7. mark the staging set `READY_FOR_ACTIVATION` without activating it.

`auth.json`, CODEX_HOME credentials, canonical sessions/threads/rollouts,
RemoteHostIdentity values, runtime generations, pending mutations, locks, PIDs,
caches, and temporary artifacts are never opened as migration inputs.

## Settings whitelist

The allowlist is the current typed `AppSettings` schema. Unknown raw fields are
reported by path only and excluded. `remoteBackendToken` and every
`remoteBackends[].token` are set to absent before serialization. Safe backend
endpoint metadata remains. `remoteBackends[].remoteHostIdentity` and the
standalone `remote-host-identity.json` value are deferred to P4.1d and are not
serialized. The preview warns that migrated remote connections require
reauthentication.

An optional migration-only `requiredMigrationFields` array declares source
fields required for correctness. A declared field outside the allowlist blocks
preflight instead of being guessed or copied.

## Workspace whitelist

Workspaces use the current typed `WorkspaceEntry` schema. IDs, names, paths,
kind, parent/worktree metadata, and explicit workspace settings are preserved.
Paths must be non-empty and valid strings. Duplicate IDs, malformed schema,
and non-empty target conflicts fail closed. The engine never guesses identity,
merges workspaces, or deletes the source.

## Preview and state

The sanitized preview reports roots, source/target schema versions, category
classifications, conflicts, warnings, excluded field paths, credential-category
counts, HostIdentity presence, and estimated actions. It contains no values from
excluded fields, credential material, raw identity, or private thread content.

States are `NOT_STARTED`, `PREFLIGHTED`, `STAGING`, `VALIDATED`,
`READY_FOR_ACTIVATION`, and `FAILED`. `ACTIVATED` does not exist in P4.1c.

An engine marker binds staging ownership to the explicit target and schema.
Interrupted owned staging is rebuilt or revalidated. Unowned staging is never
removed. Rollback deletes only a matching engine-owned staging directory.
Sanitized backup files are produced by the same allowlist serializers as the
prepared files; no full-source backup is created.

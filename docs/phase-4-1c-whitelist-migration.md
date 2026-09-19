# P4.1c — Controlled Whitelist Migration Engine

Status: **PASS / COMPLETE / FROZEN**. The deterministic preparation engine is
complete; activation remains exclusively P4.1d and has not started.

## Authority and boundary

The migration core is `src-tauri/src/shared/migration_core.rs`. It is a pure
internal Rust library with explicit `sourceRoot` and `targetRoot` inputs. It has
no Tauri command, daemon RPC, startup hook, or UI entry point. Tests use only
temporary synthetic roots. P4.1c reads no real settings, workspace store,
credential, CODEX_HOME, RemoteHostIdentity value, or daemon state.

`release-identity.json.configSchemaVersion` is the configuration-schema
authority. It is independent from software version and Build in `VERSION.json`.
Build changes never imply a configuration-schema change.

## Migration matrix

| Data | Disposition | Contract |
| --- | --- | --- |
| Allowlisted settings fields | MIGRATE | Rebuilt from a fixed raw-input DTO; later typed-schema growth never expands eligibility automatically. |
| Remote endpoint/provider/target metadata | MIGRATE | Safe metadata remains; authentication must be repeated. |
| `remoteBackendToken`, target token, auth/secret/API-key fields | EXCLUDE | Removed before serialization; never enters staging, backup, report, or errors. |
| Allowlisted workspace fields and settings | MIGRATE | IDs and explicit metadata preserved; absolute paths and unique IDs validated. |
| Unknown settings/workspace fields | EXCLUDE | Path-only preview evidence; values never copied or reported. |
| Declared unknown required fields | PRECHECK_BLOCKED | `requiredMigrationFields` names outside the allowlist fail closed. |
| `auth.json`, CODEX_HOME credentials | EXCLUDE | Classified by path/name and never opened as migration input. |
| Canonical sessions, threads, rollouts | EXCLUDE | No canonical content is read, copied, or reported. |
| `remote-host-identity.json` | DEFER_TO_P4.1d | Preview records presence only; the raw identity is never read or copied. |
| PID/lock/generation/pending/cache/socket/temp data | EXCLUDE | Runtime truth is re-established by a future activated instance. |
| Unrecognized root artifacts | UNKNOWN | Reported by filename only and never copied. |

## Preview and prepared artifacts

`inspect_migration` is mutation-free. It reports explicit roots, detected
source, schema versions, categorized actions, field paths excluded, conflicts,
warnings, credential-field count, HostIdentity presence, and estimated files
and actions. It never includes excluded values, tokens, auth material, raw
identity, or private Thread content.

The state machine is:

```text
NOT_STARTED → PREFLIGHTED → STAGING → VALIDATED → READY_FOR_ACTIVATION
                                      └────────→ FAILED
```

There is no `ACTIVATED` state. Output is written only to a deterministic
engine-owned sibling staging directory. The active target remains untouched.
The marker binds ownership to the target path and configuration schema.

Staging contains allowlisted `settings.json`, `workspaces.json`, a target/schema
manifest, sanitized report, state, and a sanitized backup produced by the same
serializers. It contains no whole-source backup. Validation reparses the staged
schemas, validates workspace paths and IDs, checks target identity/schema, and
rejects credential, RemoteHostIdentity, runtime-generation, pending-mutation,
canonical rollout, and auth artifacts.

Incomplete owned staging is deterministically rebuilt on the next run. Unowned
staging is never removed. Rollback deletes only a marker-matched staging tree;
it never writes to the source. `READY_FOR_ACTIVATION` remains prepared data and
does not change identifiers, data directories, daemon lookup, or runtime state.

## Security fixtures and evidence

The fixture family is `docs/fixtures/phase-4-1c-whitelist-migration/`. It
includes explicit fake secret sentinels. Focused tests assert zero occurrences
of all sentinels in staging, backup, reports, and state. The source fixture may
contain the sentinels solely to prove exclusion.

The RED suite failed in the real Rust file-module path before the engine
existed. The initial GREEN focused run passed 38/38; the final expanded focused
suite passed 45/45. Final closeout evidence is recorded in
`docs/evidence/phase-4-1c/README.md`.

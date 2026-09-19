# P4.1d-1 — Migration / Activation Safety Foundation

Status: **PASS / COMPLETE / FROZEN**. P4.1d remains **IN PROGRESS** and P4.1d-2
is not started. This foundation is not registered with the App, daemon,
daemonctl, Tauri commands, RPC, or the current startup path.

## Bootstrap and schema authority

`activation_foundation.rs` supplies a read-only classifier and explicit load
permission: fresh and legacy profiles are activation-only; a complete valid
activated profile may load normally; recovery-required, conflicting, and
corrupt profiles fail closed. Inspection never generates defaults or a host
identity. Fresh activation orders preparation and validation before the
activation marker and target commit.

The migration source recognizes `LegacyV0` independently from target schema
1. A missing source manifest is accepted only by the explicit legacy rule;
explicit version 0 is supported, while malformed and future versions fail.
Settings and workspace output is built from a fixed raw-input allowlist. New
fields added to Rust application types do not become migration-eligible. Token,
pin, command/argument, launch-script, CODEX_HOME, auth, canonical Thread, and
runtime-generation material is excluded before serialization. Imported Remote
targets remain local/inactive and require later pin and credential confirmation.

## Paths and activation recovery

Source, target, and staging roots are canonically bound. Equal, ancestor,
descendant, alias, and Windows reparse-point roots are rejected. The staging
marker binds source/target, source fingerprints, file hashes, schema, and an
activation-owned cleanup policy. Source or staged-content changes after preview
invalidate activation.

The activation journal binds transaction ID and canonical roots. The activation
manifest and prepared v2 identity must carry that same transaction ID. Recovery
uses journal state together with the actual legacy identity, protected staging
recovery material, and the target manifest/files. It handles interruption after
each file-system operation and before its following journal advance. In
particular, a retired legacy identity with a still-`prepared` journal continues
from protected recovery material; generic P4.1c cleanup is forbidden from
deleting it. A target already committed but not journaled is validated rather
than migrated again, so no new UUID is generated.

This is a recoverable multi-file protocol, not one atomic transaction. Tests
cover process interruption. `ReplaceFileW` provides the tested single-file
replacement guarantee. Sudden power-loss/durability behavior is not proven.

## RemoteHostIdentity compatibility and retirement

The compatibility target is the v1 loader at baseline commit
`ea79bce9d86b1b91c0afea6ff726d94685a07986`, not every historical release. It
accepts valid v1 and rejects v2 `active`, v2 `retired`, corrupt, or read-blocked
stores without rewriting the file or generating a UUID. This proves identity
loading/daemon initialization is blocked; it does not prove whether every old
UI binary can open without its daemon.

On Windows, retirement holds the exact legacy file with read access and only
`FILE_SHARE_DELETE`, so another legacy reader is denied. `ReplaceFileW` then
replaces the v1 file with the prepared v2 retired store without a missing-path
window. Native temporary-file and child-process tests prove successful replace,
blocked legacy reads during protection, continued rejection after release, and
failure preservation when replacement sharing prevents the operation. No
delete-then-write fallback exists.

Retirement is the sole designed write to the legacy source, but P4.1d-1 invokes
it only in fake temporary roots. A future real cutover requires separate
authorization for the exact root/file/effect and confirmed old-process stop.

## Coordination limits

The activation commit lock allows one local commit per target. The service
lifetime lock excludes a second new service for the same controlled profile
and is proven with a real child process. These are local filesystem mutexes,
not writer/client ownership or a distributed lease. They cannot constrain an
old binary that ignores them, nor prevent manually copied identity material in
another root. Unknown/running old-process state blocks preparation.

Current legacy identifiers, product name, data directory, daemon lookup, and
startup behavior remain unchanged. Real user data reads, migrations, identity
retirements, and startup cutovers: **0**.

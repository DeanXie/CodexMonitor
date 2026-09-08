# Phase 3.5.1b — RemoteHostIdentity

Status: **PASS / FROZEN**.

Next development start point: **Phase 3.5.1c — Host Availability: GO / NOT STARTED**.

## Frozen identity contract

`RemoteHostIdentity` is a canonical lowercase, non-nil UUID v4. It is stable, opaque, non-secret, and non-authorizing.

It is distinct from the authentication token, TCP endpoint, display name, PID, `CodexHomeIdentity`, `WorkspaceKey`, and canonical Thread identity. Canonical Thread identity remains unchanged:

```text
CodexThreadKey = (codexHomeIdentity, fullThreadId)
```

Cross-host routing uses:

```text
RemoteThreadLocator {
  remoteHostIdentity,
  threadKey: CodexThreadKey
}
```

The host namespace is not embedded in `CodexThreadKey`.

## Persistence and recovery

The daemon stores its identity at:

```text
<daemon data-dir>/remote-host-identity.json
```

- A missing store is initialized once with a new identity and persisted using an initialization lock plus temporary-file write, file sync, atomic rename, and supported parent-directory sync.
- A valid store is reused across daemon and Desktop UI restarts.
- Malformed JSON, unsupported schema, nil UUID, non-v4 UUID, non-canonical UUID, and read failures fail closed.
- Invalid or unreadable persisted identity never causes silent rotation.

Copying the complete daemon data directory preserves the same logical `RemoteHostIdentity`. Running simultaneous clones with the same persisted identity is an unsupported operational condition; this slice does not add hardware fingerprinting or clone detection.

## Authenticated handshake and TOFU

The remote connection becomes routable only after:

```text
TCP
→ token authentication
→ daemon_info
→ daemon name/mode validation
→ protocolVersion validation
→ RemoteHostIdentity validation
→ configured target pin validation
→ READY
```

`protocolVersion = 1` is the remote protocol version and is independent of the application version. Authentication proves authorization to connect; it does not define Host identity.

For an unpinned legacy target, an authenticated valid handshake learns the identity once and atomically persists it. Subsequent matching identities are accepted. A mismatch fails closed and never overwrites the stored pin. Endpoint or display-name changes do not change Host identity. No rebind/reset UI is included.

## Execution environment

Remote daemon sessions use a host-qualified `ExecutionEnvironmentKey`:

```text
remote:<canonical-uuid>
```

The same daemon data directory retains the same environment key across restarts, while different Host identities produce different environment keys. Existing local `monitor-local-windows` and `monitor-local-posix` semantics are unchanged.

## Capability boundaries

- Availability remains dynamic state and is not part of `RemoteHostIdentity`; Phase 3.5.1c owns that contract.
- The current daemon has one effective `CODEX_HOME`. The identity model allows multiple `CodexHomeIdentity` values beneath one Host, but multi-`CODEX_HOME` runtime support is not implemented by this slice.
- Writer lifecycle, approval, host-authoritative deletion tombstones, Mobile projections, client/device identity, and Phase 3.5.2+ are outside this frozen slice.

Implementation commit: `43387e0` (`feat: add stable remote host identity`).

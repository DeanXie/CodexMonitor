# Phase 3.5.1c — Host Availability

Status: **PASS / FROZEN**.

Phase 3.5.1d Windows Remote Acceptance is **IN PROGRESS**. Gate C is **PASS** after the bounded remote backend connection-ownership correction. Acceptance resumes at Gate D.

## Frozen availability contract

`RemoteHostIdentity` is stable identity. `RemoteHostAvailabilitySnapshot` is process-local, dynamic observation. They are independent.

Availability has four orthogonal layers:

```text
Transport
Auth
Daemon
Runtime { workspaceId }
```

The deterministic summary is `READY` only when the current attempt simultaneously proves:

```text
Transport = CONNECTED
Auth = AUTHENTICATED
Daemon = AVAILABLE
Runtime = READY
```

`lastSuccessfulHandshakeAt` and `lastRuntimeReadyAt` are history only. They never restore current readiness.

## Typed failure semantics

- TCP connect failure produces `ENDPOINT_UNREACHABLE`. The contract does not claim whether the Host is powered off, sleeping, blocked by a firewall, unavailable through the route, missing the daemon, or configured with the wrong endpoint. Lower-level errors remain diagnostics.
- Authoritative authentication rejection produces `AUTH FAILED`; authentication timeout or disconnect produces `AUTH UNKNOWN`.
- `daemon_info` timeout or disconnect produces `DAEMON UNKNOWN`.
- Unsupported remote protocol produces `PROTOCOL_UNSUPPORTED`.
- Host identity or configured pin mismatch produces `IDENTITY_MISMATCH`.
- Authoritative runtime failure produces `RUNTIME UNAVAILABLE`; runtime timeout or disconnect produces `RUNTIME UNKNOWN`.

Availability is derived from typed transport, RPC, validation, and runtime outcomes. Error-message parsing is not an authority.

## Runtime readiness

Runtime readiness is workspace-scoped and requires the complete Host runtime gate:

```text
connect_workspace
→ WorkspaceSession liveness validation
→ app-server initialize/initialized
→ Runtime READY
```

Workspace inventory connection flags, `daemon_info`, or `codex_doctor` alone cannot establish Runtime readiness. A cached `WorkspaceSession` is revalidated; a stale process/session cannot return an immediate false success. This does not change writer ownership, WorkspaceSession release lifecycle, or the active-writer contract.

## Attempts, disconnects, and settings changes

Each connection attempt has a monotonically increasing `attemptId`/generation. Observations from an older attempt cannot overwrite a newer attempt.

EOF, write failure, or channel disconnect invalidates current readiness and records Transport `DISCONNECTED`. Reconnect starts a new attempt and repeats TCP, authentication, daemon/protocol/identity/pin validation, and the workspace runtime gate before returning to `READY`.

Endpoint, token, provider, or active-target changes invalidate the prior attempt without inventing `ENDPOINT_UNREACHABLE`, `DISCONNECTED`, or `AUTH_FAILED`. The configured `RemoteHostIdentity` pin is preserved and is never silently cleared or rewritten.

## Thread and domain authority boundaries

Host unavailable does not imply Thread `ABSENT`. Authentication failure is not endpoint unreachability. Daemon availability is not Runtime readiness.

The availability core does not write or infer:

- Global Source Registry state;
- deletion tombstones;
- Surface Projection state;
- Workspace or Desktop Project identity;
- token accounting.

When remote inventory never executes, there is no projection observation. Transport, authentication, daemon, or runtime failure during exact read remains unavailable/unknown and never becomes Thread absence, deletion, a tombstone, or a complete empty inventory. An authoritative exact API success returning not-found retains its existing separate contract.

## Lifecycle and scope

Availability state is intentionally process-local. Stable Host identity persists separately. Phase 3.5.1c does not redesign writer lifecycle, approval, deletion authority, Mobile projection, client identity, or multi-`CODEX_HOME` runtime behavior.

Implementation commit: `bc2f6f0` (`feat: model remote host availability`).

The availability reducer and remote backend cache now share the same attempt/generation ownership rule. A stale handshake cannot replace the current cached backend, a stale client cannot clear it, and stale EOF cannot mark the current attempt disconnected. A disconnect from the current cached connection still invalidates current availability. These rules preserve the frozen availability state model.

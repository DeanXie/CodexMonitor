# Phase 3.5.2d — Remote Transport Coordination

Status: Phase 3.5.2d.1 is **PASS / COMPLETE / FROZEN**. Later Phase 3.5.2d
slices are not started.

## Authority boundary

Phase 3.5.2d.1 adds process-local transport correlation only. It does not add
a stable Remote-client identity. Authentication tokens remain credentials, not
identity. A Remote TCP transport is not a WorkspaceSession, app-server
connection, writer owner, subscription owner, lease, or takeover authority.

The existing `RemoteBackendCacheState.generation` remains a cache invalidation
counter. It prevents a stale initializer or disconnected cached backend from
replacing or clearing the current cache entry. It is not retained as request
provenance and is not reused as a transport generation.

## Remote transport generation

`RemoteTransportGeneration` is an opaque UUID-backed value. It is minted only
for a transport that has completed authentication, stays stable for that
transport, and is replaced on reconnect. It is non-secret, comparable, local
to the process, and scoped to one connection. Values from the App and daemon
endpoints are not a cross-process client identity and are not required to be
equal.

An unauthenticated `RemoteBackend` has no authoritative transport generation.
The daemon creates its connection provenance after successful token auth (or
immediately for an explicitly tokenless authenticated configuration).

## Request provenance

The request correlation key is:

```text
(
  RemoteTransportGeneration,
  transportRequestId
)
```

Therefore request ID `1` on one connection cannot collide with request ID `1`
after reconnect or on a second simultaneous connection. The daemon rejects a
duplicate compound key on the same authenticated transport instead of
dispatching it again.

`RemoteRequestProvenance` serializes only:

- the compound request key;
- method;
- received timestamp;
- current dispatch state;
- optional dispatch/session-attempt/response/transport-loss timestamps; and
- optional `SessionAttemptProvenance` containing Workspace ID,
  WorkspaceSession generation, and attempt ID.

It stores no token, request payload, writer owner, subscription owner,
Remote-client owner, lease, or takeover information. Session attempt IDs are
correlation evidence only.

## Dispatch state model

The transport-only states are:

```text
received
dispatch_started
session_attempt_bound
response_observed
transport_lost
```

These states do not encode writer admission, subscription, or runtime facts.
A disconnect before dispatch retains no session attempt. A disconnect after
dispatch preserves any bound session attempt and records that the transport
was lost. A detached daemon task may still finish, and a later directly
observed response or app-server outcome remains stronger evidence; transport
loss cannot erase that evidence.

## Reconnect and stale evidence

Reconnect creates a new generation and a new provenance map. Old-generation
keys cannot mutate or complete new-generation requests, even when transport
request IDs match. Old provenance is historical evidence only.

The model does not replay mutations after reconnect. `resume_thread` and
`thread_upstream_unsubscribe` remain excluded from automatic retry. A later
user action is a new explicit intent with a new transport provenance record,
not replay of the old request.

Notification generation gating remains deferred to Phase 3.5.2d.3. The
connection-scoped generation authority introduced here is sufficient to label
the source transport when that gate is implemented; this slice does not change
event routing.

## Generation separation

The three generation types solve different problems:

| Generation | Scope | Purpose | Must not imply |
| --- | --- | --- | --- |
| `RemoteTransportGeneration` | One authenticated Remote TCP connection in one process | Request/response correlation, reconnect isolation, stale transport evidence | Remote-client identity or any ownership |
| `WorkspaceSessionGeneration` | One WorkspaceSession/app-server process lifetime | Writer-admission evidence and session replacement isolation | TCP connection identity |
| `AppServerConnectionGeneration` | One app-server protocol connection within a WorkspaceSession | Subscription and runtime-observation isolation | Remote transport or subscriber ownership |

None may substitute for another. Multiple Remote transports may share the same
daemon and WorkspaceSession truth while retaining independent request
provenance.

## Slice boundary

Phase 3.5.2d.1 changes no resume/unsubscribe business semantics, writer or
subscription transitions, runtime transitions, notification delivery gates,
or daemon restart recovery. It adds no public UI API and performs no real
writer mutation or upstream unsubscribe capture.

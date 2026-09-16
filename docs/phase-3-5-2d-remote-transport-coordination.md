# Phase 3.5.2d — Remote Transport Coordination

Status: Phase 3.5.2d.1 through Phase 3.5.2d.4 are
**PASS / COMPLETE / FROZEN**.

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
- optional `SessionAttemptProvenance` containing attempt kind, Workspace ID,
  WorkspaceSession generation, `CodexThreadKey`, and the existing session
  attempt ID; upstream unsubscribe correlation also records the app-server
  connection generation.

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

The pre-dispatch liveness transition is atomic only with respect to the
transport provenance record. It is not a transaction around the shared
session mutation. Once dispatch starts, a later transport loss cannot cancel
the daemon task. If the shared session creates an attempt or crosses its
upstream dispatch boundary, that direct session evidence remains authoritative
even when the caller cannot observe the response.

## Dispatch correlation

Phase 3.5.2d.2 passes an optional daemon-only request context through the RPC
adapter for exactly two explicit mutation methods:

```text
resume_thread
thread_upstream_unsubscribe
```

The shared `WorkspaceSession` creates the real writer-admission or unsubscribe
attempt first, then binds that existing ID to the Remote request provenance.
No second business attempt ID is minted. App-local calls use the same shared
business paths without a transport context.

Resume correlation records the Remote compound request key, WorkspaceSession
generation, `CodexThreadKey`, and writer-admission attempt ID. Upstream
unsubscribe correlation additionally records the app-server connection
generation and unsubscribe attempt ID. These links answer only which transport
request triggered which shared attempt. They do not identify a person or
client and do not establish writer or subscriber ownership.

The daemon checks transport liveness immediately before entering RPC dispatch.
When loss is already recorded, the task stops before session attempt creation
and upstream mutation. When loss follows dispatch, the shared task continues;
direct writer/subscription results are not replaced by transport-level
unknown evidence.

Two authenticated transports may reuse the same numeric request ID because
their compound keys differ. Concurrent resume requests receive distinct
writer-admission attempts and upstream remains the admission authority.
Concurrent unsubscribe requests retain distinct transport records, while the
existing shared pending gate permits at most one unsubscribe attempt and one
upstream dispatch. A duplicate request ID on the same transport is rejected
before a second daemon task or session attempt is created.

## Reconnect and stale evidence

Reconnect creates a new generation and a new provenance map. Old-generation
keys cannot mutate or complete new-generation requests, even when transport
request IDs match. Old provenance is historical evidence only.

The model does not replay mutations after reconnect. `resume_thread` and
`thread_upstream_unsubscribe` remain excluded from automatic retry. A later
user action is a new explicit intent with a new transport provenance record,
not replay of the old request.

## Notification delivery isolation

Phase 3.5.2d.3 binds each Remote reader to the opaque generation minted after
that transport authenticates. Notification publication is disabled until the
backend completes `daemon_info`, service/mode/protocol validation, the
`RemoteHostIdentity` check, and current-cache publication. The generation gate
is checked immediately before the Tauri event hub for `app-server-event`,
`terminal-output`, and `terminal-exit`.

Only the generation published as current may emit those events. Replacing or
clearing the current backend changes the delivery authority used by all
overlapping readers, so late notifications from an older reader are dropped.
Response correlation remains connection-local and unchanged.

Old-connection EOF, read/write failure, and disconnect handling still drains
only that connection's pending map. Existing cache pointer checks and
availability-attempt checks prevent stale teardown from clearing or degrading
the current backend or availability snapshot. These transport events do not
change WorkspaceSession, writer-admission, subscription, runtime-availability,
or canonical Thread truth, and they do not trigger retry or replay.

## Daemon restart and session re-establishment

Phase 3.5.2d.4 gives each daemon process lifetime an opaque, non-persisted
`DaemonProcessGeneration`. Authenticated `daemon_info` returns this generation
alongside the persisted `RemoteHostIdentity`. A reconnect that observes the
same generation is a reconnect to the same daemon process; a different
generation on the same pinned host is a daemon restart. If either side lacks
process-generation evidence, process continuity remains `UNKNOWN`.

`RemoteHostIdentity` continues to prove only the pinned execution host. It is
not a daemon-process, transport, WorkspaceSession, app-server, writer, or
subscription continuity token. A changed Host identity still fails closed and
is never accepted as an automatic rotation.

Each new daemon process starts with an empty in-memory sessions map. Persisted
Workspace route metadata may be loaded, but `route exists` does not imply
`WorkspaceSession exists` or runtime readiness. An explicit
`connect_workspace` establishes a new shared `WorkspaceSession` and app-server
connection. Concurrent callers retain the existing single-flight behavior and
share one re-established session.

The new WorkspaceSession generations do not copy old current observations.
For the same `CodexThreadKey`, writer admission starts at `NOT_OBSERVED`,
subscription starts at `NOT_OBSERVED`, and runtime availability starts at
`UNKNOWN`. Old-generation observations remain historical evidence only. A
daemon restart does not delete a canonical Thread, mark a writer free, mark a
subscription released, or infer runtime `NOT_LOADED`.

An in-flight resume or upstream unsubscribe whose response was not observed
when the old daemon died remains unknown to its caller. It is not replayed,
retried, or rebound to the new daemon or WorkspaceSession generation. Late
old-transport responses, notifications, teardown, and request provenance are
still excluded by the d.1-d.3 generation gates and cannot mutate new-daemon
current state.

## Generation separation

The four generation types solve different problems:

| Generation | Scope | Purpose | Must not imply |
| --- | --- | --- | --- |
| `RemoteTransportGeneration` | One authenticated Remote TCP connection in one process | Request/response correlation, reconnect isolation, stale transport evidence | Remote-client identity or any ownership |
| `DaemonProcessGeneration` | One daemon process lifetime | Distinguish same-process reconnect from same-host daemon restart | Host identity, transport identity, session continuity, or ownership |
| `WorkspaceSessionGeneration` | One WorkspaceSession/app-server process lifetime | Writer-admission evidence and session replacement isolation | TCP connection identity |
| `AppServerConnectionGeneration` | One app-server protocol connection within a WorkspaceSession | Subscription and runtime-observation isolation | Remote transport or subscriber ownership |

None may substitute for another. Multiple Remote transports may share the same
daemon and WorkspaceSession truth while retaining independent request
provenance.

## Slice boundary

Phase 3.5.2d.1-d.4 change no resume/unsubscribe business response schema,
writer or subscription transitions, runtime transitions, retry/replay policy,
or canonical Thread authority. They add no public UI API and perform no real
writer mutation or upstream unsubscribe capture. Phase 3.5.2d.4 freezes daemon
restart detection and session re-establishment; final compatibility closeout
remains Phase 3.5.2d.5.

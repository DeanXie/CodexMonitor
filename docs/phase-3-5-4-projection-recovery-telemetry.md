# Phase 3.5.4 — Projection / Recovery / Telemetry

Status: Phase 3.5.4a Projection Freshness Authority and Phase 3.5.4b Generation-tagged Event Delivery are **PASS / COMPLETE / FROZEN**. Phase 3.5.4 remains **IN PROGRESS**. Phase 3.5.4c is not started.

## Projection freshness authority

`ProjectionFreshness` is process-local, read-only evidence describing whether a specific projection coverage has been authoritatively hydrated for the current generation vector. It is not canonical Thread existence, recovery orchestration, availability, ownership, a lease, or a mutation authority.

The stable status vocabulary is:

- `not_hydrated`: no authoritative evidence for this coverage and current generation;
- `hydrating`: an authoritative read is in flight;
- `current`: authoritative evidence matches the required current generations;
- `stale`: retained evidence belongs to an older required generation;
- `unavailable`: the authoritative read/session boundary was unavailable;
- `unknown`: an outcome exists but cannot be classified more precisely.

Coverage is explicit and independent:

- `workspace_catalog`;
- `thread_catalog`;
- `thread_detail`;
- `observation_snapshot`.

`thread/list` can make only `thread_catalog` current. Exact `thread/read` can make only that `thread_detail` current. A shared observation query can make only `observation_snapshot` current. `list_workspaces` can make only `workspace_catalog` current. Partial hydration is valid.

## Generation vector

The model reuses, without aliasing or replacement:

```text
DaemonProcessGeneration
RemoteTransportGeneration
WorkspaceSessionGeneration
AppServerConnectionGeneration
```

No recovery, projection, or client generation is introduced. A daemon-process change prevents old evidence from being current even when `RemoteHostIdentity` is unchanged. A WorkspaceSession generation change invalidates session-scoped coverage. An app-server connection generation change invalidates connection-scoped coverage. Old evidence remains historical and reports `stale` with its original generation vector.

Remote transport reconnect alone does not imply Thread absence. Session-direct evidence that is not transport-dependent may remain current when the WorkspaceSession and app-server generations survive. Transport-tagged evidence cannot become current under a different transport generation.

## Source and time semantics

The stable source vocabulary is:

- `workspace_list`;
- `thread_list`;
- `thread_read`;
- `observation_query`;
- `event`.

Direct authoritative reads outrank event evidence. `observedAt` and `hydratedAt` are diagnostic timestamps and tie-breakers only; they cannot override generation mismatch or direct-evidence authority.

Frontend reload, empty local state, no event, disconnect, or reconnect cannot establish absent, deleted, idle, free, released, no approval, or any mutation outcome.

## Shared query contract

App and daemon expose the same serialized `get_projection_freshness` read contract. Inputs are `workspaceId` plus optional exact `threadId`; the shared core normalizes the exact Thread to `CodexThreadKey` using the current WorkspaceSession. The query returns current-generation coverage snapshots and performs no connect, resume, subscribe, approval, delete, retry, replay, or other mutation.

Missing Workspace and unavailable WorkspaceSession are distinct. An unavailable session reports unavailable thread-scoped coverage and never fabricates `not_hydrated`, absence, or canonical Thread truth.

The runtime is process-local and not persisted. A daemon restart creates a new `DaemonProcessGeneration`, an empty freshness runtime, and no inherited current evidence.

## Generation-tagged event delivery

Every `app-server-event` carries the exact `WorkspaceSessionGeneration` and
`AppServerConnectionGeneration` that produced the message. A Remote delivery
also carries the current `DaemonProcessGeneration` and the actual
`RemoteTransportGeneration` that delivered it. The shared event never stores a
transport generation: the daemon binds that transport-scoped provenance at the
Remote delivery edge. A local App delivery leaves both Remote-only generation
fields absent.

The frontend event hub admits an event only when all required generation fields
are present and exactly match the already-hydrated current `thread_catalog`
generation context. Another current coverage cannot bypass a not-hydrated or
stale catalog.
Remote events must also match the current daemon-process and transport
generations. Missing or stale generation evidence is dropped before any
subscriber/reducer fanout. A frontend reload starts without this authoritative
context and therefore rejects events until a read-only freshness snapshot
establishes it; events are not buffered or replayed.

An admitted event may incrementally update a projection whose coverage is
already `current`. It cannot promote `not_hydrated` or `stale` coverage to
`current`, complete a broad catalog hydration, or outrank direct read evidence.
Rejected events do not mutate canonical Thread state or shared writer,
subscription, runtime, approval, or delete authority. Transport reconnect may
invalidate old transport delivery while leaving surviving WorkspaceSession
truth unchanged.

The envelope adds no event/projection/recovery/client generation. It provides
no sequence, gap detection, deduplication, replay buffer, client identity,
owner, lease, or completeness proof. The same payload under another generation
has distinct provenance. Multiple Remote clients can receive the same shared
session event while each delivery is bound to its own transport generation.

## Frozen boundaries

Phase 3.5.4a-b add no UI, recovery orchestration, persistence, polling loop,
automatic retry/replay, client identity, owner, lease, `FREE`, `AVAILABLE`, or
`RELEASED` semantics. Phase 3.5.4c owns later work.

Compatibility fixtures are stored under `docs/fixtures/projection-freshness/`
and `docs/fixtures/generation-tagged-events/`; implementation evidence is
indexed under `docs/evidence/phase-3-5-4a/` and
`docs/evidence/phase-3-5-4b/`.

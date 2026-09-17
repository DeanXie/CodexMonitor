# Phase 3.5.4 — Projection / Recovery / Telemetry

Status: Phase 3.5.4a Projection Freshness Authority, Phase 3.5.4b Generation-tagged Event Delivery, Phase 3.5.4c Authoritative Recovery / Hydration, Phase 3.5.4d Offline / Stale UI & Multi-client Isolation, and Phase 3.5.4e Telemetry / Compatibility / Docs Closeout are **PASS / COMPLETE / FROZEN**. Phase 3.5.4 is **PASS / COMPLETE / FROZEN**.

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

## Authoritative recovery and hydration

The recovery chain is `list_workspaces`, conditional `connect_workspace`,
authoritative `thread/list`, optional exact selected `thread/read`, a read-only
observation snapshot, and then an existing safe local live attach when a caller
needs one. `connect_workspace` is used only when the listed WorkspaceSession is
not connected. It establishes session authority; it does not resume a Thread,
replay a Turn, or acquire writer authority.

Each authoritative read advances only its own coverage. Workspace listing can
advance `workspace_catalog`; Thread listing can advance `thread_catalog`; an
exact Thread read can advance that `thread_detail`; and the aggregate
observation query can advance only `observation_snapshot`. A failure therefore
leaves already completed coverages valid and classifies only the failed
coverage as `unavailable` or `unknown` according to the observed failure.

The App and daemon expose one `get_authoritative_observation_snapshot` contract
implemented by the shared core. Its payload aggregates read-only snapshots for
writer admission, subscription, runtime availability, pending and historical
approvals, approval-decision attempts, and delete observation. Those fields
remain independent authority models; aggregation creates no combined business
state and no inference from missing data. The query never connects a workspace
or dispatches a mutation.

Frontend apply gates require the matching coverage to be `current`. Observation
results must additionally match their returned WorkspaceSession and app-server
connection generations. A generation change while a read is in flight leaves
the old result stale/historical, so it cannot mutate the replacement generation.
Reload, reconnect, WorkspaceSession replacement, and daemon restart converge by
re-reading authority. They never clear cached data into an asserted absence and
never replay `thread/resume`, approval decisions, Thread delete, or upstream
unsubscribe.

Recovery triggers share a per-workspace single flight within one frontend
coordinator. Separate Remote frontends retain independent caches and may each
perform read-only hydration against the same shared WorkspaceSession truth;
there is no recovery owner, lease, or Remote client identity. Events remain
incremental only: they cannot promote unhydrated/stale coverage or replace the
authoritative baseline.

## Offline and stale UI

The Remote shell derives one read-only view model from `ProjectionFreshness`.
Its compact primary vocabulary is `Current`, `Hydrating`, `Stale`,
`Unavailable`, and `Unknown`; `not_hydrated` maps to `Unknown`. The four
coverage values remain independent and are preserved in the status details, so
a current catalog does not hide stale detail or unavailable observations.
Transport, authentication, daemon, and Workspace runtime availability remain
secondary diagnostics. The legacy `Live`, `Polling`, and `Disconnected` value
is delivery-mode telemetry only and never defines projection freshness.

A daemon broadcast lag emits one ephemeral `app-server-event-gap` marker bound
to the current daemon-process and Remote transport generations. It contains a
skipped count and affected coverages, but no persisted sequence, canonical
version, business revision, replay ledger, or shared authority mutation. Only
the matching frontend derives those coverages as stale and invokes the existing
per-workspace single-flight authoritative recovery. A current authoritative
snapshot for the same generations and a hydration time after the gap clears the
local marker. Another Remote client is unaffected unless it independently
observes a gap; clients may temporarily diverge and converge through reads.

Reconnect retains cached content as stale while hydration runs. A new daemon
process cannot inherit current evidence even when `RemoteHostIdentity` is the
same. Workspace unavailability remains distinct from transport disconnection.
Insufficient evidence remains `Unknown`; no UI state claims Thread absence.

Approval controls are actionable only when `observation_snapshot` is current
and the exact generation-bound approval identity is still `pending`. Delete
outcome unknown does not render deleted, while a confirmed-delete projection is
not resurrected by a stale catalog. These UI gates do not change approval or
delete authority.

## Frozen boundaries

Phase 3.5.4a-e add no telemetry persistence, generic polling loop, automatic
mutation retry/replay, client identity, owner, lease, `FREE`, `AVAILABLE`, or
`RELEASED` semantics. Gap evidence never becomes canonical truth and never
dispatches resume, approval, delete, or unsubscribe mutations.

## Telemetry classification and persistence

The classification is stable:

- authoritative state: `RemoteHostIdentity`, current WorkspaceSession and
  app-server connection generations, and the independent shared approval and
  delete observations;
- diagnostic telemetry: daemon-process/transport generations,
  `RemoteRequestProvenance`, hydration progress, stale-event drops, event gaps,
  recovery observations, and layered availability;
- historical evidence: old-generation events, stale projection snapshots, and
  completed hydration evidence retained with their original generation;
- UI projection: `ProjectionFreshness`, per-coverage display status, and
  availability diagnostics.

Diagnostic telemetry never becomes business authority. There is no telemetry
database, persisted freshness state, persisted gap ledger, durable event
queue, or event-sourcing subsystem. Existing ephemeral counters remain
diagnostic only.

## Authority precedence

The global precedence is:

```text
current-generation direct upstream evidence
> current shared-session observation
> recovered authoritative read
> generation-tagged historical evidence
> stale UI cache/projection
```

Timestamps are diagnostic tie-breakers and cannot reverse this order. A newer
arrival time cannot make stale-generation evidence canonical. The same
`RemoteHostIdentity` cannot prove daemon, transport, WorkspaceSession, or
app-server connection continuity.

## Event-stream completeness boundary

Daemon broadcast/event-stream completeness remains **NOT PROVEN**. There is no
durable sequence ledger or replay log, so absence of a detected gap is not
proof that every event was delivered. A known `app-server-event-gap` safely
marks only affected projection coverage stale and triggers authoritative
hydration; it does not mark shared authority stale or manufacture absence.

Compatibility fixtures are stored under `docs/fixtures/projection-freshness/`
and `docs/fixtures/generation-tagged-events/`; implementation evidence is
indexed under `docs/evidence/phase-3-5-4a/` and
`docs/evidence/phase-3-5-4b/`. Phase 3.5.4c sanitized recovery fixtures live
under `src-tauri/tests/fixtures/phase-3-5-4c-authoritative-hydration/`, with
implementation evidence under `docs/evidence/phase-3-5-4c/`.
Phase 3.5.4d sanitized UI/gap/multi-client fixtures live under
`src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/`, with implementation
evidence under `docs/evidence/phase-3-5-4d/`.
Phase 3.5.4e aggregate compatibility fixtures live under
`docs/fixtures/phase-3-5-4-compatibility/`, with Rust/TypeScript compatibility
suites and closeout evidence under `docs/evidence/phase-3-5-4e/`.

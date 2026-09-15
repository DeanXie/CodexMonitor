# Phase 3.5.2c — Subscription / Release Lifecycle

Status: Phase 3.5.2c.1 shared subscription and runtime observation contracts
are **PASS / COMPLETE / FROZEN**. Phase 3.5.2c.2 synthetic detach boundary
freeze and Phase 3.5.2c.3 upstream unsubscribe instrumentation are
**PASS / COMPLETE / FROZEN**. Phase 3.5.2c remains in progress. Phase 3.5.2c.4
has not started.

## Authority separation

CodexMonitor keeps three lifecycle authorities independent:

1. Thread subscription lifecycle;
2. Thread runtime availability; and
3. writer admission lifecycle.

No state in one authority establishes a state in another. In particular,
unsubscribe, `thread/closed`, and `notLoaded` do not establish writer freedom,
availability, ownership, lease identity, or release.

## Bundled and upstream protocol evidence

The bundled authority is Windows x64 `codex-cli 0.153.4`, executable SHA-256
`444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B`.
The frozen official source authority is Codex commit
`3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`.

In that authority, upstream `thread/unsubscribe` removes the calling
app-server connection from the Thread subscriber set and returns one of
`unsubscribed`, `notSubscribed`, or `notLoaded`. The subscription identity is
an internal app-server connection identity, not a Remote-client identity.
Removing the last subscriber does not immediately unload an active runtime;
unload is delayed and separately observed.

CodexMonitor's existing `thread_live_unsubscribe` remains a synthetic local
live detach. It does not send upstream `thread/unsubscribe` or ingest
`thread/closed`.

## Normalized observation scope

Subscription and runtime observations are bound to:

```text
(
  WorkspaceSession generation,
  app-server connection generation,
  CodexThreadKey
)
```

The app-server connection generation is evidence scope only. It is not a
Remote-client identity, writer owner, or lease identifier. A new WorkspaceSession
generation or app-server connection generation starts with no inherited current
observation.

## Thread subscription observation

`ThreadSubscriptionObservation` records the scope, `observedAt`, optional
unsubscribe `attemptId`, requested full Thread ID, and minimal direct
response/error evidence. Its serialized state contract is:

```text
not_observed
subscribed_for_app_server_connection
unsubscribe_pending
unsubscribed_for_app_server_connection
not_subscribed_for_app_server_connection
unsubscribe_outcome_unknown
```

The allowed Phase 3.5.2c.1 transitions are:

```text
NOT_OBSERVED
  -> SUBSCRIBED_FOR_APP_SERVER_CONNECTION

SUBSCRIBED_FOR_APP_SERVER_CONNECTION
  -> UNSUBSCRIBE_PENDING

UNSUBSCRIBE_PENDING
  -> UNSUBSCRIBED_FOR_APP_SERVER_CONNECTION
  -> NOT_SUBSCRIBED_FOR_APP_SERVER_CONNECTION
  -> UNSUBSCRIBE_OUTCOME_UNKNOWN
```

Timeout, dispatch disconnect, or cancellation after dispatch produces
`UNSUBSCRIBE_OUTCOME_UNKNOWN`. The model defines no automatic unsubscribe
retry.

## Thread runtime availability observation

`ThreadRuntimeAvailabilityObservation` is an independent direct-evidence
model. Its serialized state contract is:

```text
unknown
loaded_observed
not_loaded_observed
```

Only an explicit evidence source may record loaded or not-loaded runtime
availability. Direct loaded evidence may follow not-loaded evidence and vice
versa. An unsubscribe result alone never changes runtime availability.

## Writer-admission non-transition contract

Subscription and runtime events do not transition
`WriterAdmissionObservation`. This includes:

- `UNSUBSCRIBED_FOR_APP_SERVER_CONNECTION`;
- `NOT_SUBSCRIBED_FOR_APP_SERVER_CONNECTION`;
- `NOT_LOADED_OBSERVED`; and
- `thread/closed`.

Only the already-frozen, directly observed end of a WorkspaceSession generation
may transition writer admission evidence to
`SESSION_ENDED_RELEASE_UNOBSERVED`. Phase 3.5.2c.1 adds no `FREE`, `AVAILABLE`,
`RELEASED`, writer-owner, lease, Remote-client ownership, force-takeover, or
retry semantics.

## Synthetic live detach boundary

Phase 3.5.2c.2 freezes `thread_live_unsubscribe` as `LOCAL SYNTHETIC DETACH
ONLY`. The App command and daemon RPC use one shared outcome contract. After
confirming that the Workspace exists and has a current WorkspaceSession, each
successful call emits exactly one local `thread/live_detached` event with
`reason = manual` and returns `{ "ok": true }`.

The operation dispatches no app-server request. It does not resume or start a
Thread, start a Turn, connect a Workspace, end a WorkspaceSession, terminate an
app-server process, or change subscription, runtime-availability, or writer-
admission evidence. A shared WorkspaceSession and all of its routes remain
intact. The outcome records no Remote-client subscription owner.

Repeated explicit calls repeat the same successful local detach event; there
is no authoritative subscription state that would turn the second call into
an upstream no-op. Transport disconnect does not automatically retry the
operation, so an ambiguous response cannot silently duplicate the local event.

A missing Workspace returns `workspace not found`. A known Workspace without
a current WorkspaceSession returns `workspace session unavailable`. Neither
case emits a local detach event or manufactures subscription, runtime, or
writer evidence.

## Explicit upstream unsubscribe boundary

Phase 3.5.2c.3 adds a separate explicit operation named
`thread_upstream_unsubscribe` on both the App command and daemon RPC surfaces.
Both adapters call `thread_upstream_unsubscribe_core`, which dispatches exactly
one app-server `thread/unsubscribe` request with the requested full Thread ID.
The mutation is excluded from automatic disconnect retry.

Before dispatch, the shared runtime creates a unique attempt ID and records
`UNSUBSCRIBE_PENDING` for the tuple:

```text
(
  WorkspaceSession generation,
  app-server connection generation,
  CodexThreadKey
)
```

The attempt ID is correlation evidence only. It is not a subscription owner,
writer owner, or lease. A second concurrent attempt for the same scoped Thread
fails closed, and an attempt from an old generation cannot update a replacement
generation.

The exact response mapping is:

```text
unsubscribed
  -> UNSUBSCRIBED_FOR_APP_SERVER_CONNECTION

notSubscribed
  -> NOT_SUBSCRIBED_FOR_APP_SERVER_CONNECTION

notLoaded
  -> NOT_SUBSCRIBED_FOR_APP_SERVER_CONNECTION
  +  NOT_LOADED_OBSERVED in the independent runtime model
```

Timeout, response loss, connection loss, cancellation after dispatch, or a
malformed response records `UNSUBSCRIBE_OUTCOME_UNKNOWN`. No such outcome is
automatically retried. Cancellation before the dispatch boundary restores the
prior subscribed evidence, because upstream acceptance is known not to have
been attempted. A response containing both JSON-RPC `error` and a recognized
result status is malformed and cannot record successful unsubscribe evidence.
Successful unsubscribe does not end the shared
WorkspaceSession or app-server process, and every response or ambiguous outcome
leaves `WriterAdmissionObservation` unchanged.

The bundled request and response fixtures live in
`docs/fixtures/app-server/thread-unsubscribe/`. No real production Thread was
used to verify this slice.

## Slice boundary

Phase 3.5.2c.1 contains crate-private shared reducers and contract tests only.
Phase 3.5.2c.2 centralizes and freezes the pre-existing local App/daemon detach
contract without adding an upstream request or public API. Phase 3.5.2c.3 adds
the explicit request, response/outcome mapping, attempt correlation, generation
isolation, and no-retry boundary. Delayed `thread/closed` ingestion, unload
timing reconciliation, reconnect reconciliation, and Phase 3.5.2c.4 remain
outside this slice.

# Phase 3.5.2c — Subscription / Release Lifecycle

Status: Phase 3.5.2c.1 shared subscription and runtime observation contracts
are **PASS / COMPLETE / FROZEN**. Phase 3.5.2c remains in progress. Phase
3.5.2c.2 has not started.

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

CodexMonitor's existing `thread_live_unsubscribe` remains synthetic local live
subscription bookkeeping. Phase 3.5.2c.1 does not send upstream
`thread/unsubscribe`, change that product path, or ingest `thread/closed`.

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

## Slice boundary

Phase 3.5.2c.1 contains crate-private shared reducers and contract tests only.
It adds no public App, daemon, frontend, or upstream mutation surface. Wiring
the existing synthetic detach boundary belongs to Phase 3.5.2c.2 and is not
part of this slice.

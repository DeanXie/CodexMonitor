# Phase 3.5.1d — Windows Remote Acceptance

Status: **IN PROGRESS**. Gate C is **PASS**. Phase 3.5.1d-b Remote Backend Connection Ownership Fix is **PASS / FROZEN**. Acceptance resumes at Gate D. Phase 3.5.2 is **NOT STARTED**.

## Frozen connection ownership contract

The Host Availability reducer and remote backend connection cache use the same attempt/generation ownership semantics:

- concurrent cache-miss callers share one single-flight initialization;
- only the current generation may publish a cached authenticated backend;
- an older completed handshake cannot replace a newer cached backend;
- cache invalidation increments the generation and prevents an in-flight stale initializer from publishing;
- a failed client clears the cache only when it is the currently cached connection;
- stale client disconnect or EOF cannot mark the current availability attempt disconnected;
- a disconnect from the current connection still invalidates current availability.

`connect_workspace` success records `RuntimeReady(workspaceId)` against the attempt owned by the current published backend. The reducer's stale-attempt rejection remains unchanged. `workspaceId` and `lastRuntimeReadyAt` therefore require current-attempt runtime evidence.

The correction does not redesign the Host Availability states, `RemoteHostIdentity`, daemon transport, `WorkspaceSession` ownership, writer lifecycle, approval, deletion authority, Thread truth, projections, Workspace/Project identity, or token accounting.

## Root cause and correction

The Gate C blocker was a concurrent reconnect race. `connect_workspace` and workspace-list polling could initialize separate authenticated connections after a shared cache miss. A later-finishing older attempt could overwrite the cache; dropping the newer connection then produced `transport read ended`, while the older attempt's `RuntimeReady` evidence was correctly rejected as stale.

The correction is single-flight initialization plus generation-aware publication and compare-and-clear ownership. Normal RPCs reuse the same authenticated long connection, and discarded stale connections cannot mutate current availability.

## Gate C authoritative evidence

The real Windows acceptance run used the production polling and `connect_workspace` paths concurrently. No polling path was disabled.

```text
Gate C = PASS
attemptId = 1
Transport = CONNECTED
Auth = AUTHENTICATED
Daemon = AVAILABLE
Runtime = READY
Summary = READY
workspaceId = phase-3-5-1d-workspace
lastRuntimeReadyAt = 1788948285839
daemon connected = true
diagnostics = []
```

After a complete 15-second polling cycle, the snapshot remained `CONNECTED / AUTHENTICATED / AVAILABLE / READY` with the same workspace identity and non-null `lastRuntimeReadyAt`. There was no recurrence of `transport read ended`, `DISCONNECTED`, Runtime `UNKNOWN`, or a null workspace identity.

Implementation commit: `096095b` (`fix: serialize remote backend connection ownership`).

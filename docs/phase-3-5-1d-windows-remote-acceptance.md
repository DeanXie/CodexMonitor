# Phase 3.5.1d — Windows Remote Acceptance

Status: **PASS / COMPLETE**. Gates A-L are **PASS**. Phase 3.5.1 is **PASS / COMPLETE** for the Windows implementation scope. Phase 3.5.2 Exact-ID Remote Admission & Writer Lifecycle is **GO / NOT STARTED**. Deferred Mobile Artifact Validation remains **NOT YET EXECUTED**.

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

## Frozen endpoint-failure admission contract

An authoritative TCP endpoint failure records `ENDPOINT_UNREACHABLE` and a bounded five-second negative initialization result owned by the same remote target and settings generation. During that window, queued and polling callers reuse the typed failure without allocating an attempt, opening another TCP connection, or changing current availability back to `CONNECTING`.

After the window expires, the next eligible caller may start exactly one new connection attempt. A repeated endpoint failure establishes a new bounded window. This is admission control, not sticky availability: a real admitted reconnect still changes current transport state to `CONNECTING`.

The negative result is invalidated by endpoint, token, provider, or active-target configuration changes. It does not clear or rewrite the pinned `RemoteHostIdentity`. Authentication rejection, identity mismatch, and unsupported protocol remain distinct typed outcomes and are not reclassified as endpoint failures.

Endpoint failure and coalescing cannot emit Thread absence or deletion, create tombstones, synthesize complete empty inventory, mutate projections or Workspace/Desktop Project identity, or change token accounting.

## Gate E authoritative evidence

The acceptance daemon was stopped and its listener was released while production polling remained enabled. The real Windows run observed:

```text
Gate E = PASS
first failed window attemptId = 10
first negative window = approximately 5.114 seconds
second attemptId = 11
second negative window = approximately 5.068 seconds
attempt sequence = 10 -> 11 -> 12
observation window = 18.36 seconds
actual TCP attempts = 3
coalesced production reads = 40
Transport = ENDPOINT_UNREACHABLE remained observable
expected RemoteHostIdentity = 014383f2-41f8-4b13-b9d7-30c511e47cec
```

The Host identity pin and historical readiness timestamps were preserved without being treated as current `READY`. No Thread, tombstone, projection, Workspace, Desktop Project, or token-accounting truth changed.

Implementation commit: `df7f6b9` (`fix: coalesce remote endpoint connection failures`).

## Frozen terminal-failure preservation contract

A cleanup disconnect for the same availability attempt preserves an already-confirmed authoritative terminal failure:

- `AuthState::FAILED`;
- `DaemonState::IDENTITY_MISMATCH`;
- `DaemonState::PROTOCOL_UNSUPPORTED`;
- `DaemonState::SERVICE_MISMATCH`;
- `DaemonState::INVALID_RESPONSE`.

The summary selector gives these terminal causes precedence over the generic transport cleanup state. `Auth FAILED + Transport DISCONNECTED` therefore summarizes as `AUTHENTICATION_FAILED`; the corresponding daemon validation failures summarize as their existing typed identity, protocol, or daemon-verification outcome.

This evidence belongs only to the current attempt. `begin_attempt(newId)` resets prior terminal evidence to the normal connecting/not-observed states, and stale-attempt events remain unable to overwrite the current attempt. Without an authoritative terminal failure, an unexpected disconnect from `CONNECTED / AUTHENTICATED / AVAILABLE / READY` still clears current success evidence to `DISCONNECTED / UNKNOWN / UNKNOWN / UNKNOWN` and summarizes as `DISCONNECTED`. The amendment is not sticky failure state and does not change transport lifecycle, retries, polling, Host identity, Thread authority, projections, Workspace/Project identity, or token accounting.

## Gate H authoritative evidence

The real Windows wrong-token acceptance run observed an authoritative daemon `invalid token` rejection followed by normal uncommitted-connection cleanup:

```text
Gate H = PASS
baseline attemptId = 1
wrong-token stable attemptId = 384
Transport = DISCONNECTED
Auth = FAILED
Daemon = NOT_OBSERVED
Runtime = NOT_OBSERVED
Summary = AUTHENTICATION_FAILED
expected RemoteHostIdentity = 014383f2-41f8-4b13-b9d7-30c511e47cec
```

Both `invalid token` and the subsequent `transport read ended` diagnostic were retained, while the Host identity pin was unchanged. The state did not become `ENDPOINT_UNREACHABLE`, `IDENTITY_MISMATCH`, `PROTOCOL_UNSUPPORTED`, or `READY`. Restoring the correct test token created attempt `719`, reset the previous terminal evidence, and recovered `CONNECTED / AUTHENTICATED / AVAILABLE / READY` for `phase-3-5-1d-workspace` with no observed Thread or accounting pollution.

Wrong-token polling produced rapidly increasing attempt IDs. That is recorded as a non-blocking retry/admission optimization observation; this Slice does not add authentication-failure cooldown or change polling/retry policy.

Implementation commit: `0e5a9ea` (`fix: preserve remote terminal availability failures`).

## Final acceptance matrix

| Gate | Acceptance | Result |
| --- | --- | --- |
| A | First-run `RemoteHostIdentity` | PASS |
| B | Authenticated handshake / TOFU | PASS |
| C | Runtime READY | PASS |
| D | Host-qualified `ExecutionEnvironmentKey` | PASS |
| E | `ENDPOINT_UNREACHABLE` safety | PASS |
| F | Same data-dir restart / reconnect | PASS |
| G | Endpoint migration / same Host identity | PASS |
| H | Wrong-token authentication failure | PASS |
| I | Host identity mismatch / fail-closed | PASS |
| J | Attempt generation / stale observation isolation | PASS |
| K | Persistence consistency | PASS |
| L | Final Thread / accounting isolation | PASS |

Gate G's first run used a restricted sandbox/command-runner daemon lifecycle and failed before app-server initialization. That result is **INVALIDATED ACCEPTANCE HARNESS EVIDENCE**, not a product regression. The production-lifecycle retest passed.

## Final identity and isolation evidence

Host A is `014383f2-41f8-4b13-b9d7-30c511e47cec`. Its `remote-host-identity.json`, authenticated `daemon_info`, persisted target pin, and `ExecutionEnvironmentKey = remote:014383f2-41f8-4b13-b9d7-30c511e47cec` remained consistent through restart, endpoint migration, authentication failure, Host B mismatch, and recovery.

Host B `0ae35772-5057-459e-b2b2-f259e492303f` was observed only as mismatch evidence and did not overwrite Host A's identity store, target pin, or execution environment. `RemoteHostIdentity` remains distinct from authentication token, endpoint, Workspace, and Thread identity. Canonical identity remains `CodexThreadKey = (codexHomeIdentity, fullThreadId)`; cross-host routing remains `RemoteThreadLocator { remoteHostIdentity, threadKey: CodexThreadKey }`.

The final authoritative snapshot recorded 491 canonical Threads / 491 unique / 0 duplicate and 489 current Turns / 489 unique / 0 duplicate. All 802 Surface projections were Desktop Catalog observations: 403 PRESENT and 399 ABSENT. The ABSENT values are Desktop projection absence, not canonical Thread absence. No Remote, Sidebar, or Project projection was synthesized by availability failure, and no projection entry contributed token or runtime fields. No fake Thread absence/deletion, tombstone, Workspace/Project reassignment, duplicate token lane, duplicate runtime entry, or duplicate current Turn was observed.

Phase 3.5.1d-b, Phase 3.5.1d-c, and Phase 3.5.1d-e are **PASS / FROZEN**. Phase 3.5.1d is **PASS / COMPLETE** and Phase 3.5.1 is **PASS / COMPLETE**. Phase 3.5.2 Exact-ID Remote Admission & Writer Lifecycle is **GO / NOT STARTED**.

Phase 3.5.1a remains frozen only for the Windows source architecture and regression scope. Real `aarch64-apple-ios` and `aarch64-apple-ios-sim` target checks plus a Tauri iOS artifact/device build remain **Deferred Mobile Artifact Validation — NOT YET EXECUTED**; no iOS artifact validation is claimed.

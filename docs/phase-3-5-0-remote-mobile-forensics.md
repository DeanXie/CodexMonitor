# Phase 3.5.0 — Remote / Mobile Interoperability Forensics & Contract

Status: **FORENSICS COMPLETE**. Phase 3.5 is **IN PROGRESS**. Phase 3.5.1 Mobile Build / Remote Host Identity & Availability Baseline is **GO / NOT STARTED**.

## Confirmed architecture

The confirmed execution path is:

```text
Mobile/Remote client
→ authenticated TCP
→ codex_monitor_daemon on execution host
→ host-local Codex app-server
→ host CODEX_HOME / persisted Thread store
```

The remote client is a control surface. It does not become the execution runtime or a new persisted Thread store.

## Identity contract

Canonical Thread identity remains unchanged:

```text
CodexThreadKey = (codexHomeIdentity, fullThreadId)
```

Phase 3.5 adds a host-routing namespace, not a second Thread identity:

```text
RemoteHostIdentity

RemoteThreadLocator {
  remoteHostIdentity,
  threadKey: CodexThreadKey
}
```

`RemoteHostIdentity` identifies and routes to the execution host. It must remain distinct from device identity, transport-session identity, `ExecutionEnvironmentKey`, `codexHomeIdentity`, `WorkspaceKey`, and `fullThreadId`.

## Surface, transport, producer, and host

The frozen layering is:

```text
initiator/control surface = MOBILE
transport = REMOTE_TCP
execution producer = MONITOR
execution host = RemoteHostIdentity
```

No `MOBILE` or `REMOTE` Producer Surface is introduced. A remote transport path does not change the host-side producer that performs execution.

## Safety and authority boundaries

- Host unavailable does not imply Thread `ABSENT`.
- TCP disconnected does not imply Thread deleted.
- Mobile UI visibility does not prove exact-ID resumability.
- Remote transport is not a token source and transport latency is not model runtime.
- Desktop Project, Remote Workspace, and Thread identity remain separate.
- Remote observations cannot authoritatively establish canonical Thread existence, deletion tombstones, effective execution settings, active-writer ownership, Workspace identity, Desktop Project assignment, token usage, or approval completion.

## Approval and deletion capabilities

- A connected one-shot approve/decline path is **CODE-CONFIRMED**.
- Reliable Mobile approval capability is a **GAP / NOT PROVEN**.
- The official host `thread/delete` request is **CONFIRMED**.
- Host-authoritative tombstone persistence is **NOT IMPLEMENTED / NOT PROVEN**.

These capability boundaries must not be replaced by UI inference, private-state writes, or force-takeover behavior.

## Mobile build boundary

- A source-level mobile compile blocker is **CONFIRMED**.
- A current iOS artifact build/run is **NOT PROVEN**.

Resolving the mobile compile boundary is the first priority of Phase 3.5.1.

## Frozen delivery route

1. Phase 3.5.1 — Mobile Build / Remote Host Identity & Availability Baseline
2. Phase 3.5.2 — Exact-ID Remote Admission & Writer Lifecycle
3. Phase 3.5.3 — Remote Approval & Deletion Authority
4. Phase 3.5.4 — Remote Projection / Recovery / Telemetry
5. Final E2E

Phase 3.5.1 passes only when:

1. the mobile cfg blocker is resolved;
2. the iOS target builds successfully;
3. a stable `RemoteHostIdentity` exists;
4. restarting the same daemon data directory preserves host identity;
5. a different host or data directory produces a different identity;
6. host unavailability never generates Thread `ABSENT`;
7. a typed availability contract is established.

Phase 3.5.1 is the next and only development start point. No Phase 3.5.1 product source change is part of this closeout.

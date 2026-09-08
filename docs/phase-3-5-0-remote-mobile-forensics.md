# Phase 3.5.0 — Remote / Mobile Interoperability Forensics & Contract

Status: **FORENSICS COMPLETE**. Phase 3.5 is **IN PROGRESS**. Phase 3.5.1a Mobile Compile Boundary is **PASS / FROZEN (Windows implementation scope)**. Phase 3.5.1b RemoteHostIdentity is **PASS / FROZEN**. Phase 3.5.1c Host Availability is **PASS / FROZEN**. Phase 3.5.1d Windows Remote Acceptance is **GO / NOT STARTED**.

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

- The source-level mobile compile boundary is resolved for the Windows development scope.
- Phase 3.5.1a is **PASS / FROZEN (Windows implementation scope)**.
- Real `aarch64-apple-ios` / `aarch64-apple-ios-sim` target checks and a Tauri iOS artifact build are **Deferred Mobile Artifact Validation — NOT YET EXECUTED**.

This status confirms the source architecture and Windows regression gates only. It does not claim that an iOS artifact has been built or validated.

## Frozen delivery route

1. Phase 3.5.1a — Mobile Compile Boundary
2. Phase 3.5.1b — RemoteHostIdentity
3. Phase 3.5.1c — Host Availability
4. Phase 3.5.1d — Windows Remote Acceptance
5. Phase 3.5.2 — Exact-ID Remote Admission & Writer Lifecycle
6. Phase 3.5.3 — Remote Approval & Deletion Authority
7. Phase 3.5.4 — Remote Projection / Recovery / Telemetry
8. Final E2E

Phase 3.5.1a Windows implementation scope is frozen with:

1. the mobile cfg blocker is resolved;
2. canonical identity value types extracted into a platform-neutral module with one definition and compatibility re-exports;
3. desktop-heavy Global Source, rollout, Desktop metadata/projection, tombstone, and local `CODEX_HOME` construction isolated from the mobile graph;
4. mobile `read_thread` and `delete_thread` routed only through the remote backend;
5. no Mobile-to-local-`CODEX_HOME` fallback or Mobile-local tombstone authority;
6. desktop and full regression gates passing on Windows.

The deferred iOS target and artifact gates will run during future Mobile build/device E2E. Phase 3.5.1b now provides the stable persisted host-routing identity, authenticated handshake, TOFU pinning, `RemoteThreadLocator`, and host-qualified remote execution environment described in `phase-3-5-1b-remote-host-identity.md`.

The current daemon has one effective `CODEX_HOME`. Copying its complete data directory preserves the same logical `RemoteHostIdentity`; simultaneous cloned identities are an unsupported operational condition. Phase 3.5.1c now provides typed, process-local Host availability without changing Thread authority, as documented in `phase-3-5-1c-host-availability.md`. Phase 3.5.1d Windows Remote Acceptance is the next and only development start point.

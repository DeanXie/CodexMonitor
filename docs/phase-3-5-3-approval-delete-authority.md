# Phase 3.5.3 — Approval and Delete Authority

Status: Phase 3.5.3 forensics and contract freeze are complete. Phase 3.5.3a
Approval Request Observation is **PASS / COMPLETE / FROZEN**. Later Phase
3.5.3 slices are not started.

## Authority boundary

Approval request observation is separate from approval-decision mutation,
action execution results, upstream auto-review, and sandbox permission policy.
Phase 3.5.3a adds observation only. It does not add, replace, or retry any
approval decision and does not change deletion behavior.

The shared observation authority is scoped by the current
`WorkspaceSessionGeneration` and `AppServerConnectionGeneration`. An approval
identity additionally contains the exact app-server JSON-RPC `requestId`,
`threadId`, `turnId`, and `itemId`. An optional `approvalId` is subordinate
correlation evidence and never replaces `requestId`.

The only Phase 3.5.3a request families are:

- `item/commandExecution/requestApproval`;
- `item/fileChange/requestApproval`;
- `item/permissions/requestApproval`.

`mcpServer/elicitation/request` remains a separate protocol family and is out
of scope for Phase 3.5.3a.

## Observation states

The request-observation states are:

- `not_observed`;
- `pending`;
- `resolved_or_cleared`;
- `session_ended_unresolved`.

No state means accepted, declined, auto-approved, free, released, owned, or
leased. Decision transport outcomes belong to a later
`ApprovalDecisionProvenance` slice.

`serverRequest/resolved` closes only the current-generation pending request
whose exact `requestId` and `threadId` match. It proves resolved or cleared,
not which decision won or who supplied it.

`item/completed` is authoritative action-result evidence. It may retire an
actionable pending request only when `requestId`, `threadId`, `turnId`, and
`itemId` all match the current approval identity. Thread/item-only correlation
is insufficient. It never proves that a human decision was accepted.

Upstream `item/autoApprovalReview/*` events are lifecycle annotations only.
CodexMonitor's local command allowlist auto-accept path is not upstream
auto-review evidence.

Pending observations become `session_ended_unresolved` only when the owning
WorkspaceSession or app-server connection generation is directly observed to
end. Remote TCP disconnect, page close, and ordinary Remote transport
reconnect do not end the authority generation.

## Projection boundary

The existing frontend approval queue remains a projection rather than an
authority. `serverRequest/resolved` removes the exact
workspace/request/Thread identity from the actionable projection. Phase
3.5.3a does not expand the decision UI or its mutation vocabulary.

Multiple Remote transports sharing one WorkspaceSession observe the same
session-scoped request state. This does not create a primary approver,
Remote-client ownership, or a decision lease.

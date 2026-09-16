# Phase 3.5.3 — Approval and Delete Authority

Status: Phase 3.5.3 forensics and contract freeze are complete. Phase 3.5.3a
Approval Request Observation and Phase 3.5.3b Remote Approval Decision
Correlation are **PASS / COMPLETE / FROZEN**. Later Phase 3.5.3 slices are not
started.

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

No request-observation state means accepted, declined, auto-approved, free,
released, owned, or leased.

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

## Remote decision correlation

Phase 3.5.3b binds each explicit Remote `respond_to_server_request` approval
decision to the exact current approval identity and to one unique
`ApprovalDecisionAttemptId`. The attempt records its Remote transport
generation/request provenance separately from the WorkspaceSession and
app-server connection generations. Transport provenance is audit evidence; it
is not a Remote-client identity, approval owner, or lease.

The decision-attempt states are:

- `not_observed`;
- `decision_pending`;
- `decision_dispatched`;
- `decision_not_dispatched`;
- `decision_outcome_unknown`;
- `decision_stale_rejected`.

Only one attempt can be admitted for an exact approval identity. Resolved,
completed, session-ended, old-generation, wrong-schema, and duplicate attempts
fail closed. A replacement transport may submit a new explicit request only
while the same exact approval remains pending and no attempt has been admitted.

Command-execution, file-change, and permissions results are validated against
their bundled Codex 0.153.4 response families before dispatch. The model stores
only the normalized response kind, not command, path, permission payload, or
credential content.

Crossing the app-server stdin write boundary records `decision_dispatched`.
This proves only that the typed response bytes were written. It does not prove
that upstream applied the decision, which decision won, or who supplied it.
Transport loss or cancellation before that boundary with zero writes records
`decision_not_dispatched`; loss, cancellation, or missing caller response after
the boundary records `decision_outcome_unknown`. Automatic retry and replay are
both zero.

`serverRequest/resolved` and exact `item/completed` may annotate the related
attempt, but neither identifies a winning attempt. Confirmed generation end
can make an unresolved dispatched outcome unknown; Remote TCP disconnect alone
cannot end the session authority. App-local response dispatch remains outside
Remote transport correlation.

All Phase 3.5.3b mutation tests use fake app-server sessions and deterministic
daemon transport fixtures. No real approval request was answered during this
slice, and Thread deletion remains unchanged.

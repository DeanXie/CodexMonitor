# Phase 3.5.3 — Approval and Delete Authority

Status: Phase 3.5.3 forensics and contract freeze are complete. Phase 3.5.3a
Approval Request Observation, Phase 3.5.3b Remote Approval Decision
Correlation, and Phase 3.5.3c Delete Authority and Exact-ID Model are
**PASS / COMPLETE / FROZEN**. Phase 3.5.3d is not started.

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
daemon transport fixtures. No real approval request was answered during that
slice.

## Delete authority and exact-ID model

Phase 3.5.3c admits `thread/delete` only for an exact canonical full Thread ID.
Title, cwd, prefix, suffix, display label, and other fuzzy matches are never
delete authority. Each explicit request creates one `DeleteAttemptId` bound to
the `RemoteHostIdentity`, `CodexThreadKey`, current
`WorkspaceSessionGeneration`, current `AppServerConnectionGeneration`, and
requested full Thread ID. A Remote request may additionally bind its existing
transport/request provenance to that same session attempt; this is audit
correlation, not a Remote-client identity, delete owner, or lease.

The delete-observation states are:

- `not_observed`;
- `delete_pending`;
- `delete_confirmed`;
- `delete_rejected`;
- `delete_outcome_unknown`;
- `session_ended_outcome_unknown`.

An exact successful `thread/delete` response with the bundled empty result is
direct `delete_confirmed` evidence. A current-generation exact
`thread/deleted` notification is also direct confirmation evidence. An
active-writer `-32600` response is `delete_rejected`. Loss, cancellation,
malformed response, or transport disconnect after dispatch without an
authoritative response is `delete_outcome_unknown`; it never creates a
tombstone. Confirmed app-server or WorkspaceSession generation end may move an
unresolved dispatched attempt to `session_ended_outcome_unknown`. Remote TCP
disconnect alone does not end either session generation.

Only `delete_confirmed` authorizes canonical tombstone reconciliation. Missing
rollout files, `thread/closed`, runtime `notLoaded`, UI removal, stale-generation
notifications, and transport loss are non-authoritative and cause no delete
transition. Confirmed reconciliation preserves exact full-ID identity and does
not fall back to title/cwd matching.

App and daemon use the same shared delete core and the same empty-success,
active-writer, unknown-outcome, and tombstone gates. Automatic retry and replay
are both zero. All Phase 3.5.3c tests use a fake app-server, deterministic
WorkspaceSession/transport fixtures, sanitized IDs, and temporary test state.
No real Thread, rollout, or frozen A3 target was deleted.

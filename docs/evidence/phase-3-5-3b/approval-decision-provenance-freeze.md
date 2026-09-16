# Phase 3.5.3b approval decision provenance evidence

Status: **PASS / COMPLETE / FROZEN**.

## RED

The first focused build failed because no shared decision-attempt model,
Remote approval transport correlation, or `ApprovalDecision` session-attempt
kind existed. The daemon transport fixture also failed before the new kind was
available. These failures established that the 3.5.3a observation registry did
not yet provide decision dispatch provenance.

## GREEN authority

The shared `ApprovalObservationRuntime` now atomically admits at most one
decision attempt for an exact current approval identity. The decision attempt
records its own UUID, current WorkspaceSession and app-server connection
generations, normalized response kind, Remote transport generation/request ID,
write-boundary state, dispatch count, and retry count.

Remote `respond_to_server_request` dispatch uses the existing app-server stdin
writer against fake/deterministic sessions in tests. A successful write records
only `decision_dispatched`. Pre-write loss with zero writes records
`decision_not_dispatched`; post-write ambiguity records
`decision_outcome_unknown`. All retries and replays remain zero.

The bundled command-execution, file-change, and permissions response families
are frozen in `docs/fixtures/app-server/approval-decision-provenance/`. Wrong
kind/schema, stale generation, resolved/completed/session-ended request,
duplicate, and simultaneous multi-transport cases fail closed.

No real approval request was answered while producing this evidence. No real
command, file modification, or permission grant was approved. Thread deletion,
Remote-client identity, approval ownership, lease semantics, and Phase 3.5.3c
remain outside this slice.

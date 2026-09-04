# Phase 3.3.2 — Creation-intent / First-Turn Coordination

Status: **PASS / FROZEN**. Phase 3.3.3 Forensics / Contract is **COMPLETE**; Phase 3.3.3a, Phase 3.3.3b, and Phase 3.3.3c are **PASS / FROZEN**; Phase 3.3.3 Effective Execution Settings Evidence is **PASS / COMPLETE**. The current/default naming mismatch is **RESOLVED**. Phase 3.3 Final Session Acceptance is **GO / NOT STARTED**.

## Contract and ownership

The guarantee is **client-side at-most-once transport dispatch per intent** in the current app/daemon process, with fail-closed unknown outcomes. It is not server-side idempotency or end-to-end exactly-once.

An explicit user action owns its CreationIntentId or FirstTurnIntentId. Frontend callbacks carry the original token through IPC; the coordinator never generates an operation ID. The token contains an opaque UUID and the backend process epoch. It is not a JSON-RPC request ID, fullThreadId, WorkspaceEntry ID, or Desktop Project ID. Distinct explicit actions remain distinct even with identical workspace, cwd, or prompt.

The shared Phase 3 coordinator is owned by AppState / DaemonState. Session connections hold only a clone for observing exact outcomes. In-flight and terminal entries have no eviction within the process lifetime. They are not stored in the Phase 2 Global Source Registry or in Desktop private metadata.

## Creation state

```text
INTENT_CREATED → START_DISPATCHING → START_IN_FLIGHT
                                      ├─ THREAD_ACKNOWLEDGED
                                      ├─ CREATION_FAILED
                                      └─ CREATION_OUTCOME_UNKNOWN
```

The first caller atomically claims an intent before asynchronous preflight. A concurrent duplicate receives ALREADY_IN_FLIGHT; settled duplicates receive the cached result without invoking transport. A changed workspace binding fails closed. Distinct intent IDs are not deduplicated by workspace or content.

Serialization and transport-lock acquisition precede the dispatch marker. The marker is set immediately before the first write attempt: partial writes and write errors are conservatively possibly sent. Failure/cancellation before that boundary is CREATION_FAILED; failure/cancellation after it is CREATION_OUTCOME_UNKNOWN. An authoritative protocol rejection or invalid creation acknowledgement is CREATION_FAILED, with no replacement start.

Only the frozen Phase 3.3.1 acknowledgement validator accepts the exact server fullThreadId. Acknowledgement does not establish persistence, ephemeral status, first-Turn acceptance, or its outcome.

## First Turn state

```text
FIRST_TURN_PENDING → FIRST_TURN_DISPATCHING → FIRST_TURN_IN_FLIGHT
                                               ├─ FIRST_TURN_ACCEPTED
                                               │    ├─ FIRST_TURN_COMPLETED
                                               │    ├─ FIRST_TURN_FAILED
                                               │    └─ FIRST_TURN_INTERRUPTED
                                               ├─ FIRST_TURN_FAILED
                                               └─ FIRST_TURN_OUTCOME_UNKNOWN
```

A Turn intent associated with a creation intent remains pending until that creation is acknowledged. Persistence confirmation is not required. The frontend can share the explicit creation action's in-flight promise and then send to the acknowledged ID; the backend independently enforces the prerequisite.

Before dispatch, the actual session CODEX_HOME plus fullThreadId must match the acknowledged canonical Thread key. Cross-home mismatches are rejected before user input is written. Preflight binds the full key so early exact terminal evidence can be buffered even for an already-existing Thread.

The same Turn intent has one claim, the same certainty rules, and cached terminal results. A response must identify a Turn before exact Thread/Turn outcome evidence can advance acceptance. Unknown acceptance is never inferred from prompt, cwd, title, time, sidebar, or catalog similarity.

Failure, protocol rejection, or interruption keeps the original acknowledged Thread. A protocol rejection uses coordination state `FIRST_TURN_FAILED` with `failureReason = REJECTED`; it does not add a separate `REJECTED` coordination state. An explicit user retry uses a new Turn intent on that same Thread, never a replacement thread/start. Newly acknowledged Threads require a Turn token until an accepted Turn is known; ordinary existing follow-up requests retain their existing protocol behavior.

## IPC and frozen boundaries

- get_creation_context returns the current processEpoch; it does not allocate an intent.
- start_thread requires creationIntent.
- send_user_message accepts turnIntent containing intent and optional creationIntent.
- get_creation_intent_status / get_first_turn_intent_status inspect process-local coordination.
- Successful responses retain their existing shape with additive creationCoordination / firstTurnCoordination metadata.
- Upstream thread/start and turn/start payloads do not gain an invented idempotency key.
- Requested model, effort, approval, sandbox, service tier, and collaboration settings are not redesigned.
- thread/start.cwd still feeds the frozen ORIGIN workspace contract. Workspace projection IDs do not enter CodexThreadKey.
- Thread acknowledgement, persistence, first-Turn acceptance, and first-Turn outcome remain separate facts.
- No thread/name/set, Desktop Project assignment, sidebar enrollment, or private-state write is part of coordination.

## Reconnect and unsupported recovery

Same-process reconnect does not reset coordination. Exact-ID read/resume attaches the existing process observer to the new WorkspaceSession; neither creates a replacement Thread.

The existing transport removes pending response/context correlation on timeout. **Late-response reconciliation is UNSUPPORTED** in this slice. A late notification without the required correlated response cannot acknowledge an unknown intent.

**Cross-process unknown-intent recovery is NOT YET SUPPORTED.** A new backend process has a new epoch and rejects tokens from an old one; clients must not silently retag them. Known fullThreadIds continue through frozen exact-ID read/resume. There is no persistent intent database, similarity discovery, transport-correlation rewrite, or automatic retry.

Process-local intent/result retention consumes memory for the process lifetime. This is the bounded safety tradeoff; no eviction that could reopen dispatch rights is implemented.

## Verification scope

Tests cover duplicate/concurrent claims, distinct actions, cancellation and real write certainty, pending first Turns, independent persistence, unknown replay suppression, explicit retry, exact identity conflicts, early terminal evidence, stale epochs, and reconnect observer binding. No real user Thread is created or resumed by these synthetic tests.

Phase 3.3.3a now defines the independent shared-core evidence model. Execution-settings ingestion, cross-process recovery, and Desktop projection behavior remain outside this implementation.

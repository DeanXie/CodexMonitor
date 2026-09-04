# Phase 3.3.3b — Execution Settings Evidence Ingestion

Status: **PASS**. Phase 3.3.3a remains **PASS / FROZEN**. Phase 3.3.3c Focused Reconciliation / Effective Settings Acceptance is **GO / NOT STARTED**.

## Authoritative inputs

The shared process-level ingestion runtime accepts three independent sources:

- `monitor-request`: fields actually present in Monitor `thread/start` or `turn/start` request parameters, retained as requested evidence;
- `app-server-response` and `app-server-settings-notification`: fields explicitly returned by `thread/start` or `thread/settings/updated`, retained as server-effective evidence;
- `rollout-turn-context:<observationId>`: fields present in a parsed rollout `turn_context`, retained as persisted-observed evidence.

Omitted fields produce no evidence record. Explicit JSON null is represented by the distinct `Null` value. Concrete strings, booleans, and string lists retain their typed canonical values. No layer is inferred from another.

## Correlation and scope

Outgoing requests first enter a request-ID pending map. This is transport correlation, not a guessed settings scope. A `turn/start` request is bound to `TURN_EXECUTION { fullTurnId }` only after its matching response supplies `result.turn.id`; a `thread/start` request is bound to `THREAD_DEFAULT` only after its response supplies `result.thread.id`. A request that loses transport correlation on failure, cancellation, or timeout is removed rather than attached to a guessed Thread or Turn.

`late-response / lost-turn correlation recovery = UNSUPPORTED`. Once timeout or disconnect removes the JSON-RPC correlation, requested settings cannot be bound reliably to a later-discovered Turn. The runtime fails closed: it does not infer a Turn from time, values, cwd, prompt, or recent events; it does not resend `turn/start`; and it does not mutate CreationIntent or FirstTurnIntent state.

The comparison group for Turn evidence is `turn:<fullTurnId>`. Rollout observations use the same group only because `turn_context.turn_id` is authoritative. Different Turns remain distinct even when they have the same Thread, values, cwd, or nearby timestamps.

`thread/settings/updated` with only `params.threadId` is a `THREAD_DEFAULT` settings snapshot. It receives its own deterministic observation comparison group and cannot conflict with a Turn observation. Only an explicit non-empty protocol `turnId` permits that notification to enter `TURN_EXECUTION`.

The app and daemon own the ingestion runtime at process scope and pass it into WorkspaceSession adapters. Reconnecting or replacing a WorkspaceSession therefore does not reset accumulated evidence. This slice does not add a persistent evidence database; process restart recovery remains outside this contract.

## Boundaries

Ingestion does not change `CodexThreadKey`, `WorkspaceKey`, scoped workspace relations, creation acknowledgement, creation-intent coordination, `ThreadCodexParams`, frontend current/default settings, access mode, or UI. It does not read or write Desktop private state. The rollout watcher emits a supplemental settings observation alongside its existing reconciliation report without changing Phase 2 canonical source-lane semantics.

Phase 3.3.3c remains the next possible slice and is not implemented here.

## Verification

- Phase 3.3.3b shared-core focused: 15 passed; app-server response/notification and rollout watcher focused tests passed.
- Phase 3.3.3a evidence model: 20 passed.
- Phase 3.3.2 creation coordination: 28 passed.
- Phase 3.3.1 acknowledgement: 18 passed.
- Phase 3.2 workspace interoperability: 77 passed / 1 ignored.
- Phase 3.1 exact-ID: 5 passed.
- Phase 2 Global Source regression: 68 passed / 1 ignored.
- Daemon regression: 399 passed / 2 ignored.
- `cargo test --lib`: 467 passed / 3 ignored.
- `cargo check --all-targets`, `cargo fmt --check`, `npm run typecheck`, and `git diff --check`: passed.

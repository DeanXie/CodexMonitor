# Phase 3.5.4c — Authoritative Recovery / Hydration Evidence

Status: **PASS / COMPLETE / FROZEN**

## Authority

The frontend recovery coordinator performs `list_workspaces`, conditional
`connect_workspace`, `thread/list`, optional exact `thread/read`, and a
read-only observation snapshot. The shared Rust query is used by both App and
daemon and preserves writer, subscription, runtime, approval,
approval-decision, and delete observations as separate authority models.

Coverage advances independently through `NOT_HYDRATED -> HYDRATING -> CURRENT`.
Observed timeout, cancellation, malformed, or lost-response failures become
`UNKNOWN`; other read/session failures become `UNAVAILABLE`. A stale response
cannot be applied after daemon, WorkspaceSession, or app-server connection
generation replacement.

## Safety contract

- Recovery retains historical projection data instead of asserting absence.
- Missing Thread/list data does not create a tombstone or delete confirmation.
- Read failures do not create canonical absence.
- Same-generation triggers share a local per-workspace single flight.
- Events remain incremental and cannot promote unhydrated/stale coverage.
- Remote frontends have independent caches but converge on shared session truth.
- `thread/resume`, approval decision, Thread delete, upstream unsubscribe,
  force takeover, mutation retry, and mutation replay counts are all zero.
- No Remote client identity, recovery owner, or lease exists.

## Fixtures and tests

Sanitized fixtures are under
`src-tauri/tests/fixtures/phase-3-5-4c-authoritative-hydration/` and contain no
real Thread IDs, rollout paths, auth tokens, or approval contents. Focused Rust
tests cover read instrumentation, observation-query parity, generation
replacement, and fixture safety. Frontend tests cover the complete recovery
chain, coverage isolation, reload/reconnect/restart behavior, stale apply gates,
partial failure, single-flight orchestration, zero mutation, and multi-client
convergence.

The initial RED was the missing `authoritativeRecovery` coordinator module. The
same tests pass after the minimal shared query, frontend orchestration, and
generation apply-gate implementation.

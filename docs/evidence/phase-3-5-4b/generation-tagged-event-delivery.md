# Phase 3.5.4b — Generation-tagged Event Delivery evidence

Status: **PASS / COMPLETE / FROZEN**.

## Contract

- Shared `app-server-event` evidence carries required WorkspaceSession and
  app-server connection generations.
- Remote delivery binds the current daemon-process and actual authenticated
  transport generations at the delivery edge; local delivery invents neither.
- Frontend delivery requires an already-current `thread_catalog` authority
  context and exact generation match before subscriber/reducer fanout. A
  current non-catalog coverage cannot bypass a not-hydrated or stale catalog.
- Missing and stale generations fail closed with zero current reducer mutation.
- Events cannot promote not-hydrated or stale coverage to current and cannot
  establish broad hydration completeness.
- Rejected events do not mutate shared writer, subscription, runtime, approval,
  or delete authority.
- No sequence, gap detection, deduplication, buffering, retry, replay, client
  identity, owner, lease, or completeness proof is introduced.

## Sanitized compatibility evidence

The fixtures under `docs/fixtures/generation-tagged-events/` cover current
local and Remote delivery, stale transport/session/connection/daemon evidence,
missing required generations, event-before-hydration, already-current coverage,
same-payload generation distinction, and multi-client transport binding.

The RED baseline failed because the former envelope exposed only `workspaceId`
and the raw app-server message, and the frontend event hub had no authoritative
generation admission context. The GREEN contract is exercised through shared
Rust envelope/freshness tests, daemon delivery tests, Remote stale-delivery
tests, frontend hub tests, TypeScript checking, and full Rust/frontend
regression.

A follow-up mixed-coverage RED proved that selecting any current coverage could
incorrectly admit an event while `thread_catalog` remained not hydrated. The
gate now binds specifically to current `thread_catalog` evidence; the same
fixture is GREEN without adding hydration or recovery behavior.

## Fresh verification

- Rust generation envelope: 7 passed, 0 failed.
- Rust projection freshness: 27 passed, 0 failed.
- Rust stale Remote delivery: 11 passed, 0 failed.
- Daemon delivery/freshness/transport-context focused checks: 3 passed, 0
  failed.
- Frontend event/IPC focused checks: 88 passed, 0 failed.
- Rust all-targets: 950 passed, 0 failed, 3 ignored.
- Frontend full suite: 1141 passed; the six previously approved zh-CN
  localization baseline failures remain unchanged.
- `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run
  typecheck`, and `git diff --check`: passed.

## Scope boundary

This Slice adds no recovery orchestration or UI. Phase 3.5.4c is not started.

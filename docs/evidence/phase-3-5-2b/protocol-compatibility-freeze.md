# Phase 3.5.2b.5 protocol compatibility freeze evidence

Status: **PASS / COMPLETE / FROZEN**.

## Fixture authority

The sanitized protocol fixtures and pinned source provenance are in
`docs/fixtures/app-server/writer-admission-observation/`. They are the durable
compatibility inputs for the shared observation core and daemon read RPC tests.
They reuse the frozen Phase 3.5.2a A2 blocked and A3 accepted evidence and do
not perform a new `thread/resume`.

The fixture contract covers exact resume accepted, active writer blocked,
ambiguous admission outcome, session ended with release unobserved,
current-generation not observed, Workspace unavailable, and WorkspaceSession
unavailable.

## RED evidence

Before the fixture files existed, the production-path fixture suite compiled
and all 12 initial tests failed while attempting to read their absent fixture
authority. No production observation behavior was changed to obtain GREEN.

## Fresh GREEN evidence

Verified on 2026-09-15:

- protocol fixture suite: 14 passed, 0 failed;
- real daemon fixture RPC route: 1 passed, 0 failed;
- focused shared writer-admission suite: 67 passed, 0 failed;
- focused daemon writer-admission suite: 70 passed, 0 failed;
- Rust all-target suites: 701/701, 590/590, 29/29, and 1/1 passed;
- `cargo check --all-targets`: passed;
- `cargo fmt --all -- --check`: passed;
- `npm run typecheck`: passed; and
- `git diff --check`: passed.

One all-target run saw the existing concurrent RemoteHostIdentity lock test
fail with Windows `Access denied`. The exact test then passed in isolation and
the fresh all-target rerun passed 590/590 for the daemon suite. No waiver was
applied.

## Frozen boundaries

App and daemon use the same shared snapshot schema and current-generation
semantics. Unknown Workspace and unavailable WorkspaceSession remain query
errors distinct from current-generation `not_observed`. Multiple Remote
clients see the same session-scoped observation without client ownership.

CodexMonitor's `thread_live_unsubscribe` is synthetic and dispatches no
upstream request. Real upstream `thread/unsubscribe` is subscription lifecycle
evidence with delayed idle unload, not writer-release acknowledgement.

Fixture tests reject authentication-secret fields, writer/lease identity,
Remote-client ownership, and global `FREE`, `AVAILABLE`, or `RELEASED` states.

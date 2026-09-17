# Phase 3.5.3c delete authority evidence

Phase 3.5.3c freezes exact-ID Thread deletion as a shared App/daemon mutation
contract. The implementation accepts only a canonical full Thread UUID and
binds one `DeleteAttemptId` to the Host identity, `CodexThreadKey`, current
WorkspaceSession generation, and current app-server connection generation.

## RED evidence

The first focused delete-observation test run failed because the shared
`delete_mutation_observation` module and its public contract did not exist.
The fixture and instrumentation tests then exercised the new shared path
through a fake app-server only. No real Thread, rollout, or A3 target was used.

## Frozen direct evidence

- bundled exact `thread/delete` empty success -> `delete_confirmed`;
- exact current-generation `thread/deleted` -> `delete_confirmed`;
- active-writer `-32600` -> `delete_rejected`;
- post-dispatch loss, cancellation, disconnect, or malformed response ->
  `delete_outcome_unknown`;
- confirmed generation end after an unresolved dispatch ->
  `session_ended_outcome_unknown`.

Only `delete_confirmed` can yield a canonical tombstone. Pending, rejected,
unknown, missing rollout, closed, notLoaded, UI-removal, transport-loss, and
stale-generation evidence cannot. Remote request provenance may correlate to
the session attempt but is not a Remote-client identity, delete owner, or
lease. Automatic retry and replay are zero.

## Fixture authority

Sanitized protocol fixtures live in
`docs/fixtures/app-server/delete-mutation-observation/`. Focused model,
instrumentation, and fixture tests execute against deterministic fake process
and session authority. Fresh all-target Rust, check, formatting, frontend
typecheck, and diff validation are the closeout gate for this slice.

## Fresh closeout verification

- focused delete authority/model/instrumentation/fixture tests: 45 passed;
- focused confirmed-reconciliation tests: 6 passed;
- focused daemon Remote delete correlation: 1 passed;
- Rust all targets: library 916 passed / 4 ignored, daemon 824 passed / 3
  ignored, daemonctl 29 passed, Tauri config 1 passed, zero failures;
- `cargo check --all-targets`, `cargo fmt --all -- --check`,
  `npm run typecheck`, and `git diff --check`: passed.

# Phase 3.5.3d delete isolation evidence

Phase 3.5.3d freezes concurrent, stale-generation, and transport-loss behavior
around the Phase 3.5.3c exact-ID delete authority. The shared WorkspaceSession
runtime owns the active gate. Its key is the current WorkspaceSession
generation, current app-server connection generation, and exact
`CodexThreadKey`.

## RED evidence

The first focused test compile failed because the shared runtime had no delete
admission result, rejection source/reason schema, active same-key gate, or typed
pre-dispatch rejection API. The tests were retained and driven through fake
session/transport authority before implementation.

## Frozen behavior

- two same-key intents receive distinct attempt IDs, with at most one upstream
  dispatch and one local pre-dispatch duplicate rejection;
- different exact Thread keys remain independently dispatchable;
- transport loss before the atomic app-server write boundary cancels the write,
  records zero dispatch, and never creates mutation uncertainty;
- loss after the boundary cannot cancel the shared task and records outcome
  unknown only until direct success or rejection arrives;
- direct current-generation app-server evidence supersedes transport unknown,
  while confirmed success or authoritative upstream rejection cannot be
  downgraded by late transport loss;
- terminal outcomes release the gate; a later explicit intent after unknown has
  a new UUID and does not rewrite the earlier attempt;
- new WorkspaceSession/app-server generations inherit no gate, attempt, or
  pending mutation, and reconnect/restart never replays delete;
- only confirmed direct evidence reaches exact idempotent tombstone
  reconciliation; projection absence/presence is not delete authority.

Remote correlation continues to use `RemoteRequestProvenance`; no second
transport-provenance model is introduced. Rejection annotations distinguish
local pre-dispatch rejection from upstream rejection and retain a minimal typed
reason. Retry and replay counts are zero.

## Verification boundary

All delete requests use deterministic fake app-server sessions, fake daemon
transports, synthetic IDs, sanitized fixtures, and temporary test state. No real
Thread, rollout, user data, or frozen A3 target is deleted. The suite proves
CodexMonitor's local single-dispatch behavior; exact upstream concurrent-delete
winner/ordering remains unclaimed and would require a separately authorized
disposable isolation capture.

## Fresh closeout verification

- focused delete observation, multi-client, generation, transport-loss,
  direct-rejection precedence, tombstone, and fixture tests: 76 passed, 0
  failed;
- dispatch-boundary coordination tests: 28 passed, 0 failed;
- daemon dispatch-correlation tests: 16 passed, 0 failed;
- Rust all-targets: 1,833 passed, 7 ignored, 0 failed;
- `cargo check --all-targets`, `cargo fmt --all -- --check`,
  `npm run typecheck`, and `git diff --check`: passed.

No real delete E2E was executed. Phase 3.5.3e remains not started.

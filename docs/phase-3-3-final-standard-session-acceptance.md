# Phase 3.3 — Final Standard Session Acceptance

Status: **PASS / COMPLETE**. Phase 3.3.0 is **FORENSICS COMPLETE**; Phase 3.3.1, Phase 3.3.2, Phase 3.3.3a, Phase 3.3.3b, and Phase 3.3.3c are **PASS / FROZEN**. Phase 3.3.3 Effective Execution Settings Evidence is **PASS / COMPLETE**. Phase 3.4.0 is **FORENSICS COMPLETE**, Phase 3.4.1 is **PASS / FROZEN**, and Phase 3.4.2 is **GO / NOT STARTED**.

The sanitized authoritative evidence is `docs/evidence/phase-3-3-final/standard-session.json`. Raw rollout observations remain only in the ignored `.codexmonitor/phase-3-3-final-acceptance/` run directory.

## Accepted lifecycle

One explicit Monitor creation action produced one valid server Thread ID, one persisted rollout, and one completed first Turn. The same exact Thread ID was then continued in Desktop and CLI, producing one distinct completed Turn per Surface. No duplicate canonical Thread was observed.

| Gate | Evidence | Result |
| --- | --- | --- |
| Monitor creation acknowledgement | Exact server fullThreadId accepted by the production acknowledgement gate | PASS |
| Persistence | Authoritative rollout `session_meta` for the exact Thread | PASS |
| First Turn | Distinct authoritative fullTurnId; completed outcome | PASS |
| Monitor → Desktop idle continuation | Exact-ID resume after the Monitor writer released | PASS |
| Desktop → CLI while Desktop remained loaded | App-server error `-32600`, active writer | BLOCKED_BY_ACTIVE_WRITER |
| Desktop → CLI after Desktop fully exited | Exact-ID resume; prior Monitor and Desktop history visible; distinct CLI Turn completed | PASS |
| Post-release backend probe | Exact-ID `thread/resume`, with no prompt or Turn creation | PASS |
| Restart/reconstruction | Fresh exact-ID `thread/read`; Monitor UI showed the same Thread and all three Surface histories | PASS |
| Canonical identity | One fullThreadId, one rollout record, zero duplicate canonical Threads | PASS |

`Turn completed / idle` does not imply that a loaded Surface has released its writer. The first Desktop → CLI attempt correctly exercised occupancy protection. It is not a CLI continuation failure and was not bypassed with force takeover. Normal continuation succeeded after Desktop fully exited and released the writer.

## Independent facts

- Session exists: **CONFIRMED**.
- Session resumable by exact ID: **CONFIRMED** for Monitor → Desktop and Desktop → CLI after writer release.
- Persistence confirmed: **CONFIRMED** by authoritative `session_meta`.
- Ephemeral: **UNKNOWN**; it is not inferred from rollout timing or visibility.
- Workspace relation: ORIGIN cwd and all three TURN_EXECUTION cwd observations are preserved for the same Thread. Workspace identity does not enter the Thread ID.
- Desktop Project assignment: **UNKNOWN**.
- Desktop sidebar visibility: **NOT OBSERVED**.
- Monitor list visibility after restart: **CONFIRMED**.
- Remote discoverability: **UNKNOWN / NOT TESTED**.

Desktop Project assignment, sidebar visibility, and remote discovery are not Session creation success conditions and are not inferred from cwd, root membership, history visibility, or Thread identity.

## Settings and recovery boundaries

The Monitor, Desktop, and CLI Turns retain independent persisted execution settings under their distinct fullTurnIds. Different Turn settings do not alter the shared CodexThreadKey or create a Thread identity conflict.

Restart reconstruction recovers persisted `turn_context` observations. Process-local requested evidence and app-server notification evidence are **NOT RECOVERABLE BY CURRENT CONTRACT** and are not reconstructed from nearby values or event order.

The creation dispatch count was not directly instrumented in this run. The bounded evidence is one explicit user creation action, one production acknowledgement, one canonical fullThreadId, and one rollout. This does not expand the Phase 3.3.2 guarantee beyond client-side at-most-once dispatch per intent; server-side idempotency and end-to-end exactly-once remain unsupported claims.

## Privacy and read-only boundary

The committed evidence contains no prompt, assistant response, reasoning, credentials, cookies, tokens, private file contents, or raw local paths. The harness did not write Desktop SQLite, global state, catalogs, Project assignments, or sidebar state. It did not operate on an existing Thread.

## Verification

- Acceptance collector: 10 passed.
- Evidence validator: PASS.
- Creation acknowledgement: 18 passed.
- Creation coordination: 28 passed.
- Execution settings: 38 passed / 1 ignored real-runtime test.
- Phase 3.3.3c focused: 2 passed.
- Workspace interoperability: 77 passed / 1 ignored real-environment probe.
- Exact-ID: 5 passed.
- Global Source: 82 passed / 2 ignored manual probes.
- `cargo test --lib`: 470 passed / 4 ignored.
- daemon regression: 402 passed / 3 ignored.
- `cargo check --all-targets`: PASS.
- `cargo fmt --check`: PASS.
- `npm run typecheck`: PASS.
- Full frontend suite: 1106 passed; the approved six zh-CN locale/date baseline failures remain; new Phase 3.3 regressions = 0.
- `git diff --check`: PASS.

Phase 3.3 Monitor-created Standard Sessions is PASS / COMPLETE. This acceptance does not start Phase 3.4.

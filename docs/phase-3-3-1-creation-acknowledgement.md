# Phase 3.3.1 — Creation Acknowledgement Contract

Phase 3.2 is PASS / COMPLETE. Phase 3.3.0 is FORENSICS COMPLETE.
Phase 3.3.1 and Phase 3.3.2 Creation-intent / First-Turn Coordination are PASS / FROZEN.
Phase 3.3 Monitor-created Standard Sessions is PASS / COMPLETE. Phase 3.4 remains NOT STARTED.
Implementation commit: `ab4d9d7 feat: define standard session creation acknowledgement`.

## Independent facts

`THREAD_ACKNOWLEDGED != PERSISTENCE_CONFIRMED != FIRST_TURN_ACCEPTED != FIRST_TURN_OUTCOME`.
`CREATION_OUTCOME_UNKNOWN` is contract vocabulary only, not implemented recovery.

`CreationAcknowledgement` uses the existing `CodexThreadKey` and `CodexTurnKey`.
Its fields are private; consumers obtain facts through the shared validator and
explicit evidence application methods, not deserialization of arbitrary state.

| Fact | Immediately after valid thread/start | Independent evidence |
| --- | --- | --- |
| state | THREAD_ACKNOWLEDGED | Successful response with a valid full thread.id |
| persistence | NOT_YET_CONFIRMED | Matching persisted rollout session_meta observation |
| ephemeral | TRUE / FALSE / UNKNOWN | Boolean result.thread.ephemeral only |
| firstTurnAcceptance | NOT_YET_ACCEPTED | Explicit accepted Turn observation with matching Thread key |
| firstTurn | null | Accepted CodexTurnKey |
| firstTurnOutcome | UNKNOWN | Explicit completed / failed / interrupted / rejected observation |

CREATE_IN_FLIGHT, CREATION_FAILED, and CREATION_OUTCOME_UNKNOWN are vocabulary
for the creation boundary, not a persistent intent state machine. Decoded
invalid responses fail with CREATION_FAILED / INVALID_RESPONSE; server errors,
identity conflicts, and contradictory evidence have distinct failure reasons.
Transport errors remain transport errors. This slice does not classify ambiguous
transport outcomes, reconcile timeouts, or attempt discovery after disconnect.

## Identity validation

Current runtime full IDs are non-nil, 36-byte hyphenated UUIDs. Validation does
not restrict the UUID version, lowercase the ID, trim it, abbreviate it, or
generate a replacement. Missing, null, non-string, malformed, and ambiguous
response identities fail closed. An explicitly supplied expected Thread key
must match both the Codex-home namespace and full ID.

This format policy applies only to new creation acknowledgements. Frozen
Phase 3.1 exact-ID read/resume validation is unchanged. A future upstream ID
format change requires an explicit versioned contract update, not silent repair.

## Persistence and ephemeral

The pure evidence method accepts `CodexHomeIdentity` and `SessionMetaRecord`
from the existing persisted rollout reader/parser. Both home and Thread ID must
match the acknowledgement. A filename, start-response path, UI presence,
Desktop metadata, non-ephemeral flag, or accepted Turn is not persistence proof.
No second reader, SQLite access, scan, or automatic thread/read is introduced.

Absent, null, or non-boolean ephemeral fields remain UNKNOWN. Persisted rollout
evidence does not rewrite that field to FALSE. Confirmed ephemeral TRUE plus
claimed persistence is an evidence conflict and is rejected without mutation.
A standard-persisted classification requires confirmed persistence and no
explicit ephemeral TRUE; persistence can be known while ephemeral remains
UNKNOWN.

## Thin integration

`start_thread_core` reads the existing runtime's resolved Codex-home identity
through a read-only accessor and delegates one request to the acknowledgement
boundary. App and daemon already share this core. The upstream request is
unchanged: thread/start with cwd and approvalPolicy=on-request.

The existing result.thread and other server response fields are retained.
`result.creationAcknowledgement` is an additive Monitor IPC field. Its initial
snapshot never claims persistence or a first Turn. The frontend IPC type is
updated, and the creation hook checks acknowledgement/response ID agreement
before activation. Missing acknowledgements fail closed, including a stale
backend response; they do not trigger a compatibility retry.

The returned snapshot is not a live registry. Persistence and first-Turn evidence
application are pure contract APIs, not newly installed event subscriptions or
background discovery. Runtime storage and creation-intent coordination are not
implemented in this slice.

The only workspace-interoperability change is a read-only namespace accessor.
Root resolution, ORIGIN direct cwd, scoped Turn relations, effective selection,
and derived routing-cache semantics are unchanged. WorkspaceEntry.id, settings,
display names, Projects, and sidebar state never enter the canonical Thread key.

## Separation and failures

The creation boundary dispatches once and has no name, Turn, Project, sidebar,
or persistence-repair side effects. Only an explicit send uses turn/start with
the existing full ID. A rejected first send has no accepted Turn; a failed or
interrupted accepted Turn retains its Turn key. All preserve the acknowledged
Thread identity and do not create a replacement. Foreign/later Turn evidence
cannot replace the first Turn key.

Requested, server-effective, and persisted settings remain distinct. No model,
effort, approval, sandbox, or network policy is redesigned here.

## Verification boundary

TDD observed 16 failing shared-core tests before GREEN and four failing frontend
acknowledgement-admission tests before GREEN. Frozen admission/resume, workspace,
and app-server tests are included in the regression run.

No real Session was created or resumed for this slice. No Desktop private state
was read or written by the new implementation. Historical TURN_EXECUTION remains
NOT RECOVERABLE BY CURRENT CONTRACT when recovery evidence is absent.

The additional full frontend run exposes six existing locale-dependent failures:
five in useTraySessionUsage and one in Home. The same six failures reproduce on
an unmodified archive of HEAD 1530ab2 with the same dependencies and zh-CN default
locale. They expect English relative-time/date strings. No unrelated UI or test
locale change is included in Phase 3.3.1. The approved baseline waiver remains:
pre-existing, non-blocking; new Phase 3.3.1 regressions = 0.

| Verification | Result |
| --- | --- |
| Creation acknowledgement focused | 18 passed |
| shared::codex_core | 45 passed |
| Phase 3.1 external admission | 10 passed |
| Phase 3.1 exact_thread_ | 5 passed |
| Workspace interoperability | 77 passed, 1 ignored |
| App-server adapter | 20 passed |
| cargo test --lib | 403 passed, 3 ignored |
| Creation/actions/messaging integration and IPC wrappers | 170 passed |
| npm run test (additional full frontend run) | 1095 passed, 6 baseline locale failures |
| cargo check --all-targets | PASS |
| cargo fmt --check | PASS |
| npm run typecheck | PASS |
| git diff --check | PASS |

Phase 3.3.1 remains accepted and frozen with the approved baseline waiver; this is not
a claim that the unrelated full frontend baseline is green. Phase 3.3 Final Standard
Session Acceptance is PASS / COMPLETE. Phase 3.4 remains not started.

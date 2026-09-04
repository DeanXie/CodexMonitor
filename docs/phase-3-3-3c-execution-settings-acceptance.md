# Phase 3.3.3c — Effective Execution Settings Acceptance

Status: **PASS / FROZEN**. Phase 3.3.3 Effective Execution Settings Evidence is **PASS / COMPLETE**. The current/default naming mismatch is **RESOLVED**. Phase 3.3 Final Session Acceptance is **GO / NOT STARTED**. Phase 3.4 remains **NOT STARTED**.

## Accepted evidence chain

The isolated acceptance used Codex CLI/app-server `0.153.1` and only newly created test Threads. Monitor production shared-core code created a Thread and three Turns. The rollout watcher reconstructed each Turn from its persisted `turn_context`. A compatible CLI then resumed the exact full Thread ID and appended a fourth, independently scoped Turn.

The real runtime established all three evidence layers without promoting one into another:

- Monitor `turn/start` parameters produced `requested` evidence after response correlation supplied the full Turn ID.
- `thread/start` response and a real `thread/settings/updated` notification produced `serverEffective` evidence at `THREAD_DEFAULT` scope.
- rollout `turn_context` produced `persistedObserved` evidence at `TURN_EXECUTION { fullTurnId }` scope.

The tested app-server did not return Turn settings in `turn/start` responses. Turn-scoped server-effective evidence is therefore `NOT OBSERVED`; requested and persisted-observed evidence remain independently valid. The real `thread/settings/updated` notification had only a Thread ID, so it remains `THREAD_DEFAULT` and is not assigned to a Turn.

## Gate results

| Gate | Evidence | Result |
| --- | --- | --- |
| A1 explicit model/effort | Requested `gpt-5.6-terra` / `low`; matching rollout observation | PASS |
| A2 explicit null | Monitor emitted JSON null; rollout observed inherited concrete values | PASS (`MISMATCH`, not omitted) |
| A3 omitted model/effort | Normal Monitor builder always emits null or concrete values | NOT TESTABLE on real Monitor path; fixture PASS |
| B full-access | `never` + `danger-full-access`; model, effort, policy, sandbox, cwd matched | PASS |
| B read-only | `on-request` + `read-only`; policy, sandbox, cwd matched | PASS |
| B current/default | Actual request is fixed `on-request` + `workspace-write` + network true + workspace writable root; UI names the fixed preset accurately | PASS |
| C roots | Server thread settings reported `writableRoots=[]`; thread start separately reported `runtimeWorkspaceRoots=[workspace]` | PASS with relationship `UNKNOWN` |
| D CLI continuation | Same full Thread ID; new full Turn ID; CLI model/effort/policy/sandbox persisted independently | PASS |
| E settings notification | Real notification without Turn ID ingested as `THREAD_DEFAULT` | PASS |
| F assessments | Fixture covers `UNKNOWN`, `REQUESTED_ONLY`, `EFFECTIVE_CONFIRMED`, `OBSERVED_CONFIRMED`, `MATCH`, `MISMATCH`, `CONFLICT` | PASS |
| G restart/reconstruction | rollout observations reconstruct; process-local requested/effective-notification evidence does not | PASS with `NOT RECOVERABLE` boundary |

`OVERRIDDEN` remains a mismatch reason/provenance value, never an assessment state.

## Field matrix

| Source / Turn | Model | Effort | Approval | Sandbox | Network | Writable roots | cwd |
| --- | --- | --- | --- | --- | --- | --- | --- |
| thread/start server-effective | `gpt-5.6-sol` | `medium` | `on-request` | `workspace-write` | false | `[]` | isolated workspace |
| Monitor full-access | `gpt-5.6-terra` | `low` | `never` | `danger-full-access` | UNKNOWN | UNKNOWN | isolated workspace |
| Monitor read-only, null model/effort | observed `gpt-5.6-terra` | observed `low` | `on-request` | `read-only` | UNKNOWN | UNKNOWN | isolated workspace |
| Monitor current/default | `gpt-5.6-terra` | `low` | `on-request` | `workspace-write` | true | requested workspace; not present in rollout | isolated workspace |
| CLI exact-ID continuation | `gpt-5.6-terra` | `medium` | `never` | `read-only` | UNKNOWN | UNKNOWN | isolated workspace |

The `thread/start` response also reported `runtimeWorkspaceRoots=[isolated workspace]`. `writableRoots` and `runtimeWorkspaceRoots` are retained as distinct protocol facts. No current evidence proves that one overrides, supplements, or normalizes the other, so no `MATCH`, `MISMATCH`, or `CONFLICT` is inferred between them.

## Defect and minimal fix

Real ingestion initially assessed sandbox policy as `MISMATCH` because app-server requests use camelCase enum values (`workspaceWrite`, `readOnly`, `dangerFullAccess`) while persisted rollout records use kebab-case values (`workspace-write`, `read-only`, `danger-full-access`). These values describe the same protocol policies. The ingestion boundary now canonicalizes only these three confirmed aliases. A focused RED regression reproduced the false mismatch; the minimal GREEN fix and real rerun produced `MATCH` for all three policies.

No assessment-enum, scope, creation, Thread identity, Workspace, access-mode, or UI behavior changed.

## Current/default product boundary

`current/default` does not inherit an existing Thread policy. It sends a fixed preset: approval `on-request`, sandbox `workspaceWrite`, network enabled, and the Monitor workspace as a requested writable root. Both user-visible access-mode entry points now name it `Workspace access (network enabled)` and describe those fixed semantics explicitly.

The internal access-mode value remains `current`. Persistence, shortcut/cycle semantics, the Rust request builder, and every outgoing request field are unchanged. True inheritance, if ever required, remains a separate product contract rather than an implication of this preset.

## Recovery and privacy boundaries

The process-level evidence runtime survives `WorkspaceSession` reconnects but not process restart. After restart, rollout `turn_context` can reconstruct persisted-observed Turn evidence. Requested request payloads and transient server-effective notifications are `NOT RECOVERABLE BY CURRENT CONTRACT` and are never inferred from persisted values.

The stored acceptance evidence is sanitized and contains no user prompt, assistant response, credentials, cookies, tokens, or private file contents. Desktop private SQLite/global-state/catalog data was not written or modified.

Canonical evidence: `docs/evidence/phase-3-3-3c/real-runtime.json`. Deterministic assessment fixture: `docs/fixtures/execution-settings/phase-3-3-3c-assessments.json`.

The remaining Phase 3.3 work is the final integrated Standard Session acceptance across creation acknowledgement, creation/first-Turn at-most-once coordination, exact-ID continuation, Workspace relations, and execution-settings evidence. The naming blocker is resolved, so Phase 3.3 Final Session Acceptance is GO / NOT STARTED. Phase 3.4 remains not started.

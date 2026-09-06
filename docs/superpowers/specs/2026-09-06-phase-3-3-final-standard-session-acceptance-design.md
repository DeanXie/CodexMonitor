# Phase 3.3 Final Standard Session Acceptance Design

## Goal

Prove the complete lifecycle of one newly created Monitor Standard Session across Monitor, Desktop, and CLI without modifying Desktop private state, operating on an existing Thread, or inferring identity from similarity.

## Frozen boundaries

- `CodexThreadKey`, Workspace identity, Desktop Project projection, creation acknowledgement, creation/first-Turn coordination, and execution-settings evidence retain their Phase 3.1–3.3.3 semantics.
- The run uses one new main Thread. Existing Threads are never read or resumed by the harness.
- Raw runtime observations stay below the ignored `.codexmonitor/phase-3-3-final-acceptance/` directory. Committed evidence contains no prompt, assistant response, reasoning, credentials, cookies, tokens, or private file contents.
- The collector only reads rollout files and invokes exact-ID app-server `thread/read` / `thread/resume` probes. It never writes Desktop SQLite, global-state, catalog, Project assignment, or sidebar state.
- Missing evidence is reported as `UNKNOWN`, `NOT OBSERVED`, or `NOT RECOVERABLE`; it is never reconstructed from title, prompt, cwd, timing proximity, or a recent event.

## Correlation

Preparation records every rollout path present before the acceptance action. After the user confirms the Monitor action, discovery accepts the result only when exactly one new rollout contains a valid authoritative `session_meta.id`. Zero or multiple candidates fail closed. Once discovered, every later operation is locked to that exact fullThreadId, and Turn observations are keyed only by authoritative `turn_context.turn_id` / task event turn IDs.

Each manual Surface step has an explicit before/after checkpoint. A newly observed Turn is attributed to that step only when it is the sole new fullTurnId for the locked Thread between those checkpoints. Ambiguous deltas stop the run.

## Acceptance sequence

1. Monitor creates the new Thread and completes its first Turn.
2. The collector locks the exact fullThreadId, confirms persisted `session_meta`, captures the Monitor Turn, and verifies it is idle.
3. Desktop resumes that exact ID only while idle and completes one Turn.
4. CLI resumes that exact ID only after the Desktop Turn ends and completes one Turn.
5. The dedicated occupancy Gate starts a deliberately long-running Turn on one Surface; an exact-ID resume probe from another Surface must return `BLOCKED_BY_ACTIVE_WRITER`. This is protection, not a continuation failure.
6. After the writer releases, an exact-ID read/resume probe and Monitor restart/list reconstruction must preserve the same Thread ID and recover only evidence supported by the current contract.

Normal idle continuation and active-writer protection are separate gates. No normal continuation is attempted while another Surface owns the writer.

## Automatic evidence collector

`scripts/phase-3-3-final-acceptance.mjs` provides:

- `prepare`: create an ignored run manifest and baseline rollout inventory;
- `capture <stage>`: discover/lock the Monitor Thread or capture a single exact-ID Turn delta after a confirmed user step;
- `probe <read|resume>`: run a bounded app-server exact-ID probe and store only sanitized outcome fields;
- `status`: print the current run state and the next admissible stage;
- `finalize`: emit a sanitized evidence document only when all required observations exist.

The collector is deterministic, idempotent per stage, and fail-closed. It never sends a prompt or `turn/start`.

## PASS conditions

- One Monitor `thread/start` acknowledgement corresponds to one valid fullThreadId and one persisted rollout.
- The first Turn exists and has a distinct fullTurnId; persistence and first-Turn outcome remain separate evidence.
- ORIGIN cwd and each Turn cwd/settings are preserved without changing Thread identity.
- Monitor → Desktop and Desktop → CLI idle exact-ID continuation succeed with the same fullThreadId and new distinct fullTurnIds.
- No duplicate canonical Thread is observed.
- The occupancy Gate returns `BLOCKED_BY_ACTIVE_WRITER`, while a later idle exact-ID continuation/read succeeds.
- Restart/list reconstruction is deterministic and does not fabricate non-recoverable process-local evidence.
- Desktop Project/sidebar visibility remain independent observations and are not Session creation success conditions.

Any unproven required condition prevents Phase 3.3 PASS.

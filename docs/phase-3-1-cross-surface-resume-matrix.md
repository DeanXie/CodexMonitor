# Phase 3.1 Cross-Surface Resume Matrix

## Status

Phase 3.0 is FORENSICS COMPLETE. Phase 3.1.1, Phase 3.1.2, and Phase 3.1.2b are PASS. Phase 3.1 Shared Thread Identity / Resume is PASS / COMPLETE, and the six-way Cross-Surface Resume E2E is PASS. Phase 3.2 Project / Workspace Interoperability is GO / NOT STARTED.

All Gate evidence uses dedicated Phase 3.1.2/3.1.2b test Threads. No user development Thread was resumed. No force takeover, fork, replacement `thread/start`, Desktop private database edit, catalog edit, sidebar edit, rebase, force push, or history rewrite was performed.

## Resume matrix

| Gate | Direction | READ | RESUME | SAME THREAD ID | SAME ROLLOUT | NEW TURN ID | TOKEN CONTINUITY | DUPLICATE | PROJECT RETAINED | SIDEBAR VISIBLE | RESULT |
|---|---|---|---|---|---|---|---|---:|---|---|---|
| A | Desktop → Monitor | PASS | PASS when idle; `BLOCKED_BY_ACTIVE_WRITER` when occupied | PASS | PASS, appended after explicit send | PASS | PASS | 0 | NOT OBSERVED | UNKNOWN | PASS |
| B | Desktop → CLI | Auxiliary Monitor read PASS | PASS when idle; `BLOCKED_BY_ACTIVE_WRITER` when occupied | PASS | PASS, appended after explicit send | PASS | PASS | 0 | NOT OBSERVED | UNKNOWN | PASS |
| C | Monitor → Desktop | PASS | PASS behavior; protocol method observation `UNKNOWN` | PASS | PASS, appended | PASS | PASS | 0 | NOT OBSERVED; no project assigned | PASS | PASS |
| D | Monitor → CLI | Auxiliary Monitor read PASS | PASS in both version-matched interactive and exec modes | PASS | PASS, appended by exec after interactive load-only probe | PASS | PASS | 0 | NOT OBSERVED; no project assigned | PASS | PASS |
| E | CLI exec → Desktop | PASS | PASS behavior; protocol method observation `UNKNOWN` | PASS | PASS, appended | PASS | PASS | 0 | UNKNOWN | UNKNOWN | PASS |
| F | CLI exec → Monitor | PASS | PASS | PASS | PASS, appended | PASS | PASS | 0 | UNKNOWN | UNKNOWN | PASS |

For C and E, Desktop behavior proves exact-ID load without a Turn, followed by explicit continuation on the same Thread and rollout. The app-facing evidence does not expose the literal internal `thread/resume` method, so `protocol method observation = UNKNOWN`; this does not reduce behavioral interoperability from PASS.

`BLOCKED_BY_ACTIVE_WRITER` is a valid occupied-state outcome, not a capability failure. The idle A2/B2 probes prove the same producer Threads are resumable after the Desktop writer is released through the public archive/unarchive lifecycle.

## Identity, rollout, Turn, model, and token evidence

| Gate | Thread | Old Turn | New Turn | Bytes before → resume → after | Token observation before → after | Token evidence | Model |
|---|---|---|---|---|---:|---:|---|
| A2 | `01a05dd5-648d-7432-a4ad-e32d60474763` | `01a05dd5-6559-7781-b0cc-f981943e9c5d` | `01a05de1-9130-7ae0-841c-f86f965248aa` | 420318 → 420318 → 824140 | 28523 → 33002 | 33002 final observation | `gpt-5.6-sol` → `gpt-5.6-sol` |
| B2 | `01a05dd6-8ec0-77b1-9eaa-33d6fa3356fb` | `01a05dd6-8faf-79e3-90f3-e778d0be2a91` | `01a05de3-aee8-7fb2-a457-e6cc965481d9` | 420474 → 420474 → 813723 | 28576 → 31713 | 31713 final observation | `gpt-5.6-sol` → `gpt-5.6-sol` |
| C | `01a05d7e-4073-71d1-b171-d6c5a4f3367d` | `01a05d7e-6d7f-73f0-ab38-d86ab742aa9e` | `01a05d8e-702d-7ac2-893a-6ad338c03ee3` | 414327 → 414327 → 792441 | 22542 → 58435 | 35893 | `gpt-5.6-sol` → `gpt-5.6-sol` |
| D2 | `01a05de4-4098-7833-9deb-e3763e15f397` | `01a05de4-59d9-7533-8479-c44425d0f851` | `01a05deb-7c52-7863-9da7-5f5929bb0e9e` | 422796 → 422796 → 765358 | 22508 → 24250 | 24250 final observation | `gpt-5.6-sol` → `gpt-5.6-sol` |
| E | `01a05d80-0f19-73e0-91a7-c5e33888bcff` | `01a05d80-0fbe-7741-9baf-b7f5e3f87002` | `01a05d91-72d4-7ac3-8228-76979685a347` | 399955 → 399955 → 768359 | 19973 → 50976 | 31003 | `gpt-5.6-sol` → `gpt-5.6-sol` |
| F | `01a05d82-2f35-7011-839a-4678972aba23` | `01a05d82-2fda-70f0-9539-9665474b7f28` | `01a05d9b-bc5c-7ea0-8b2c-dabd1a5640d6` | 400512 → 400512 → 734681 | 19973 → 42733 | 22760 | `gpt-5.6-sol` → `gpt-5.6-sol` |

The A2, B2, and D2 token values are the canonical rollout `token_count` observations before and after continuation; they establish continuity without treating the final observation as an additive per-Turn value. Each closure Thread matched exactly one rollout file, so duplicate canonical Thread count is zero.

## Resume and execution separation

- A2 and F directly exercised Monitor `thread/read` followed by `thread/resume`. A2 proved that resume creates no Turn and changes no rollout byte position. Its explicit `send_user_message` then created the new Turn.
- C and E proved the same behavioral separation through Desktop: exact read, exact navigation/load with no Turn or append, then explicit send and a new Turn. The literal Desktop protocol method is NOT OBSERVED.
- B2 used the version-matched Desktop-bundled CLI. Exact interactive resume loaded the same session without appending; the explicit TUI prompt then created one new Turn.
- D2 tested interactive and exec entry points separately. Version-matched `codex resume <ID>` loaded the exact session without a Turn. Version-matched `codex exec resume <ID> <prompt>` continued it with one new Turn. No fallback Thread was created.

## Desktop Project, catalog, and sidebar observations

- Monitor-created C and D were present in the Desktop task list and sidebar, but `projectId` was null. Thread existence and resumability therefore remained separate from project assignment.
- C was behaviorally continued by Desktop despite lacking project assignment.
- A2 and B2 were dedicated projectless Desktop Threads. Project retention is not applicable; later sidebar membership was not used as a Resume precondition.
- E and F were not found in the bounded recent Desktop listing. Complete catalog membership and sidebar visibility remain UNKNOWN; absence from that bounded view is not treated as resume failure.
- No private SQLite, catalog, global state, project assignment, or sidebar state was modified.

## Active-writer conflict Gate

A dedicated Monitor instance started Turn `01a05d9f-6f73-7671-a56d-a9b8de84205c` on Thread `01a05d9f-38e5-7db0-85b7-8b82494a669d`. While the Turn was in progress, a second isolated Monitor instance:

1. completed exact-ID `thread/read` successfully;
2. attempted exact-ID `thread/resume`;
3. received error code `-32600`, `thread already has an active writer`;
4. did not take over, fork, interrupt, close the producer, or start a replacement Thread.

The producer completed naturally. Its rollout retained one canonical Thread and one Turn.

## Closure analysis

### A2 and B2: capability versus occupancy

- A completed `task_complete` and an idle/notLoaded presentation do not alone prove that Desktop has released the writer.
- While occupied, exact read succeeds and exact resume returns `-32600`, `thread already has an active writer`, without a Turn, takeover, fork, or replacement Thread. This is `BLOCKED_BY_ACTIVE_WRITER` and is correct occupancy protection.
- After public archive/unarchive released the Desktop writer, A2 resumed through Monitor and B2 resumed through the version-matched interactive CLI. Resume/load did not create a Turn; the subsequent explicit send did.
- Capability is PASS when idle. No Monitor source fix is indicated.

### D2: CLI version and product-mode result

- Global PATH command `codex` resolved to CLI `0.147.0`. Its interactive `codex resume <ID>` stopped at authentication UI, so exact-ID discovery was not reached. Its `codex exec resume <ID> <prompt>` exited 1 with `no rollout found`.
- The Desktop/Monitor runtime was CLI `0.151.0-alpha.7.2`. With that exact binary, `codex resume <ID>` and `codex exec resume <ID> <prompt>` both passed against the same D2 Thread.
- D2 existed once under current `CODEX_HOME/sessions`, was absent from `session_index.jsonl` and `history.jsonl`, and had source `vscode`, originator `Codex Desktop`, with no `thread_source` field. Both 0.151 modes still found it.
- Current exact-ID discovery therefore does not require history membership, index membership, source `exec`, interactive origin, or exec origin. The earlier/global failure is a versioned CLI authentication/discovery compatibility boundary, not a non-standard Monitor Thread or an interactive-versus-exec product-mode restriction.

## Decision

All six behavioral Resume directions are PASS. Occupied Desktop producers return `BLOCKED_BY_ACTIVE_WRITER`, which is correct behavior and does not reduce idle capability. C/E literal Desktop protocol-method observation remains `UNKNOWN` without reducing their behavior-level PASS.

No Phase 3.1.1 implementation defect was proven, so no source-code fix was made. Phase 3.1 is formally PASS / COMPLETE: Cross-Surface Resume six-way E2E is PASS, duplicate canonical Thread count is zero, active-writer protection is CONFIRMED, and the CLI version compatibility caveat is KNOWN.

Codex `0.151.0-alpha.7.2` passes both interactive resume and exec resume. Global PATH Codex `0.147.0` has interactive authentication/discovery UNKNOWN and exec resume `no rollout found`. This is a CLI version/history/protocol compatibility boundary, not a Monitor-created Thread identity failure; Phase 3.1 will not be changed to support the stale `0.147.0` binary.

Phase 3.2 Project / Workspace Interoperability is GO / NOT STARTED and is the next sole development starting point. This closeout does not begin Phase 3.2 implementation.

Original sanitized evidence remains in `docs/evidence/phase-3-1-2/`; closure evidence is stored in `docs/evidence/phase-3-1-2b/`.

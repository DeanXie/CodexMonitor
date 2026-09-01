# Phase 3.1 Cross-Surface Resume Matrix

## Status

Phase 3.0 is FORENSICS COMPLETE. Phase 3.1.1 is PASS. Phase 3.1.2 is NOT PASS: one Gate passed, two are partial, and three failed. Phase 3.2 has not started.

All Gate evidence uses new Phase 3.1.2 test Threads. No user development Thread was resumed. No force takeover, fork, replacement `thread/start`, Desktop private database edit, catalog edit, sidebar edit, rebase, force push, or history rewrite was performed.

## Resume matrix

| Gate | Direction | READ | RESUME | SAME THREAD ID | SAME ROLLOUT | NEW TURN ID | TOKEN CONTINUITY | DUPLICATE | PROJECT RETAINED | SIDEBAR VISIBLE | RESULT |
|---|---|---|---|---|---|---|---|---:|---|---|---|
| A | Desktop → Monitor | PASS | FAIL: active writer | PASS | PASS, unchanged | NOT OBSERVED | PARTIAL: unchanged only | 0 | NOT OBSERVED | UNKNOWN | FAIL |
| B | Desktop → CLI | NOT OBSERVED in CLI; auxiliary Monitor read PASS | FAIL: active writer | PASS | PASS, unchanged | NOT OBSERVED | PARTIAL: unchanged only | 0 | NOT OBSERVED | UNKNOWN | FAIL |
| C | Monitor → Desktop | PASS | PARTIAL: exact-ID load observed; literal method not exposed | PASS | PASS, appended | PASS | PASS | 0 | NOT OBSERVED; no project assigned | PASS | PARTIAL |
| D | Monitor → CLI | NOT OBSERVED in CLI | FAIL: no rollout found | PASS | PASS, unchanged | NOT OBSERVED | PARTIAL: unchanged only | 0 | NOT OBSERVED; no project assigned | PASS | FAIL |
| E | CLI exec → Desktop | PASS | PARTIAL: exact-ID load observed; literal method not exposed | PASS | PASS, appended | PASS | PASS | 0 | UNKNOWN | UNKNOWN | PARTIAL |
| F | CLI exec → Monitor | PASS | PASS | PASS | PASS, appended | PASS | PASS | 0 | UNKNOWN | UNKNOWN | PASS |

`PARTIAL` for the Desktop consumer is deliberately conservative. Desktop changed the exact Thread from `notLoaded` to `idle`, produced no Turn or rollout append during load, and then continued the same Thread after an explicit send. The app-facing evidence does not expose the literal internal `thread/resume` method, so protocol-method proof is NOT OBSERVED.

## Identity, rollout, Turn, model, and token evidence

| Gate | Thread | Old Turn | New Turn | Bytes before → resume → after | Cumulative token before → after | New Turn token | Model |
|---|---|---|---|---|---:|---:|---|
| A | `01a05ccf-9886-7f33-b642-6534e371d1d0` | `01a05ccf-996a-71f0-9be5-609e522488d1` | NOT OBSERVED | 423696 → 423696 → 423696 | 29782 → 29782 | NOT OBSERVED | `gpt-5.6-sol` → `gpt-5.6-sol` |
| B | `01a05ccf-bc65-7b11-8f76-9b22621149f3` | `01a05ccf-bd4c-71c3-8255-a3dc38053a1b` | NOT OBSERVED | 423592 → 423592 → 423592 | 29771 → 29771 | NOT OBSERVED | `gpt-5.6-sol` → `gpt-5.6-sol` |
| C | `01a05d7e-4073-71d1-b171-d6c5a4f3367d` | `01a05d7e-6d7f-73f0-ab38-d86ab742aa9e` | `01a05d8e-702d-7ac2-893a-6ad338c03ee3` | 414327 → 414327 → 792441 | 22542 → 58435 | 35893 | `gpt-5.6-sol` → `gpt-5.6-sol` |
| D | `01a05d7e-8d11-7ab3-b1f1-32f97ce5014c` | `01a05d7e-b521-7f53-8020-85bed5818cb2` | NOT OBSERVED | 414327 → 414327 → 414327 | 22542 → 22542 | NOT OBSERVED | `gpt-5.6-sol` → `gpt-5.6-sol` |
| E | `01a05d80-0f19-73e0-91a7-c5e33888bcff` | `01a05d80-0fbe-7741-9baf-b7f5e3f87002` | `01a05d91-72d4-7ac3-8228-76979685a347` | 399955 → 399955 → 768359 | 19973 → 50976 | 31003 | `gpt-5.6-sol` → `gpt-5.6-sol` |
| F | `01a05d82-2f35-7011-839a-4678972aba23` | `01a05d82-2fda-70f0-9539-9665474b7f28` | `01a05d9b-bc5c-7ea0-8b2c-dabd1a5640d6` | 400512 → 400512 → 734681 | 19973 → 42733 | 22760 | `gpt-5.6-sol` → `gpt-5.6-sol` |

For C, E, and F, cumulative after equals cumulative before plus the new Turn token. The new Turn token is not added twice. Every Gate matched exactly one rollout file, so duplicate canonical Thread count is zero for A–F.

## Resume and execution separation

- A and F directly exercised Monitor `thread/read` followed by `thread/resume`. F proved that resume creates no Turn and changes no rollout byte position. Its explicit `send_user_message` then created the new Turn.
- C and E proved the same behavioral separation through Desktop: exact read, exact navigation/load with no Turn or append, then explicit send and a new Turn. The literal Desktop protocol method is NOT OBSERVED.
- B's CLI has no separate `thread/read` command. Exact interactive resume reached the active-writer rejection before any Turn was created.
- D's interactive CLI entered an authentication UI despite `codex login status` reporting ChatGPT login. The additional exact-ID `codex exec resume` reached `thread/resume` but returned `no rollout found`; it did not create a Turn. No fallback Thread was created.

## Desktop Project, catalog, and sidebar observations

- Monitor-created C and D were present in the Desktop task list and sidebar, but `projectId` was null. Thread existence and resumability therefore remained separate from project assignment.
- C was behaviorally continued by Desktop despite lacking project assignment.
- A and B were created against the saved CodexMonitor project. Later retention and sidebar membership were not directly observed and remain UNKNOWN.
- E and F were not found in the bounded recent Desktop listing. Complete catalog membership and sidebar visibility remain UNKNOWN; absence from that bounded view is not treated as resume failure.
- No private SQLite, catalog, global state, project assignment, or sidebar state was modified.

## Active-writer conflict Gate

A dedicated Monitor instance started Turn `01a05d9f-6f73-7671-a56d-a9b8de84205c` on Thread `01a05d9f-38e5-7db0-85b7-8b82494a669d`. While the Turn was in progress, a second isolated Monitor instance:

1. completed exact-ID `thread/read` successfully;
2. attempted exact-ID `thread/resume`;
3. received error code `-32600`, `thread already has an active writer`;
4. did not take over, fork, interrupt, close the producer, or start a replacement Thread.

The producer completed naturally. Its rollout retained one canonical Thread and one Turn.

## Failure analysis

### A and B: Desktop writer retention

- Expected: an idle persisted Desktop Thread can be read and resumed by Monitor or CLI, with no Turn until explicit continuation.
- Actual: exact read succeeded where available, but both consumers received `already has an active writer` with code `-32600`.
- Rollout evidence: file identity, byte position, old Turn, model, and cumulative token were unchanged; no new Turn appeared.
- State evidence: the Desktop creator Turn was complete, while official persistence still reported an active writer.
- Root cause classification: Codex product boundary. Phase 3.1.1 correctly passed the exact full Thread ID and did not call `thread/start`.
- Minimal fix: none in CodexMonitor. A safe official unload/writer-release mechanism would be required; force takeover is prohibited.

### D: Monitor-created Thread not discoverable by CLI resume

- Expected: CLI exact-ID resume finds the persisted rollout produced by standard Monitor `thread/start` plus `turn/start`.
- Actual: interactive CLI diverged into authentication UI; exact-ID `codex exec resume` returned `no rollout found for thread id` with code `-32600`.
- Rollout evidence: the rollout existed at one identity, retained one old Turn, and remained byte-for-byte at the same append position after both attempts.
- State evidence: Desktop indexed and displayed the Thread with null project assignment; Desktop exact read also found it.
- Root cause classification: Codex product boundary or upstream CLI discovery limitation; the deeper upstream cause is UNKNOWN because no matching upstream Codex checkout was available.
- Minimal fix: none proven in CodexMonitor. Changing Phase 3.1.1 would be unjustified without upstream evidence that Monitor emitted a non-standard start contract.

## Decision

Monitor can directly resume and continue a CLI-exec Thread. Desktop can behaviorally load and continue Monitor-created and CLI-exec Threads, though the literal Desktop resume method is not exposed by the available evidence surface. Monitor and CLI cannot resume Desktop-created Threads while Desktop retains the writer. CLI could not resume the Monitor-created test Thread.

No Phase 3.1.1 implementation defect was proven, so no source-code fix was made. Phase 3.1 cannot be formally marked PASS, and the prerequisites for Phase 3.2 Project / Workspace Interoperability are not yet satisfied. Phase 3.2 remains NOT STARTED pending user confirmation and resolution or acceptance of the observed product boundaries.

Sanitized machine-readable evidence is stored in `docs/evidence/phase-3-1-2/`.

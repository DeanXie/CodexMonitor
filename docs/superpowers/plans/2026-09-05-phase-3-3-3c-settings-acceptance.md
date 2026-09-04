# Phase 3.3.3c Execution Settings Acceptance Plan

**Goal:** Validate the frozen Phase 3.3.3a/b execution-settings evidence contracts against isolated Monitor/app-server/CLI scenarios, preserve sanitized evidence, and classify product semantics or recovery limits without entering Phase 3.4.

**Architecture boundary:** Reuse the production `codex_core`, app-server reconciliation, rollout watcher, and shared evidence reducer. Add only acceptance fixtures, a bounded real-runtime probe, and canonical acceptance documentation. If a Phase 3.3.3a/b defect is proven, add a focused failing test and the smallest shared-core fix before rerunning affected gates.

**Tech stack:** Rust/Tokio shared backend tests, JSON/JSONL fixtures, Codex app-server protocol, Cargo and npm validation.

---

### Task 1: Establish clean baseline and acceptance harness

- Confirm the isolated branch contains no unrelated changes.
- Run the focused Phase 3.3.3a/b baseline tests.
- Add a test-only acceptance harness that can observe production ingestion results without adding a production IPC/UI surface.

### Task 2: Capture isolated real runtime evidence

- Create only new dedicated test Threads/Turns through the Monitor production request path.
- Exercise explicit model/effort, explicit null, full-access, read-only, and current/default request shapes.
- Capture app-server effective settings, `thread/settings/updated`, and rollout `turn_context` observations.
- Continue one acknowledged test Thread with a compatible CLI by exact full Thread ID when available.
- Never infer omitted values, correlations, runtime root semantics, or lost process-local requested evidence.

### Task 3: Encode deterministic contract fixtures and gates

- Save sanitized protocol/rollout evidence and a Gate A-G matrix.
- Cover every assessment state with provenance-preserving deterministic fixture tests.
- Verify Thread-default notifications remain independent from Turn execution evidence.
- Verify restart/reconstruction reports process-local requested evidence as not recoverable instead of fabricating it.

### Task 4: Characterize product boundaries

- Record actual current/default outgoing policy as fixed preset or inherited behavior.
- Record writable-root, runtime-root, sandbox, network, cwd, and cross-surface findings without claiming undocumented causality.
- If current/default is a fixed preset, document bounded implementation and naming options plus a recommendation; do not change behavior in this slice.

### Task 5: Verify and report

- Run Phase 3.3.3c focused tests and all requested regressions/checks.
- Confirm Desktop private state remains untouched and no user/historical Thread was modified.
- Report PASS only for observed or executable fixture-backed gates; use `UNKNOWN`, `NOT TESTABLE`, or `NOT RECOVERABLE` where evidence is absent.
- Stop before Phase 3.4.

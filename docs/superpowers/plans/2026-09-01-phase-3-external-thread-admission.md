# Phase 3 External Thread Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first Phase 3 external-thread admission and exact-ID read/resume contract without adding resume/control-plane semantics to the frozen Phase 2 Global Source canonical observation model.

**Architecture:** A focused `shared/codex_core/external_thread_admission.rs` module owns Phase 3 capability state keyed by the existing `CodexThreadKey`. `shared/codex_core.rs` owns exact-ID `thread/read` and `thread/resume` request construction and response validation; app and daemon remain thin adapters. Phase 2 types may be consumed as observation inputs, but Global Source Registry and canonical observations are not extended.

**Tech Stack:** Rust, Tokio, Serde JSON, existing Tauri/app-server adapters.

**Spec:** `CodexMonitor_四阶段开发路线图.md` Phase 3 plus the user-approved 2026-09-01 architecture amendment in this task.

## Global Constraints

- `THREAD_LIST_SOURCE_KINDS` includes `exec` without treating source kind as Producer Surface authority.
- `thread/read` and `thread/resume` use one exact-ID request builder and validate successful response `thread.id` against the requested full thread ID.
- Resume never falls back to `thread/start`; `thread/resume` is not `turn/start`.
- A `CodexThreadKey` identifies one admission record across Surface projections; equal titles never merge different IDs.
- Tombstones override lower-authority evidence.
- Workspace assignment reuses longest-root matching; equal-best ambiguity stays unassigned.
- Writer/occupancy, `projectAssigned`, and `sidebarVisible` stay `UNKNOWN` without direct evidence; cwd/catalog presence is not evidence for the latter two.
- No force takeover, real six-way Resume E2E, or Phase 3.2 work.

---

### Task 1: Exact-ID read/resume boundary

**Files:**
- Modify: `src-tauri/src/shared/codex_core.rs`

**Interfaces:**
- Produces: `build_exact_thread_request(method: ExactThreadMethod, thread_id: &str) -> Result<ExactThreadRequest, String>`
- Produces: `validate_exact_thread_response(requested_thread_id: &str, response: Value) -> Result<Value, String>`

- [ ] **Step 1: Write six failing behavior tests**

```rust
thread_list_includes_exec_source_kind()
exact_thread_read_builds_only_thread_read_with_full_id()
exact_thread_resume_builds_only_thread_resume_with_full_id()
exact_thread_request_rejects_blank_full_id()
exact_thread_response_rejects_missing_or_mismatched_full_id()
exact_thread_response_uses_the_normalized_requested_full_id()
```

The request tests assert literal method and JSON payload values. The response test covers both missing and mismatched `thread.id`; changing validation to accept either makes it fail.

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test --lib shared::codex_core::tests::exact_thread -- --nocapture`
Expected: compile/test failure because the exact-ID builder and validation behavior do not exist and `exec` is absent.

- [ ] **Step 3: Implement the minimal boundary**

```rust
enum ExactThreadMethod { Read, Resume }
struct ExactThreadRequest { method: &'static str, params: Value }
```

Route both core functions through the builder and validate successful responses before returning. Do not add a `thread/start` or `turn/start` branch.

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test --lib shared::codex_core::tests::exact_thread -- --nocapture`
Expected: PASS.

### Task 2: Minimal Phase 3 admission state

**Files:**
- Create: `src-tauri/src/shared/codex_core/external_thread_admission.rs`
- Create: `src-tauri/src/shared/codex_core/external_thread_admission_tests.rs`
- Modify: `src-tauri/src/shared/codex_core.rs`

**Interfaces:**
- Consumes: `CodexThreadKey`, Phase 2 `WorkspaceAssignment`, and direct Surface projection evidence.
- Produces: `ExternalThreadAdmissionRegistry::observe(ExternalThreadAdmissionEvidence) -> &ExternalThreadAdmissionState`
- Produces: `ExternalThreadAdmissionRegistry::get(&CodexThreadKey) -> Option<&ExternalThreadAdmissionState>`

- [ ] **Step 1: Write ten failing behavior tests**

```rust
same_full_id_from_multiple_surfaces_has_one_admission_record()
same_title_with_different_full_ids_stays_independent()
tombstone_overrides_exists_and_resumable_evidence()
confirmed_exact_read_marks_exists_and_resumable()
unavailable_read_keeps_exists_and_resumable_unknown()
workspace_assignment_preserves_longest_root_match()
ambiguous_equal_longest_roots_remain_unassigned()
writer_and_occupancy_stay_unknown_without_direct_evidence()
surface_projection_evidence_sets_project_and_sidebar_state()
cwd_or_catalog_presence_cannot_infer_project_or_sidebar_state()
```

Each expected enum value is a hand-written literal. The tests exercise registry behavior rather than private fields or source text.

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test --lib external_thread_admission -- --nocapture`
Expected: compile failure because the Phase 3 module and state types do not exist.

- [ ] **Step 3: Implement the minimal state container**

```rust
struct ExternalThreadAdmissionState {
    thread_key: CodexThreadKey,
    exists: KnowledgeState,
    resumable: KnowledgeState,
    writer_occupancy: WriterOccupancy,
    workspace_assignment: WorkspaceAssignment,
    surface_projections: BTreeMap<Surface, SurfaceProjection>,
    project_assigned: KnowledgeState,
    sidebar_visible: KnowledgeState,
}
```

Merge only records with identical `CodexThreadKey`. Apply tombstone first. Accept project/sidebar facts only from direct Surface projection evidence. Leave writer/occupancy unknown unless explicitly observed; do not implement mutation/takeover APIs.

- [ ] **Step 4: Run tests to verify GREEN**

Run: `cargo test --lib external_thread_admission -- --nocapture`
Expected: PASS.

### Task 3: Documentation and complete verification

**Files:**
- Modify: `docs/app-server-events.md`
- Modify: `CodexMonitor_四阶段开发路线图.md`

**Interfaces:**
- Documents the Phase 2/Phase 3 boundary and actual `thread/read` support.

- [ ] **Step 1: Update canonical live-state documentation**

Record the Phase 3.1 boundary, 16-test contract, and deliberate exclusions. Move `thread/read` from missing to supported requests. Do not claim real six-way E2E or Phase 3 completion.

- [ ] **Step 2: Run targeted and complete verification**

Run: `cargo test --lib external_thread_admission -- --nocapture`
Run: `cargo test --lib shared::codex_core::tests -- --nocapture`
Run: `npm run typecheck`
Run: `npm run test`
Run: `cd src-tauri && cargo check`

- [ ] **Step 3: Inspect delivery boundaries**

Run: `git status --short`
Run: `git diff --check`
Run: `git diff --name-status`
Run placeholder and accidental-scope-activation scans in touched files. Confirm the pre-existing `src-tauri/src/main.rs` and launcher-script changes are untouched.

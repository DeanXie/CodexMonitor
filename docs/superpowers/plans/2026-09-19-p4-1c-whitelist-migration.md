# P4.1c Controlled Whitelist Migration Engine Implementation Plan

**Goal:** Add a deterministic, source-read-only migration engine that prepares
sanitized legacy settings and workspaces in staging without activating the
DeanX runtime identity.

**Architecture:** Put the engine in `src-tauri/src/shared/migration_core.rs` and
tests in `src-tauri/src/shared/migration_core_tests.rs`. Keep it unregistered
from App, daemon, and startup surfaces. Use typed deserialization as the field
allowlist, raw JSON only to enumerate unknown paths, and a marker/state manifest
to constrain staging and rollback ownership.

**Tech stack:** Rust, serde/serde_json, std filesystem APIs, existing Node
version-authority tooling.

---

### Task 1: Freeze the contract with failing tests

**Files:**
- Add: `src-tauri/src/shared/migration_core_tests.rs`
- Modify: `src-tauri/src/shared/mod.rs`
- Add: `docs/fixtures/phase-4-1c-whitelist-migration/contract.json`

Write the approved preview, allowlist, secret exclusion, workspace identity,
target conflict, staging, validation, interruption, rollback, explicit-root,
and inactive-runtime tests against the proposed shared API. Include fake secret
sentinels and assert zero occurrences in every generated artifact. Run the
focused Rust tests and retain their compile/test failure as RED evidence.

### Task 2: Implement pure discovery and sanitized planning

**Files:**
- Add: `src-tauri/src/shared/migration_core.rs`
- Modify: `src-tauri/src/shared/mod.rs`

Implement explicit-root discovery without recursive credential reading. Parse
settings/workspaces through read-only `std::fs::read_to_string`, enumerate
unknown field paths without values, classify root artifacts, detect target and
staging conflicts, and produce the sanitized preview. Parse
`release-identity.json.configSchemaVersion` as the separate schema authority.
Run the focused preview and source-integrity tests.

### Task 3: Implement allowlisted staging and validation

**Files:**
- Modify: `src-tauri/src/shared/migration_core.rs`
- Modify: `src-tauri/src/shared/migration_core_tests.rs`

Rebuild typed settings and workspaces, clear credentials and deferred identity
before serialization, write prepared and sanitized backup files, then validate
schema, paths, identity metadata, and forbidden keys/content before writing
`READY_FOR_ACTIVATION`. Preserve active target/source bytes. Run focused
allowlist, security, workspace, and validation tests.

### Task 4: Implement interruption and rollback safety

**Files:**
- Modify: `src-tauri/src/shared/migration_core.rs`
- Modify: `src-tauri/src/shared/migration_core_tests.rs`

Detect owned incomplete staging, rebuild deterministically, reject unowned
staging, and remove only a marker-bound staging directory during rollback. Add
failpoint-driven tests for every approved interruption boundary and prove no
activation occurs. Run focused lifecycle tests.

### Task 5: Close documentation, version, and verification

**Files:**
- Add: `docs/phase-4-1c-whitelist-migration.md`
- Add: `docs/evidence/phase-4-1c/README.md`
- Modify: `docs/evidence/README.md`
- Modify: `docs/phase-4-0-truth-release-boundary.md`
- Modify: `docs/phase-4-1b-release-version-authority.md`
- Modify: `README.md`
- Modify through tooling: `VERSION.json`, version projections and locks

Run focused migration/security/lifecycle tests and P4.1a/P4.1b regressions.
Then invoke canonical `version:bump -- build`, `version:sync`, and
`version:check` to reach `0.7.68 · Build 2 · development`. Run Rust all-targets,
check, fmt, typecheck, production build, frontend regression with exactly the
existing six waivers, and `git diff --check`. Inspect the scoped diff, explicitly
stage only P4.1c files, commit, ff-only merge, push, and verify refs. Do not begin
P4.1d.

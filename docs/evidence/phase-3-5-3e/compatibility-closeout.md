# Phase 3.5.3e Compatibility Closeout Evidence

Status: **PASS / COMPLETE / FROZEN**.

## Scope

This closeout adds no approval or delete product behavior. It freezes the
existing Phase 3.5.3a-d shared authority through sanitized fixtures and focused
Rust compatibility tests. No real approval decision, Thread deletion, rollout
deletion, writer mutation, retry, replay, or force takeover was executed.

## Frozen fixture authority

- `docs/fixtures/app-server/phase-3-5-3-compatibility/authority-contract.json`
  records the stable approval/delete state spellings, serialized fields,
  generation hierarchy, zero retry/replay counts, and confirmed-only tombstone
  authority.
- `docs/fixtures/app-server/phase-3-5-3-compatibility/fixture-family-manifest.json`
  binds the closeout suite to the existing sanitized approval, delete,
  isolation, and transport-generation fixture families.
- `src-tauri/src/shared/codex_core/phase_3_5_3_compatibility_tests.rs`
  exercises the real shared App/daemon authority types using only synthetic
  identities and deterministic inputs.

## Frozen contract

- Approval request identity is exact and scoped to WorkspaceSession plus
  app-server connection generations.
- Approval decision provenance is transport-scoped audit evidence; one exact
  pending approval admits at most one decision attempt.
- Delete authority requires an exact canonical `CodexThreadKey` and full Thread
  ID. Same-key concurrent intents dispatch at most once; different exact keys
  remain independent.
- Pre-dispatch local rejection and upstream rejection remain distinguishable.
  Direct current-generation upstream evidence outranks stale transport
  uncertainty.
- Only `delete_confirmed` authorizes a tombstone. Projection absence is not
  canonical deletion evidence.
- Approval and delete automatic retry/replay counts are zero.

## Remaining not proven

- Exact upstream behavior for duplicate or late approval responses.
- Exact upstream ordering or winner semantics for concurrent duplicate delete
  requests.

These unknowns are not represented as CodexMonitor ownership, lease,
availability, release, or takeover facts.

## RED / GREEN evidence

- Initial RED: the focused compatibility contract test failed because
  `authority-contract.json` did not yet exist.
- Coverage RED: the fixture-family test failed while the closeout manifest did
  not enumerate every required approval/delete family.
- GREEN: the focused compatibility suite passes all 19 tests against the real
  shared authority types and the complete sanitized fixture-family manifest.

## Fresh verification

- Rust all-targets: library 966 passed / 4 ignored; daemon 875 passed / 3
  ignored; daemonctl 29 passed; Tauri config 1 passed; zero failures.
- `cargo check --all-targets`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `npm run typecheck`: PASS.
- `git diff --check`: PASS (line-ending warnings only).

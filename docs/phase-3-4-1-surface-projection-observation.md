# Phase 3.4.1 — Surface Projection Observation Model

Status: **PASS / FROZEN**. Phase 3.4.0 is **FORENSICS COMPLETE**. Phase 3.4.2 Projection Observation Engine is **GO / NOT STARTED**.

## Contract

`SurfaceProjectionKey` combines an exact `CodexThreadKey`, a Surface, and a projection kind. It is projection correlation state only; it does not alter Thread, Workspace, or Desktop Project identity.

`SurfaceProjectionObservation` records requested membership, source coverage, observation time, provenance, diagnostics, reconciliation state, and action capability. The store keeps append-only raw history, deduplicates identical observations, and derives effective state with a deterministic selector independent of insertion or map iteration order.

## Projection states

- `PRESENT`: the projection directly observed the exact fullThreadId.
- `ABSENT`: a successful, complete source read confirmed that the exact fullThreadId is absent.
- `STALE`: the Surface remains `PRESENT` while higher-authority canonical evidence confirms deletion or absence.
- `UNKNOWN`: the source was bounded, partial, failed, not observed, or otherwise insufficient to establish membership.
- `NOT_APPLICABLE`: the projection kind is not defined for the Surface or scenario.

Bounded lists, incomplete pagination, filtered reads, failed reads, and unobserved sources cannot produce `ABSENT`. Coverage is evaluated before presence absence is asserted.

## Authority and reconciliation

Deletion tombstones and confirmed canonical absence outrank Surface projections. Tombstone plus `PRESENT` resolves to `STALE` with reconciliation `PENDING`; it cannot revive the canonical Thread. A later complete observation of `ABSENT` advances that stale projection to `RECONCILED`.

Action capability is independent of both presence and reconciliation. `REFRESHABLE`, `INVALIDATABLE`, `OBSERVE_ONLY`, and `UNSUPPORTED` state what an adapter may do; none can manufacture membership evidence or reconciliation success.

An absent required projection may carry `MISSING_PROJECTION`. Optional absence does not. Phase 2.5 Desktop orphan semantics remain compatible as generic `STALE` plus the `DESKTOP_STALE_ORPHAN` diagnostic, without changing its frozen criteria or admitting it to canonical Registry or runtime state.

## Scope boundary

This Slice is pure shared-core state and selection logic. It does not ingest Desktop metadata, probe CLI state, change Monitor UI, write Desktop private state, or implement the Phase 3.4.2 observation engine.

## Verification

- Phase 3.4.1 focused tests: 17 passed.
- Shared-core regression: 351 passed / 3 ignored.
- `cargo test --lib`: 487 passed / 4 ignored.
- `cargo check --all-targets`: PASS.
- `cargo fmt --check`: PASS.
- `npm run typecheck`: PASS.
- `git diff --check`: PASS.

Phase 3.4.2 Projection Observation Engine is the next and only development start point.

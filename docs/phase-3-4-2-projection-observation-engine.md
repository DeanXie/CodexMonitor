# Phase 3.4.2 — Projection Observation Engine

Status: **PASS / FROZEN**. Phase 3.4.0 is **FORENSICS COMPLETE**, and Phase 3.4.1 through Phase 3.4.4 are **PASS / FROZEN**. Phase 3.4.5 Focused Cross-Surface E2E is **GO / NOT STARTED**.

## Engine boundary

`surface_projection_engine` converts already-observed source results into the frozen Phase 3.4.1 `SurfaceProjectionObservation` contract. It owns append-only observation history and delegates effective-state selection to `SurfaceProjectionStore`. Canonical authority is always supplied by the caller as `PRESENT`, `ABSENT`, `TOMBSTONED`, or `UNKNOWN`; the engine does not rebuild the Global Source Registry, read tombstone truth, or create canonical Threads.

Monitor app-server responses feed the engine at the existing correlated response boundary. Exact `thread/read` success records `PRESENT`; authoritative exact-ID not-found records `ABSENT`; transport, schema, or mismatched-ID failures record `UNKNOWN`. `thread/list` remains a bounded projection inventory because it is source-kind filtered and may be paginated or limited, so an exact hit can prove `PRESENT` but a miss cannot prove `ABSENT`.

Desktop adapters consume the existing read-only `DesktopMetadataSnapshot`, `DesktopProjectProjection`, and Phase 2.5 stale-orphan assessment. They perform no Desktop I/O and do not modify SQLite, global state, catalog, sidebar, or Project assignment. Catalog, Project, sidebar, and other inventories are separate `ProjectionKind` values: a complete Desktop Catalog miss establishes only `DesktopCatalog = ABSENT`; it does not establish sidebar absence or Project unassignment.

CLI exact-ID discoverability uses `Discoverability`, while interactive picker/history membership uses `HistoryList`. No CLI probe process is started in this Slice. Picker/history miss without complete coverage remains `UNKNOWN`.

The Global Source canonical snapshot is accepted only as a projection-observation input for the exact `CodexThreadKey`; it does not become another identity authority.

## Coverage and reconciliation

- Exact hit: `PRESENT`.
- Complete inventory miss or authoritative exact-ID not-found: `ABSENT`.
- Bounded, partial, filtered, failed, or not-observed miss: `UNKNOWN`.
- `TOMBSTONED + Surface PRESENT`: effective `STALE + PENDING`.
- A later complete `ABSENT` for the same Surface and `ProjectionKind`: `RECONCILED`.

Surface observations cannot revive tombstoned Threads. Phase 2.5 `DESKTOP_STALE_ORPHAN` remains generic `STALE` with the `DESKTOP_STALE_ORPHAN` diagnostic; its frozen admission criteria are unchanged.

`WorkspaceSession.projection_observations` is initialized with an empty engine. The daemon and workspace test fixture `Default` additions create no observation and encode neither `UNKNOWN` nor `ABSENT`; they do not change daemon runtime routing or canonical truth.

## Verification

- Phase 3.4.2 focused engine tests: 23 passed.
- Phase 3.4.1 regression: 17 passed.
- Desktop projection regression: 16 passed.
- Global Source regression: 84 passed / 2 ignored.
- Phase 3.1 exact-ID regression: 7 passed.
- Phase 3.2 Workspace/Project regression: 77 passed / 1 ignored.
- Phase 3.3 creation regression: 46 passed.
- Phase 3.3 execution-settings regression: 38 passed / 1 ignored.
- Phase 3.3 Final Standard Session evidence tests: 10 passed.
- `cargo test --lib`: 513 passed / 4 ignored.
- daemon tests: 445 passed / 3 ignored.
- `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`: PASS.

Phase 3.4.5 Focused Cross-Surface E2E is the next and only development start point.

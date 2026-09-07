# Phase 3.4.3 — Desktop Stale / Missing Projection Handling

Status: **PASS / FROZEN**. Phase 3.4.1, Phase 3.4.2, and Phase 3.4.4 remain **PASS / FROZEN**. Phase 3.4.5 Focused Cross-Surface E2E is **GO / NOT STARTED**.

## Authority boundary

Canonical Thread truth remains independent from Desktop Catalog, Sidebar, and Project projections. `DesktopProjectionHandling` accepts already-observed Desktop projection evidence and externally loaded deletion tombstones; it performs no Desktop I/O, owns no canonical Registry, and exposes no repair or write operation.

A restored exact `CodexThreadKey` tombstone outranks any Desktop projection and any lower caller-supplied canonical state. A Desktop `PRESENT` observation therefore resolves to `STALE / PENDING` and preserves the `DESKTOP_STALE_ORPHAN` diagnostic. It cannot recreate a canonical Thread, produce an Agent Monitor runtime node, or alter Thread, Workspace, or Project identity.

## Absence, missing, and unknown

- A successful complete read that misses the exact fullThreadId records `ABSENT` for only the observed `ProjectionKind`.
- Bounded, partial, filtered, failed, schema-drifted, and not-observed reads remain `UNKNOWN`; they never establish absence.
- Canonical `PRESENT` plus Desktop `ABSENT` does not mean the Thread is missing.
- `MISSING_PROJECTION` is added only when the specific projection carries an explicit `Required` membership expectation. Optional absence has no missing diagnostic.
- Catalog, Sidebar, and Project remain independent. Catalog absence does not imply Sidebar absence or Project unassignment, and direct Project assignment does not imply Sidebar membership.

## Reconciliation lifecycle

For a tombstoned Thread, Desktop `PRESENT` resolves to `STALE / PENDING`. Only a later `COMPLETE` observation of `ABSENT` for the same Desktop Surface and the same `ProjectionKind` advances the effective observation to `ABSENT / RECONCILED`. Monitor deletion completion, app-server list misses, bounded Desktop misses, and process restart do not claim Desktop reconciliation.

Deletion tombstones are the restart anchor. After a persisted tombstone document is loaded, a fresh exact Desktop `PRESENT` observation deterministically reconstructs `STALE / PENDING`; a later complete absence can then reconcile it. Observation history itself is not fabricated across restart.

## Capability and known cases

Desktop Catalog is `OBSERVE_ONLY`; Sidebar is `UNSUPPORTED`. These capabilities do not claim active repair or reconciliation success. There is no write to `codex-dev.db`, `local_thread_catalog`, `.codex-global-state.json`, sidebar state, or Project assignment.

The contract preserves these valid combinations:

- a Monitor-created Session may be present in Desktop Catalog while Desktop Project remains `UNKNOWN`;
- a CLI/exec canonical Thread with incomplete Desktop coverage remains `UNKNOWN` on that Desktop projection;
- a long-lived Thread uses its latest source activity timestamp rather than creation time for activity ordering.

Phase 3.4.4 displays these already-derived states and capabilities without inventing repair authority. Phase 3.4.5 Focused Cross-Surface E2E is the next and only development start point.

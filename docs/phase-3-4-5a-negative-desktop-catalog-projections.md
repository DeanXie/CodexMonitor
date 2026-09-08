# Phase 3.4.5a Negative Desktop Catalog Projections

Status: **PASS / FROZEN**. Phase 3.4.5 Focused Cross-Surface E2E and Phase 3.4 Cross-Surface Projection Reconciliation are **PASS / COMPLETE**. Phase 3.5.0 is **FORENSICS COMPLETE** and Phase 3.5.1 is **GO / NOT STARTED**.

## Contract

Desktop Catalog inventory reports are emitted independently for each actually observed `codexHomeIdentity`. A successful complete read remains reportable even when the inventory is empty.

Canonical `PRESENT` Thread keys are joined with Desktop Catalog inventory only by the exact pair `(codexHomeIdentity, fullThreadId)`:

- exact hit produces `PRESENT`;
- `COMPLETE` exact miss produces `ABSENT`;
- `FAILED`, `NOT_OBSERVED`, `BOUNDED`, or `PARTIAL` exact miss produces `UNKNOWN`;
- no matching-home report produces no observation.

The join is deterministic and consumes the canonical Registry snapshot as external input. It does not create canonical authority. Catalog misses create only Desktop Catalog observations and do not imply Desktop Sidebar absence, Desktop Project assignment state, or canonical Thread absence. The former production shortcut that paired one reported ID with hardcoded `COMPLETE` coverage is removed.

The deletion path remains unchanged: a tombstoned Thread with an exact Desktop Catalog hit is `STALE / PENDING` with `DESKTOP_STALE_ORPHAN`, cannot revive canonical identity, and cannot create an Agent or runtime node.

## Gate C evidence

- full Thread ID: `01a080bd-66b1-7e82-8c02-3759c32c4283`
- Codex home identity: `codex-home:76c92e1eaaa451ef02c92f75f12738ea3337e69b890333a72cd14ec778cac75f`
- canonical state: `PRESENT`; canonical Agent nodes: `1`; duplicates: `0`
- Desktop Catalog coverage: `COMPLETE`
- inventory comparison: matching-home exact miss
- projection: `ABSENT`
- UI: `Not present in this surface`
- reconciliation: `Not required`
- capability: `Observe only`
- Desktop Sidebar projection: not generated
- Desktop Project projection: not generated

The `threads.project_id is missing` observation remains a Desktop private-schema drift diagnostic. It does not change Catalog coverage, canonical identity, or Desktop Project assignment state.

Gate C is **PASS** through the production watcher, projection join, snapshot transport, exact-key frontend selector, and Agent Monitor UI.

## Phase 3.4.5 outcome

Final Acceptance consolidated Gates A–I and is **PASS / COMPLETE**. Natural Desktop cleanup in Gate F is `NOT OBSERVED IN THIS E2E WINDOW`; this is an allowed Desktop OBSERVE_ONLY / UNSUPPORTED boundary and was not manufactured by writing Desktop private state. Phase 3.5.0 forensics are complete; Phase 3.5.1 is the next and only development start point.

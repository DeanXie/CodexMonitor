# Phase 3.4.4 — Monitor Projection Status UI

Status: **PASS / FROZEN**. Phase 3.4.1 through Phase 3.4.3 and Phase 3.4.5a remain **PASS / FROZEN**. Phase 3.4.5 / Phase 3.4 are **PASS / COMPLETE**. Phase 3.5 is **GO / NOT STARTED**.

## Presentation boundary

Canonical Thread status, Surface projection status, reconciliation state, and action capability remain independent. The backend publishes frozen engine output through the optional top-level `GlobalSourceSnapshot.surfaceProjections` field. Older snapshots that omit the field produce an empty, not-observed frontend view; the bridge does not invent `ABSENT` or `UNKNOWN` observations.

The frontend joins observations by exact `(codexHomeIdentity, fullThreadId)` only. It does not recalculate projection truth or alter the canonical Thread selector, Workspace or Project identity, current Session, token totals, or runtime lifecycle.

## UI contract

- Selected canonical Sessions expose a compact, expandable Projection status panel.
- `PRESENT` renders as `Present in this surface`.
- `ABSENT` renders as `Not present in this surface` and never implies that the canonical Thread was deleted.
- `STALE` renders as `Surface still references a deleted Thread`.
- `UNKNOWN` renders as `Not enough evidence`.
- `NOT_APPLICABLE` renders as `Not applicable`.
- Reconciliation and capability are displayed separately from presence.
- Desktop `PENDING` with `OBSERVE_ONLY` or `UNSUPPORTED` renders as `Waiting for Desktop to refresh`; no Repair action is exposed.

Projection-only `DESKTOP_STALE_ORPHAN` observations appear in the lightweight Projection Issues surface. They never create a canonical Session or Agent node and do not appear as an active Session.

## Time semantics

Reliable source or observation activity evidence is presented as primary `Latest activity`, while `Created` remains visible as historical metadata. When reliable activity evidence is unavailable, the UI shows only `Created`; creation-time fallback is never labeled as latest activity.

## Verification

- Phase 3.4.4 focused and Agent Monitor regression: 85 passed.
- Rust library: 529 passed / 4 ignored.
- `cargo check --all-targets`, `cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check`: PASS.
- Full frontend: 1121 passed; the only six failures are the approved pre-existing zh-CN locale/date baseline. New Phase 3.4.4 regressions: 0.

Implementation commit: `155a277 feat: expose projection status in agent monitor`.

Phase 3.4.5 and Phase 3.4 are complete. Phase 3.5 is the next and only development start point.

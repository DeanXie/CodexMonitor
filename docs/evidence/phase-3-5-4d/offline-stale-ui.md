# Phase 3.5.4d — Offline / Stale UI & Multi-client Isolation

Status: **PASS / COMPLETE / FROZEN**.

The Remote shell projects the existing `ProjectionFreshness` authority into
five user-visible states: `Current`, `Hydrating`, `Stale`, `Unavailable`, and
`Unknown`. Coverage remains independent. Availability layers and
`Live`/`Polling`/`Disconnected` delivery mode are displayed only as secondary
diagnostics.

Daemon broadcast lag produces an ephemeral, transport-generation-bound gap
marker. It has no durable sequence or persistence. Only the matching frontend
marks affected coverage stale and invokes the existing single-flight,
read-only recovery chain. Shared WorkspaceSession and canonical Thread,
writer, subscription, runtime, approval, and delete authority are unchanged.

The App disables stale/unavailable/unknown approval projections unless the
exact current-generation pending identity is present. Unknown delete outcome
does not render deleted; confirmed deletion retains precedence over stale
catalog content. All recovery mutation, retry, and replay counts remain zero.

Sanitized compatibility fixtures are in
`src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/`. Focused frontend
tests freeze the view-model, UI, gap subscription, multi-client isolation,
approval/delete safety, and fixture schema. Rust tests freeze daemon lag-marker
serialization and current-transport delivery isolation.

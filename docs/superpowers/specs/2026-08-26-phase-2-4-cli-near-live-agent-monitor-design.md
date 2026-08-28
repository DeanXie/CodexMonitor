# Phase 2.4 CLI Near-Live Agent Monitor Design

## Goal

Expose the backend Source Authority Registry as a stable read-only canonical snapshot and project external Codex CLI rollout sessions into Agent Monitor without changing or feeding the Phase 1 Runtime state machine.

## Architecture

The shared Global Source Core owns canonical identity and per-field authority. The Tauri app caches immutable, revisioned snapshots and emits full-snapshot updates. A single MainApp-scoped GlobalSourceViewStore subscribes once and ignores stale revisions. Agent Monitor merges its existing Phase 1 Runtime projection with the canonical source projection at the selector boundary.

The canonical key is `(codexHomeIdentity, fullThreadId)`. Phase 1 Runtime entries acquire their home identity through the backend-provided workspace-to-CODEX_HOME identity map; no Runtime state shape changes are required. A matching Phase 1 LIVE node wins. An external rollout-only node is projected as `NEAR_LIVE`. Historical-only canonical records never enter the live tree.

## Backend Contract

`global_source_snapshot` returns a revisioned snapshot containing workspace home identities and canonical thread records. Each thread contains its key, current turn, confirmed parent/agent path, resolved lifecycle/model/token snapshot, lane counts, and field-level provenance with source and observed timestamps plus freshness.

The backend emits `global-source-snapshot-updated` only when the canonical payload changes. The UI never holds or reads the mutable registry.

## Frontend Contract

`GlobalSourceViewStore` consumes only the snapshot command/event. It does not normalize rollout into app-server records and does not write Phase 1 Runtime.

The unified selector produces one Agent Monitor node per canonical key. Source filters are `All Sources`, `Monitor LIVE`, and `CLI NEAR LIVE`. External CLI roots are labeled `CLI — Main Agent — <shortId>`; sub-agents use confirmed `agentPath` where available. Current Chat applies only to the Phase 1 current thread.

## Authority and Display

The selector uses the backend-resolved cumulative token snapshot directly. It never sums LIVE and rollout tokens. LIVE-to-rollout fallback therefore follows Source Authority Registry monotonic snapshot rules.

Nodes display `LIVE`, `NEAR LIVE`, or `HISTORICAL` and a freshness age/state. Unknown fields remain `unavailable`. Running runtime uses confirmed turn/lifecycle start evidence; completed duration is shown only when both start and completion evidence exist.

## Scope Boundaries

No Desktop adapter, Historical Scanner refactor, Phase 1 Runtime semantic change, multi-window runtime, Model Router, installer, or large Agent Monitor redesign is included.


# App-server realtime protocol evidence

## Capture boundary

Evidence was captured through the existing CodexMonitor Debug Panel with Codex CLI `0.147.0` and CodexMonitor `0.7.68` on 2026-08-23. No business code was changed.

The Debug Panel records only while open and retains at most 200 records. The first multi-agent completion capture reached that limit and lost its early events. A second, cleared capture retained all 142 records and was used for child-start evidence. Array-valued payload fields are subject to the Debug Panel's existing summarization; no conclusion relies on a summarized array.

Canonical sanitized evidence is under `docs/fixtures/app-server/`.

## Observed realtime chain

```text
Codex app-server message
  -> Tauri wrapper { workspace_id, message }
  -> Debug Panel record { source, capturedAt, label, payload }
  -> message.method + message.params
```

`workspace_id` is the Monitor wrapper's workspace identity. Protocol event time is `message.emittedAtMs`; the Debug Panel display time is not used as the canonical timestamp.

## Event shapes observed

| Notification | Identity fields observed | State/data fields observed | Time fields observed | Model fields observed |
| --- | --- | --- | --- | --- |
| `thread/started` | `params.thread.id`, `sessionId`, `parentThreadId` | `thread.status.type`, `agentNickname`, `agentRole`, `source`, `modelProvider` | `thread.createdAt`, `updatedAt`, `recencyAt`, envelope `emittedAtMs` | No `model`, `modelId`, or `modelName` |
| `turn/started` | `params.threadId`, `turn.id` | `turn.status="inProgress"`, `error`, `itemsView` | `turn.startedAt`, envelope `emittedAtMs` | None |
| `item/started` | `params.threadId`, `turnId`, `item.id` | Shape depends on `item.type`; child creation uses `subAgentActivity` | `params.startedAtMs`, envelope `emittedAtMs` | None in observed items |
| `item/completed` | `params.threadId`, `turnId`, `item.id` | Completed item payload; child-start activity retains `kind="started"` | `params.completedAtMs`, envelope `emittedAtMs` | Observed `collabAgentToolCall.model`, but its value was `null` on `wait`; no usable child model |
| `thread/tokenUsage/updated` | `params.threadId`, `turnId` | `tokenUsage.last`, `tokenUsage.total`, `modelContextWindow` | Envelope `emittedAtMs` only | None |
| `thread/status/changed` | `params.threadId` | Observed `active`, `idle`, and `notLoaded` | Envelope `emittedAtMs` only | None |
| `turn/completed` | `params.threadId`, `turn.id` | `status="completed"`, `error`, item summary | `turn.startedAt`, `completedAt`, `durationMs`, envelope `emittedAtMs` | None |
| `model/rerouted` | Not observed in any capture | Schema unverified | Schema unverified | Schema unverified |

Additional server-sourced model evidence was observed outside the requested six notifications:

- `thread/start` response: `result.model`, `result.modelProvider`, `result.reasoningEffort`.
- `thread/settings/updated`: `params.threadId`, `params.threadSettings.model`, `modelProvider`, `effort`, and collaboration-mode model settings.

The client `turn/start` request also contains `model`, but it is request intent and must not override later server evidence.

## Reliable identity and hierarchy mapping

| Meaning | Reliable source | Confidence boundary |
| --- | --- | --- |
| Top-level thread ID | `thread/started.params.thread.id` | Directly observed |
| Event thread ID | `params.threadId` | Directly observed on turn, item, status, and token events |
| Turn ID | `params.turn.id` or `params.turnId` | Directly observed |
| Top-level parent | `thread/started.params.thread.parentThreadId` | Observed as `null` in the single-agent capture |
| Child thread ID | `item/*.params.item.agentThreadId` where `item.type="subAgentActivity"` and `kind="started"` | Directly observed for all three children |
| Child-to-parent edge | Parent is outer `item/*.params.threadId`; child is `item.agentThreadId` | Directly observed three times; no child `parentThreadId` field was delivered to the Debug Panel |
| Agent path/name | `item.agentPath` | Directly observed as `/root/early_a`, `/root/early_b`, and `/root/early_reviewer_c` before sanitization of IDs |

The fully retained multi-agent start capture delivered no child `thread/started` notification. The Monitor therefore cannot depend on `thread/started.params.thread.parentThreadId` for live child discovery with this app-server version. The `subAgentActivity` edge is the only captured live parent-child evidence.

## Model evidence and the `unavailable` cause

The top-level single-agent sequence showed:

1. `thread/started` with `modelProvider="openai"` but no model identifier.
2. `thread/start` response with `result.model="gpt-5.6-sol"`.
3. Client `turn/start.model="gpt-5.6-sol"`.
4. Server `thread/settings/updated.params.threadSettings.model="gpt-5.6-sol"`.
5. `turn/started`, item, status, and token notifications without a model field.

This explains the realtime `unavailable` gap: the high-frequency runtime notifications do not carry a model, while the server response/settings event that does carry it must be correlated and retained by thread ID.

For the three children, the prompt requested three different models, but the captured child-start, turn, status, token, and completion messages contained no non-null `model`, `modelId`, or `modelName`. Requested prompt text is not observed execution evidence. Their observed model must therefore remain `unavailable` from this evidence set.

No `model/rerouted` notification occurred across the single-agent run or either multi-agent window. Its params structure and model-switch semantics remain unverified and must not be implemented from assumption.

## Multi-agent runtime evidence

The retained start window established this live tree:

```text
thread-main
├── thread-early-child-a       via /root/early_a
├── thread-early-child-b       via /root/early_b
└── thread-early-reviewer-c    via /root/early_reviewer_c
```

Each child appeared first through a separate `subAgentActivity` start item on the main thread. Each child then emitted its own `thread/status/changed`, `turn/started`, `thread/tokenUsage/updated`, `turn/completed`, and return-to-`idle` sequence. Child count can therefore change live by counting unique `agentThreadId` values from these start activities.

The main thread also emitted `collabAgentToolCall` items with `tool="wait"` and `status` transitions from `inProgress` to `completed`. This is direct evidence that the main agent was waiting on collaboration work, but the captured wait items had empty `receiverThreadIds` and `agentsStates`; they cannot identify which child was awaited.

When the second group of children was spawned, the earlier child threads emitted `thread/status/changed` with `status.type="notLoaded"`. This is an observed runtime state outside the proposed five-state product vocabulary and must be handled explicitly rather than silently treated as failure or completion.

## Token attribution

Observed `tokenUsage` contains:

```text
last/total.cacheWriteInputTokens
last/total.cachedInputTokens
last/total.inputTokens
last/total.outputTokens
last/total.reasoningOutputTokens
last/total.totalTokens
modelContextWindow
```

The main follow-up turn received six notifications before completion. Its cumulative `total.totalTokens` increased monotonically:

```text
245797 -> 274667 -> 303636 -> 332691 -> 361832 -> 391060
```

Each increment equaled that notification's `last.totalTokens`. This is real incremental delivery at model-call boundaries, not a single history read after task completion. It is not per-token streaming.

Each child received a notification under its own `threadId` and `turnId`, followed immediately by its completed/idle events. In this short test, each child produced only one token update, so sub-turn incremental cadence for children was not demonstrated.

Safe attribution rules from the evidence are:

- Attribute a notification only to its exact `threadId`; use `turnId` for the active turn.
- Treat `tokenUsage.total` as cumulative thread usage, not the cost of the current turn.
- For a fully captured turn, aggregate `last` records for that `turnId`, or calculate the delta between cumulative totals with a known baseline.
- `cachedInputTokens` is a component of input usage and must not be added again to `totalTokens`.
- Do not merge child totals into the parent or assume the parent total includes/excludes child usage; that accounting relationship was not proven by these messages.
- Do not equate thread and agent usage unless the runtime mapping establishes that the agent owns that unique thread.

## Lifecycle evidence boundary

| Product concept | Direct evidence available | Limitation |
| --- | --- | --- |
| Created | Top-level `thread/started`; child `subAgentActivity(kind="started")` | Child activity time is a spawn observation, not a captured `thread.createdAt` |
| Running | `thread/status/changed(active)` plus `turn/started(status="inProgress")` | Both should be retained rather than inferred from elapsed time |
| Waiting | In-progress `collabAgentToolCall(tool="wait")` | Evidence applies to the main collaboration item; a final state rule is not yet selected |
| Completed | `turn/completed(status="completed")` with `completedAt` and `durationMs` | This completes a turn/task, not necessarily the reusable thread |
| Failed | No failed turn or thread was observed | No schema or mapping is established |

After a completed turn, observed thread status was `idle`, not `completed`. A single `AgentRuntimeState` must therefore decide whether it models a reusable thread, a turn execution, or a spawned-agent assignment before its final structure is fixed.

## Runtime implementation boundary

The Agent Monitor runtime model is implemented as three separate layers under
`src/features/agent-monitor/runtime/`:

- `ThreadRuntimeState` for reusable thread containers.
- `TurnRuntimeState` for one execution, including collaboration waiting.
- `AgentAssignment` for a captured `subAgentActivity` parent-child edge.

The normalizer accepts the sanitized fixture record shape and emits provenance-bearing,
idempotent normalized events. Thread cumulative Token snapshots and Turn `last`
increments remain separate, as do parent and child Thread totals. A completed Turn does
not complete its Thread.

The implementation does not guess child models, invent `model/rerouted` params, map
`notLoaded` to failure/completion, or synthesize a Failed transition without a captured
failure fixture.

The Runtime Store is now wired into the Agent Monitor Live view and is owned by
`MainApp`. The legacy Threads reducer remains separate. Runtime hydration may restore
only verifiable active Thread identity, hierarchy, status, and timing evidence; it does
not restore historical model or Token values into Live state.

## Runtime lifecycle acceptance

| Scenario | Status | Observed evidence |
| --- | --- | --- |
| A. Agent Monitor is already visible before a new Main + Sub-Agent task starts | PASS | In the same `MainApp` Split View, Main and Sub-Agent nodes appeared live, the Agent count grew, the parent-child tree was correct, status and Runtime changed, and Thread Token appeared and increased. |
| B. Task starts before Agent Monitor is opened | PASS | Opening Agent Monitor after the task started selected the Current Session and restored Main, existing Sub-Agents, hierarchy, status, Runtime, and continuing Thread Token updates. |
| C. Agent Monitor -> Chat/Home/Workspace -> Agent Monitor | PASS | Internal SPA navigation preserved the same Session and Runtime Store; Main and newly spawned Sub-Agents remained visible and Token/Runtime continued updating without a frontend reload. |
| D. A Sub-Agent starts while Agent Monitor is not mounted | PASS | A direct child named `late_child` created while the page was closed appeared after returning with the correct parent, Running state, Runtime, and independent Thread Token once its real event arrived. |

All four lifecycle scenarios were accepted using only internal SPA navigation. Their
acceptance is independent of model confirmation: a Thread can pass lifecycle monitoring
while its Live model remains `unavailable` when no confirmed observed-model event was
retained.

`Ctrl+R` is classified only as a Reload/Hydration test. It reloads the frontend,
recreates `MainApp` and the in-memory Runtime Store, and therefore cannot be used as
evidence for A, B, C, or D. Lifecycle acceptance must use only the application's
internal Chat, Home, and Agent Monitor navigation controls.

## Phase-one closure

Monitor-owned Codex realtime observation is the phase-one reference path and is
classified as `LIVE`. The Event Normalizer, three-layer Runtime State, MainApp-owned
Runtime Store, selectors, Full Page and Split View are accepted as the stable baseline
for future source adapters. External CLI and Desktop observation remains outside this
phase and must not be represented as equivalent to the app-server `LIVE` path.

## Current observed-model diagnostic boundary

The retained single-Agent fixture proves that a newly created Monitor-owned Thread can
receive both confirmed sources: `thread/start` response `result.model` and
`thread/settings/updated.params.threadSettings.model`. The retained multi-Agent start
window begins with `turn/start` on an existing Main Thread and contains neither source.
Hydration deliberately cannot fill that evidence gap.

Spawned child Threads remain a separate protocol limitation: their observed
`subAgentActivity`, turn, status, and Token events contain no confirmed model identifier.
They must remain `unavailable` until a real child-scoped confirmed-model event is
captured.

For an affected Main Thread, the decisive next capture is the exact Debug Panel sequence
from Thread creation through the first `turn/started`. If a `thread/start response` or
`thread/settings/updated` record contains the Main Thread ID and a non-empty model but
the Runtime still shows `unavailable`, the defect is in Runtime ingestion/correlation.
If neither record is present during the Store lifetime, `unavailable` is the correct
result under the confirmed-evidence rule.

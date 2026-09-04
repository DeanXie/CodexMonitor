# App-Server Events Reference (Codex `19702e190ebf16f789617ca5f16bfc373c238fe7`)

This document helps agents quickly answer:
- Which app-server events CodexMonitor supports right now.
- Which app-server requests CodexMonitor sends right now.
- Where to look in CodexMonitor to add support.
- Where to look in `../Codex` to compare event lists and find emitters.

When updating this document:
1. Fetch latest refs with `git -C ../Codex fetch --all --prune`.
2. Update the Codex hash in the title using `git -C ../Codex rev-parse origin/main`.
3. Compare Codex events vs CodexMonitor routing.
4. Compare Codex client request methods vs CodexMonitor outgoing request methods.
5. Compare Codex server request methods vs CodexMonitor inbound request handling.
6. Update supported and missing lists below.

Related project skill:
- `.codex/skills/app-server-events-sync/SKILL.md`

## Where To Look In CodexMonitor

Primary app-server event source of truth (methods + typed parsing helpers):
- `src/utils/appServerEvents.ts`

Primary event router:
- `src/features/app/hooks/useAppServerEvents.ts`

Agent Monitor runtime normalization and state (wired to the Live UI):
- `src/features/agent-monitor/runtime/eventNormalizer.ts`
- `src/features/agent-monitor/runtime/hydration.ts`
- `src/features/agent-monitor/runtime/runtimeState.ts`
- `src/features/agent-monitor/runtime/types.ts`
- `src/features/agent-monitor/hooks/useAgentRuntimeStore.ts`
- `src/features/agent-monitor/hooks/useAgentRuntimeHydration.ts`
- sanitized protocol inputs under `docs/fixtures/app-server/`

The runtime normalizer consumes raw Debug Panel records so it can distinguish
notifications, client request intent, and server responses. It currently confirms
an observed model only from `thread/start` responses and
`thread/settings/updated`; client `turn/start.model` remains requested-model
evidence. Child links come from `item.type = "subAgentActivity"` with
`kind = "started"`. `model/rerouted` remains deliberately unsupported until a
real payload is captured.

The Runtime Store is owned by `MainApp`, not by `AgentMonitorPage`, so it remains
mounted while the user moves between Chat, Home, Git, and Agent Monitor. Raw
app-server notifications, `thread/start` responses, and client `turn/start`
requests enter the same normalizer while any page is visible.

Runtime catch-up uses explicit `HYDRATION` provenance. It projects only currently
processing threads (plus their known ancestors and descendants) from the app's
current thread identity, parent, thread-status, and active-turn state. Those
inputs are maintained by the existing `thread/list`, `thread/read`/resume, and
runtime status paths. Hydration has no server timestamp and never imports a
historical model or token value. Confirmed observed models still require
`thread/start` response or `thread/settings/updated`; Thread Token still requires
`thread/tokenUsage/updated`.

The unified Agent Monitor selector may supplement Runtime rows with confirmed
Global Source model evidence from `LIVE` or `NEAR_LIVE` lanes. It must reject an
`observedModel` whose field provenance is `HISTORICAL`, even when the same Thread
has a separate Live or Near Live lifecycle lane; historical model values remain
available only in the separately labelled usage history view.

Event handler composition:
- `src/features/threads/hooks/useThreadEventHandlers.ts`

Thread/turn/item handlers:
- `src/features/threads/hooks/useThreadTurnEvents.ts`
- `src/features/threads/hooks/useThreadItemEvents.ts`
- `src/features/threads/hooks/useThreadApprovalEvents.ts`
- `src/features/threads/hooks/useThreadUserInputEvents.ts`
- `src/features/skills/hooks/useSkills.ts`

State updates:
- `src/features/threads/hooks/useThreadsReducer.ts`

Item normalization / display shaping:
- `src/utils/threadItems.ts`

UI rendering of items:
- `src/features/messages/components/Messages.tsx`

Primary outgoing request layer:
- `src/services/tauri.ts`
- `src-tauri/src/shared/codex_core.rs`
- `src-tauri/src/codex/mod.rs`
- `src-tauri/src/bin/codex_monitor_daemon.rs`

Phase 3.3.2 wraps outgoing `thread/start` and first `turn/start` requests in a
Monitor-owned, process-local coordination contract. It provides client-side
at-most-once transport dispatch per explicit intent without adding an upstream
idempotency field. `turn/completed` supplies exact correlated first-Turn outcome
evidence when Thread and Turn IDs are already known; protocol rejection remains
coordination state `FIRST_TURN_FAILED` with `failureReason = REJECTED`.

## Supported Notifications (Codex v2)

These are the current Codex v2 `ServerNotification` methods that CodexMonitor
supports in `src/utils/appServerEvents.ts` (`SUPPORTED_APP_SERVER_METHODS`) and
then either routes in `useAppServerEvents.ts` or handles in feature-specific
subscriptions.

- `account/login/completed`
- `account/rateLimits/updated`
- `account/updated`
- `app/list/updated`
- `error`
- `hook/completed`
- `hook/started`
- `item/agentMessage/delta`
- `item/commandExecution/outputDelta`
- `item/commandExecution/terminalInteraction`
- `item/completed`
- `item/fileChange/outputDelta`
- `item/plan/delta`
- `item/reasoning/summaryPartAdded`
- `item/reasoning/summaryTextDelta`
- `item/reasoning/textDelta`
- `item/started`
- `thread/archived`
- `thread/closed`
- `thread/deleted`
- `thread/name/updated`
- `thread/started`
- `thread/status/changed`
- `thread/tokenUsage/updated`
- `thread/unarchived`
- `turn/completed`
- `turn/diff/updated`
- `turn/plan/updated`
- `turn/started`

## Additional Stream Methods Handled In CodexMonitor

These arrive on the same frontend event stream but are not Codex v2
`ServerNotification` methods:

- approval requests ending in `requestApproval`, including
  `item/commandExecution/requestApproval`,
  `item/fileChange/requestApproval`, and
  `item/permissions/requestApproval`, via suffix match in
  `isApprovalRequestMethod(method)`
- `item/tool/requestUserInput` (a Codex v2 server request, not a notification)
- `codex/backgroundThread` (CodexMonitor synthetic bridge event)
- `codex/connected` (CodexMonitor synthetic bridge event)
- `codex/event/skills_update_available` (handled via
  `isSkillsUpdateAvailableEvent(...)` in `useSkills.ts`)

## Conversation Compaction Signals (Codex v2)

Codex currently exposes two compaction signals:

- Preferred: `item/started` + `item/completed` with `item.type = "contextCompaction"` (`ThreadItem::ContextCompaction`).
- Deprecated: `thread/compacted` (`ContextCompactedNotification`).

CodexMonitor status:

- It routes `item/started` and `item/completed`, so the preferred signal reaches the frontend event layer.
- It renders/stores `contextCompaction` items via the normal item lifecycle.
- It no longer routes deprecated `thread/compacted`.

## Missing Events (Codex v2 Notifications)

Compared against Codex app-server protocol v2 notifications, the following
events are currently not routed:

- `configWarning`
- `command/exec/outputDelta`
- `deprecationNotice`
- `fuzzyFileSearch/sessionCompleted`
- `fuzzyFileSearch/sessionUpdated`
- `item/mcpToolCall/progress`
- `item/autoApprovalReview/completed`
- `item/autoApprovalReview/started`
- `mcpServer/oauthLogin/completed`
- `mcpServer/startupStatus/updated`
- `model/rerouted`
- `rawResponseItem/completed`
- `serverRequest/resolved`
- `skills/changed`
- `thread/compacted` (deprecated; intentionally not routed)
- `thread/realtime/closed`
- `thread/realtime/error`
- `thread/realtime/itemAdded`
- `thread/realtime/outputAudio/delta`
- `thread/realtime/started`
- `thread/realtime/transcriptUpdated`
- `windows/worldWritableWarning`
- `windowsSandbox/setupCompleted`

## Supported Requests (CodexMonitor -> App-Server, v2)

These are v2 request methods CodexMonitor currently sends to Codex app-server:

- `thread/delete` permanently removes the selected thread. Codex app-server
  also deletes its spawned descendants. Current upstream integration coverage
  emits `thread/deleted` child-first for every deleted thread and then observes
  an empty `thread/loaded/list`. CodexMonitor does not depend on that per-child
  notification granularity: after a successful delete it persists a canonical
  fullThreadId tombstone for the root and confirmed descendants, retires their
  Registry/Watcher/checkpoint state, and then performs an authoritative
  `thread/list` reconciliation with anchor preservation disabled. Independent
  `thread/deleted` notifications are retained as confirmation evidence but do
  not create a local delete operation or infer a descendant closure.
  Sanitized protocol evidence lives in
  `docs/fixtures/app-server/thread-delete-cascade.protocol.json`.

- `thread/start`
- `thread/read`
- `thread/resume`
  Both exact-ID operations use the same request-construction boundary. A
  successful response is accepted only when `result.thread.id` exactly equals
  the requested full thread ID. Resume never falls back to `thread/start` and
  is not a `turn/start` operation.
- `thread/fork`
- `thread/list`
- `thread/archive`
- `thread/compact/start`
- `thread/name/set`
- `turn/start`
- `turn/steer` (used for explicit steer follow-ups while a turn is active)
- `turn/interrupt`
- `review/start`
- `model/list`
- `experimentalFeature/list`
- `collaborationMode/list`
- `mcpServerStatus/list`
- `account/login/start`
- `account/login/cancel`
- `account/rateLimits/read`
- `account/read`
- `skills/list`
- `app/list`

Notes:
- `turn/start` now forwards the optional `serviceTier` override (`"fast"` for `/fast`, `null` for default/off) alongside `model`, `effort`, and `collaborationMode`.

## Missing Client Requests (Codex v2 ClientRequest Methods)

Compared against Codex v2 request methods, CodexMonitor currently does not send:

- `account/logout`
- `command/exec`
- `command/exec/resize`
- `command/exec/terminate`
- `command/exec/write`
- `config/batchWrite`
- `config/mcpServer/reload`
- `config/read`
- `config/value/write`
- `configRequirements/read`
- `externalAgentConfig/detect`
- `externalAgentConfig/import`
- `feedback/upload`
- `fs/copy`
- `fs/createDirectory`
- `fs/getMetadata`
- `fs/readDirectory`
- `fs/readFile`
- `fs/remove`
- `fs/writeFile`
- `fuzzyFileSearch/sessionStart`
- `fuzzyFileSearch/sessionStop`
- `fuzzyFileSearch/sessionUpdate`
- `mcpServer/oauth/login`
- `mock/experimentalMethod`
- `plugin/install`
- `plugin/list`
- `plugin/read`
- `plugin/uninstall`
- `skills/config/write`
- `thread/backgroundTerminals/clean`
- `thread/decrement_elicitation`
- `thread/increment_elicitation`
- `thread/loaded/list`
- `thread/metadata/update`
- `thread/realtime/appendAudio`
- `thread/realtime/appendText`
- `thread/realtime/start`
- `thread/realtime/stop`
- `thread/rollback`
- `thread/shellCommand`
- `thread/unarchive`
- `thread/unsubscribe`
- `windowsSandbox/setupStart`

## Server Requests (App-Server -> CodexMonitor, v2)

Supported server requests:

- `item/commandExecution/requestApproval`
- `item/fileChange/requestApproval`
- `item/permissions/requestApproval`
- `item/tool/requestUserInput`

Missing server requests:

- `item/tool/call`
- `account/chatgptAuthTokens/refresh`
- `mcpServer/elicitation/request`

## Where To Look In ../Codex

Start here for the authoritative v2 notification list:
- `../Codex/codex-rs/app-server-protocol/src/protocol/common.rs`

Useful follow-ups:
- Notification payload types:
  - `../Codex/codex-rs/app-server-protocol/src/protocol/v2.rs`
- Emitters / wiring from core events to server notifications:
  - `../Codex/codex-rs/app-server/src/bespoke_event_handling.rs`
- Human-readable protocol notes:
  - `../Codex/codex-rs/app-server/README.md`

## Quick Comparison Workflow

Use this workflow to update the lists above:

1. Get the current Codex hash:
   - `git -C ../Codex fetch --all --prune && git -C ../Codex rev-parse origin/main`
2. List Codex v2 notification methods:
   - `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/common.rs | awk '/server_notification_definitions! \\{/,/client_notification_definitions! \\{/' | rg -N -o '=>\\s*\"[^\"]+\"|rename = \"[^\"]+\"' | sed -E 's/.*\"([^\"]+)\".*/\\1/' | sort -u`
3. List CodexMonitor routed methods:
   - `rg -n \"SUPPORTED_APP_SERVER_METHODS\" src/utils/appServerEvents.ts`
4. Update the Supported and Missing sections.

## Quick Request Comparison Workflow

Use this workflow to update request support lists:

1. Get the current Codex hash:
   - `git -C ../Codex fetch --all --prune && git -C ../Codex rev-parse origin/main`
2. List Codex client request methods:
   - `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/common.rs | awk '/client_request_definitions! \\{/,/\\/\\/\\/ DEPRECATED APIs below/' | rg -N -o '=>\\s*\"[^\"]+\"\\s*\\{' | sed -E 's/.*\"([^\"]+)\".*/\\1/' | sort -u`
3. List Codex server request methods:
   - `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/common.rs | awk '/server_request_definitions! \\{/,/\\/\\/\\/ DEPRECATED APIs below/' | rg -N -o '=>\\s*\"[^\"]+\"\\s*\\{' | sed -E 's/.*\"([^\"]+)\".*/\\1/' | sort -u`
4. List CodexMonitor outgoing requests:
   - `perl -0777 -ne 'while(/send_request_for_workspace\\(\\s*&[^,]+\\s*,\\s*\"([^\"]+)\"/g){print \"$1\\n\"}' src-tauri/src/shared/codex_core.rs | sort -u`
5. Update the Supported Requests, Missing Client Requests, and Server Requests sections.

## Schema Drift Workflow (Best)

Use this when the method list is unchanged but behavior looks off.

1. Confirm the current Codex hash:
   - `git -C ../Codex fetch --all --prune && git -C ../Codex rev-parse origin/main`
2. Inspect the authoritative notification structs:
   - `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/v2.rs | rg -n \"struct .*Notification\"`
3. For a specific method, jump to its struct definition:
   - Example: `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/v2.rs | rg -n \"struct TurnPlanUpdatedNotification|struct ThreadTokenUsageUpdatedNotification|struct AccountRateLimitsUpdatedNotification|struct ItemStartedNotification|struct ItemCompletedNotification\"`
4. Compare payload shapes to the router expectations:
   - Parser/source of truth: `src/utils/appServerEvents.ts`
   - Router: `src/features/app/hooks/useAppServerEvents.ts`
   - Turn/plan/token/rate-limit normalization: `src/features/threads/utils/threadNormalize.ts`
   - Item shaping for display: `src/utils/threadItems.ts`
5. Verify the ThreadItem schema (many UI issues start here):
   - `git -C ../Codex show origin/main:codex-rs/app-server-protocol/src/protocol/v2.rs | rg -n \"enum ThreadItem|CommandExecution|FileChange|McpToolCall|EnteredReviewMode|ExitedReviewMode|ContextCompaction\"`
6. Check for camelCase vs snake_case mismatches:
   - The protocol uses `#[serde(rename_all = \"camelCase\")]`, but fields are often declared in snake_case.
   - CodexMonitor generally defends against this by checking both forms (for example in `threadNormalize.ts` and `useAppServerEvents.ts`), while centralizing method/type parsing in `appServerEvents.ts`.
7. If a schema change is found, fix it at the edges first:
   - Prefer updating `src/utils/appServerEvents.ts`, `useAppServerEvents.ts`, and `threadNormalize.ts` rather than spreading conditionals into components.

## Notes

- Not all missing events must be surfaced in the conversation view; some may
  be better as toasts, settings warnings, or debug-only entries.
- For conversation view changes, prefer:
  - Add method/type support in `src/utils/appServerEvents.ts`
  - Route in `useAppServerEvents.ts`
  - Handle in `useThreadTurnEvents.ts` or `useThreadItemEvents.ts`
  - Update state in `useThreadsReducer.ts`
  - Render in `Messages.tsx`
- `turn/diff/updated` is now fully wired:
  - Routed in `useAppServerEvents.ts`
  - Handled in `useThreadTurnEvents.ts` / `useThreadEventHandlers.ts`
  - Stored in `useThreadsReducer.ts` (`turnDiffByThread`)
  - Exposed by `useThreads.ts` for UI consumers
- Steering behavior while a turn is processing:
  - CodexMonitor attempts `turn/steer` only when steer capability is enabled, the thread is processing, and an active turn id exists.
  - If `turn/steer` fails, CodexMonitor does not fall back to `turn/start`; it clears stale processing/turn state when applicable, surfaces an error, and returns `steer_failed`.
  - Local queue fallback on `steer_failed` is handled in the composer queued-send flow (`useQueuedSend`), not by all direct `sendUserMessageToThread` callers.
- Feature toggles in Settings:
  - `experimentalFeature/list` is an app-server request.
  - Toggle writes use local/daemon command surfaces (`set_codex_feature_flag` and app settings update),
    which write `config.toml`; they are not app-server `ClientRequest` methods.

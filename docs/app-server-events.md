# App-Server Events Reference

This document helps agents quickly answer:
- Which app-server events CodexMonitor supports right now.
- Which app-server requests CodexMonitor sends right now.
- Where to look in CodexMonitor to add support.
- Where to look in `../Codex` to compare event lists and find emitters.

The broad event/request inventory below was last compared at Codex commit
`19702e190ebf16f789617ca5f16bfc373c238fe7`. Phase 3.5.2b.5 separately
recalibrated the writer-admission and unsubscribe boundary against the bundled
Windows x64 `codex-cli 0.153.4` executable (SHA-256
`444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B`;
official source commit `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`) and upstream `main` commit
`e9633d7a0226eac91c7a791dc4f92cf8f25df2ae`, checked 2026-09-14. The frozen
machine-readable writer and subscription/runtime provenance is in
`docs/fixtures/app-server/writer-admission-observation/protocol-provenance.json`
and `docs/fixtures/app-server/thread-lifecycle-observation/protocol-provenance.json`.

When updating this document:
1. Fetch latest refs with `git -C ../Codex fetch --all --prune`.
2. Update the broad-inventory baseline hash above using
   `git -C ../Codex rev-parse origin/main`.
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

For a Remote App backend, notification delivery is additionally gated by the
authenticated `RemoteTransportGeneration`. A reader may publish
`app-server-event`, `terminal-output`, or `terminal-exit` only after its backend
has completed the full host-readiness handshake and that generation is the
current cached backend. Notifications from replaced, initializing, cleared, or
unauthenticated generations are dropped immediately before the Tauri event
hub. This transport gate changes no app-server method or payload schema and
does not infer WorkspaceSession, subscription, runtime, or writer state.

Phase 3.5.2d freezes five distinct continuity authorities around this delivery
path: `RemoteHostIdentity` identifies the host;
`DaemonProcessGeneration`, `RemoteTransportGeneration`,
`WorkspaceSessionGeneration`, and `AppServerConnectionGeneration` identify
their respective process, TCP transport, session, and protocol-connection
lifetimes. Reconnect creates a new transport generation. Daemon restart keeps
the pinned Host identity but creates a new daemon-process generation and an
empty sessions map; explicit re-establishment creates new WorkspaceSession and
app-server generations. Multiple transports may share one WorkspaceSession,
but request provenance remains transport-scoped, writer/runtime truth remains
session-scoped, and subscription truth remains app-server-connection-scoped.
No generation is a client identity or ownership record. Stale transport
response, notification, disconnect, EOF, and read-error evidence cannot enter
the current generation, and no mutation is retried or replayed across reconnect
or restart. The sanitized compatibility authority is
`docs/fixtures/remote-transport-coordination/`.

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

Phase 3.3.3b also sends execution-setting observations from the app-server
transport into the shared evidence store. The app and daemon keep one
process-level store across WorkspaceSession reconnects. A pending `thread/start`
or `turn/start` request is correlated by its JSON-RPC request ID, but requested
field evidence is not scoped until the response supplies the authoritative full
Thread or Turn ID. `thread/start` response settings are `THREAD_DEFAULT`
server-effective evidence. A `thread/settings/updated` notification without a
full Turn ID is also only a `THREAD_DEFAULT` snapshot; it is never assigned to a
nearby Turn. Rollout `turn_context` records provide `TURN_EXECUTION`
persisted-observed evidence using their own full Thread and Turn IDs. Omitted,
explicit-null, and concrete values remain distinct, and no source promotes one
evidence layer into another.

Phase 3.3.3c real acceptance confirmed that current app-server request enums use
camelCase sandbox values while rollout `turn_context` uses equivalent kebab-case
values. The evidence ingestion boundary canonicalizes only the confirmed
`workspaceWrite`/`workspace-write`, `readOnly`/`read-only`, and
`dangerFullAccess`/`danger-full-access` aliases before comparison. It does not
normalize unrelated or unknown values. The same acceptance observed
`writableRoots=[]` and a separate `runtimeWorkspaceRoots=[workspace]`; their
relationship remains unknown and is not inferred as a conflict.

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
- `serverRequest/resolved`
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

## Approval request observation

Phase 3.5.3a observes command-execution, file-change, and permissions approval
requests in the shared `WorkspaceSession` boundary before App/daemon event
projection. The exact identity contains the WorkspaceSession generation,
app-server connection generation, JSON-RPC request ID, Thread ID, Turn ID, and
item ID. An upstream `approvalId` is subordinate evidence only.

The states are `not_observed`, `pending`, `resolved_or_cleared`, and
`session_ended_unresolved`. `serverRequest/resolved` clears only an exact
current-generation request ID and Thread ID match. `item/completed` clears only
when request ID, Thread ID, Turn ID, and item ID all match. Auto-review
started/completed messages are lifecycle annotations, not human decisions.

Only confirmed app-server process/WorkspaceSession generation end changes a
pending request to `session_ended_unresolved`. Remote TCP disconnect, page
close, and transport reconnect are non-transitions. Multiple Remote transports
share this session registry without acquiring client ownership.

The request-observation authority itself sends no decision and exposes no
approver identity, owner, or lease. Sanitized fixtures live in
`docs/fixtures/app-server/approval-request-observation/`.

Phase 3.5.3b instruments only explicit Remote approval decisions sent through
`respond_to_server_request`. The shared decision core requires an exact current
approval identity, validates the bundled command/file/permissions response
schema, admits at most one attempt, and binds the attempt to both session
generations and Remote transport/request provenance.

An app-server stdin write records `decision_dispatched`, not decision
application or approval ownership. Pre-write loss with zero writes records
`decision_not_dispatched`; an unobserved outcome after the write boundary
records `decision_outcome_unknown`. There is no automatic retry or replay.
Resolved/completed events may annotate the exact related attempt but do not
prove which attempt won. App-local responses retain their existing path and do
not acquire Remote transport provenance. Deterministic fixtures live in
`docs/fixtures/app-server/approval-decision-provenance/`.

Phase 3.5.3e freezes this approval contract together with exact-ID delete
authority in `docs/fixtures/app-server/phase-3-5-3-compatibility/`. The
compatibility tests serialize the real shared App/daemon authority types and
verify generation isolation, single-dispatch gates, direct-evidence precedence,
confirmed-only tombstones, and zero retry/replay. They add no request method or
payload field. Exact upstream duplicate/late approval-response behavior remains
not proven.

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

- `thread/delete` permanently removes the exact requested full Thread ID.
  Codex app-server also deletes its spawned descendants. CodexMonitor admits no
  title/cwd/fuzzy delete target and binds each explicit request to one
  `DeleteAttemptId`, the exact `CodexThreadKey`, Host identity, and current
  WorkspaceSession/app-server generations. The bundled empty success response
  and an exact current-generation `thread/deleted` notification are direct
  `delete_confirmed` evidence; active-writer `-32600` is `delete_rejected`.
  Post-dispatch response loss, disconnect, cancellation, or malformed response
  is `delete_outcome_unknown`, never a tombstone. Only confirmed deletion
  persists the canonical fullThreadId tombstone, retires Registry/Watcher/
  checkpoint state, and permits authoritative reconciliation. Missing rollout,
  `thread/closed`, runtime `notLoaded`, UI removal, and stale notification
  evidence do not confirm deletion. Retry and replay are zero. Sanitized fake
  app-server evidence lives in
  `docs/fixtures/app-server/delete-mutation-observation/`; concurrent/stale
  isolation evidence lives in
  `docs/fixtures/app-server/delete-mutation-isolation/`; cascade projection
  evidence remains in
  `docs/fixtures/app-server/thread-delete-cascade.protocol.json`.

  The active delete gate is shared by the WorkspaceSession and keyed by current
  WorkspaceSession generation, current app-server connection generation, and
  exact `CodexThreadKey`. Same-key simultaneous intents have distinct attempt
  IDs but at most one local dispatch; different keys remain concurrent. A
  pre-write transport loss cancels the write and records local rejection with
  zero dispatch. A post-write transport loss is outcome-unknown until direct
  response/event evidence arrives. Direct current-generation success or
  rejection supersedes transport uncertainty; stale transport, session,
  app-server, and projection evidence cannot change current confirmed truth.
  Reconnect and daemon/session replacement never replay `thread/delete`.
  Phase 3.5.3e compatibility fixtures freeze these rules without claiming an
  upstream winner or ordering for concurrent duplicate deletes; that upstream
  behavior remains not proven.

- `thread/start`
- `thread/read`
- `thread/resume`
  The exact-ID read and resume operations use the same request-construction
  boundary but have different authority. `thread/read` is observation-only and
  does not attach the creation coordinator or mark the client as writer-admitted;
  selection, focus, polling, and ordinary refresh use this path. A successful
  stored-thread response may report runtime status `notLoaded`; that remains
  direct existence evidence. A textual `thread not loaded` failure is unavailable
  evidence, never authoritative absence. Only `thread not found` or
  `no rollout found` is accepted as authoritative exact-ID absence.

  `thread/resume` is explicit writer admission used by continue/send/resume
  interactions. A
  successful response is accepted only when `result.thread.id` exactly equals
  the requested full thread ID. Resume never falls back to `thread/start` and
  is not a `turn/start` operation. Active-writer error `-32600` is classified as
  `BLOCKED_BY_ACTIVE_WRITER`. Because dispatch outcome after a disconnect is not
  proven idempotent, `resume_thread` is excluded from automatic safe retry.
  Phase 3.5.2a's frozen implementation and acceptance evidence are recorded in
  `docs/phase-3-5-2a-exact-id-remote-admission.md`.

  Phase 3.5.2b.1 defines the shared-core observation contract, Phase 3.5.2b.2
  wires it to the existing exact `thread/resume` dispatch boundary, Phase
  3.5.2b.3 invalidates observations when the corresponding app-server process
  generation is directly observed to end, and Phase 3.5.2b.4 exposes that same
  authority through the App command and daemon RPC
  `get_writer_admission_observation`. This is a CodexMonitor read-only surface,
  not an upstream app-server request.
  Each WorkspaceSession creates a distinct generation; every explicit resume
  gets a unique attempt ID and records pending before dispatch. Exact-ID success,
  typed active-writer rejection, and ambiguous post-dispatch outcomes are
  recorded for that session generation and `CodexThreadKey`. Read, refresh,
  polling, unsubscribe, and Remote-client disconnect remain non-transitioning.
  Removing one route to a shared WorkspaceSession does not end its generation;
  last-route teardown and runtime-argument respawn record the end only after
  process termination is observed. A replacement session begins with a new
  generation and `NOT_OBSERVED`. Daemon hard exit cannot persist a reliable
  per-session acknowledgement and therefore writes no synthetic session-end
  observation.
  The read query returns `workspace session unavailable` when no current
  WorkspaceSession exists, while a connected current generation with no
  evidence returns `not_observed`. It neither connects the Workspace nor
  dispatches an app-server request. The model is not wired to UI and never reports global writer freedom,
  writer/lease identity, Remote-client ownership, or release. It does not use
  legacy `WriterOccupancy` as authority. See
  `docs/phase-3-5-2b-host-session-writer-admission-observation.md`.

  Phase 3.5.2b.5 freezes this request/response normalization in sanitized
  fixtures shared by the App serialization tests and the real daemon query RPC
  tests. The frozen state strings and snapshot fields are documented in the
  Phase 3.5.2b authority document; the fixtures contain no token, writer/lease
  identity, Remote-client ownership, or global freedom/release field.
- `thread/unsubscribe`
  Phase 3.5.2c.3 exposes this real upstream request through the explicit App
  command and daemon RPC `thread_upstream_unsubscribe`. Both adapters call one
  shared core and send `{ "threadId": <full Thread ID> }` exactly once. The
  request is separate from the local-only `thread_live_unsubscribe` operation.

  Each attempt is scoped to the current WorkspaceSession generation,
  app-server connection generation, and `CodexThreadKey`. A response status of
  `unsubscribed` records `UNSUBSCRIBED_FOR_APP_SERVER_CONNECTION`;
  `notSubscribed` records `NOT_SUBSCRIBED_FOR_APP_SERVER_CONNECTION`; and
  `notLoaded` records the same subscription state plus independent runtime
  evidence `NOT_LOADED_OBSERVED`. Timeout, response loss, disconnect,
  cancellation after dispatch, and malformed response record
  `UNSUBSCRIBE_OUTCOME_UNKNOWN`. Ambiguous outcomes are never retried, stale
  generations cannot update current evidence, and concurrent attempts for one
  scoped Thread fail closed. Cancellation before the dispatch boundary restores
  the prior subscribed evidence instead of leaving a false pending attempt. A
  response containing both JSON-RPC `error` and a recognized result status is
  malformed and cannot produce success evidence.

  None of these outcomes transitions `WriterAdmissionObservation` or proves
  writer release, availability, ownership, or a lease. A later
  `thread/closed` or `thread/status/changed(notLoaded)` is reconciled as
  independent `NOT_LOADED_OBSERVED` runtime evidence and never rewrites the
  subscription response or unknown outcome.
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
- `windowsSandbox/setupStart`

CodexMonitor's similarly named `thread_live_unsubscribe` is not this upstream
request. It is a synthetic local live detach and currently sends no app-server
RPC. Bundled `codex-cli 0.153.4` and checked upstream commit
`e9633d7a0226eac91c7a791dc4f92cf8f25df2ae` define real
`thread/unsubscribe` as connection subscription lifecycle with delayed idle
unload and response statuses `notLoaded`, `notSubscribed`, or `unsubscribed`.
None is a writer-release acknowledgement, so the operation cannot transition
the writer observation to `FREE`, `AVAILABLE`, or `RELEASED`.

Phase 3.5.2c.1 defines crate-private, connection-generation-scoped subscription
and runtime-availability observation reducers for this boundary. The two models
are independent from each other and from `WriterAdmissionObservation`.

Phase 3.5.2c.2 freezes `thread_live_unsubscribe` as a local synthetic live
detach shared by the App and daemon adapters. It validates Workspace/session
availability, emits `thread/live_detached`, and sends no app-server request.
It does not transition subscription, runtime-availability, or writer-admission
evidence, and it is excluded from automatic disconnect retry. See
`docs/phase-3-5-2c-subscription-release-lifecycle.md`.

Phase 3.5.2c.3 wires the explicit upstream request to those reducers through a
single App/daemon shared core. Phase 3.5.2c.4 adds shared ingestion for
`thread/closed` and `thread/status/changed(status.type = notLoaded)`, preserving
runtime evidence independently of subscription response order. Transport end,
confirmed app-server process end, and last shared-route teardown are recorded
as connection-generation lifecycle evidence rather than unsubscribe success.
Replacement connection and WorkspaceSession generations start without inherited
current subscription/runtime observations or retried unsubscribe attempts.
Phase 3.5.2c.5 freezes those normalized state strings, fields, outcome mappings,
generation boundaries, ordering rules, and App/daemon parity in sanitized
fixtures under `docs/fixtures/app-server/thread-lifecycle-observation/`.

The three authorities remain distinct in every fixture: subscription evidence
does not rewrite runtime evidence, and neither subscription nor runtime evidence
transitions writer admission. Missing Workspace, unavailable WorkspaceSession,
subscription `not_observed`, and runtime `unknown` remain four different
results. The fixture provenance separately identifies bundled behavior, the
checked upstream `main` reference, and CodexMonitor's normalized contract.

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

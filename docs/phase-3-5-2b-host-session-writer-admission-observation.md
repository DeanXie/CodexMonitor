# Phase 3.5.2b — Host-session Writer Admission Observation

Status: Phase 3.5.2b.1 shared observation model, Phase 3.5.2b.2
resume-boundary instrumentation, Phase 3.5.2b.3 session-lifecycle
invalidation, Phase 3.5.2b.4 App/daemon read-only observation exposure,
and Phase 3.5.2b.5 protocol compatibility freeze are
**PASS / COMPLETE / FROZEN**. Phase 3.5.2b is
**PASS / COMPLETE / FROZEN**.

## Authority boundary

The upstream Codex app-server/core remains the only writer authority.
CodexMonitor may request writer admission and record direct response evidence;
it does not decide ownership, availability, lease state, takeover, or release.

`WriterAdmissionObservation` is the writer-admission evidence authority.
`ExternalThreadAdmissionState.writer_occupancy` and `WriterOccupancy` are legacy,
insufficient, and non-authoritative for writer admission. The new model does not
read or derive from their `Occupied` or `Unoccupied` values.

The observation authority remains crate-private shared core. Phase 3.5.2b.4
adds a stable read-only serde/IPC projection and matching TypeScript contract;
it does not add frontend state or UI. Phase 3.5.2b.2 connects the authority only
to the existing exact `thread/resume` transport boundary shared by the local
app and daemon. None of these slices adds a mutation, retry, takeover, or
client-owned writer concept.

## Observation scope and provenance

An observation belongs to exactly:

```text
(WorkspaceSessionGeneration, CodexThreadKey)
```

Every actual admission observation carries the canonical Thread key, the
WorkspaceSession generation, observation timestamp, explicit resume attempt ID,
requested full Thread ID, request method, and the minimal non-sensitive
response/error evidence available. The attempt ID correlates one explicit
resume intent; it is not a writer owner or lease identifier.

Every `WorkspaceSession` creates one UUID-backed
`WorkspaceSessionGeneration` with its process-local observation runtime. A
replacement session therefore begins with an empty observation map and cannot
inherit admission, blocking, or ambiguous-outcome evidence from the prior
generation. No rollout/source generation or execution-environment identity is
repurposed.

Observations are attributed to the WorkspaceSession/app-server generation, not
to a Remote TCP connection, Remote client, or Mobile page. CodexMonitor has no
Remote-client identity evidence and does not invent it.

## State model

| State | Direct meaning | Explicit non-meaning |
| --- | --- | --- |
| `NOT_OBSERVED` | This generation has no explicit resume-attempt evidence for the Thread. | Not globally free or absent. |
| `ADMISSION_PENDING` | One explicit `thread/resume` intent is awaiting a classified outcome. | Not admitted and not globally occupied. |
| `ADMITTED_FOR_SESSION` | The upstream response returned the exact requested full Thread ID for this WorkspaceSession generation. | No owner identity, lease identity, global exclusivity, or future release claim. |
| `BLOCKED_BY_ACTIVE_WRITER` | Upstream rejected the attempt with `-32600` active-writer evidence. | Not `ABSENT`, `NOT_FOUND`, `FREE`, or permission to create a replacement Thread. |
| `ADMISSION_OUTCOME_UNKNOWN` | Timeout, post-dispatch disconnect, or equivalent uncertainty prevents classifying the attempt. | Not retry permission and not proof of release. |
| `SESSION_ENDED_RELEASE_UNOBSERVED` | The corresponding WorkspaceSession generation ended after admission evidence existed. | Release was not observed; this is never `RELEASED`. |

No state represents global `FREE`, `AVAILABLE`, `RELEASED`, `UNOCCUPIED`, a
known owner, or an active lease.

## Allowed transitions

```text
NOT_OBSERVED -> ADMISSION_PENDING
ADMISSION_PENDING -> ADMITTED_FOR_SESSION
ADMISSION_PENDING -> BLOCKED_BY_ACTIVE_WRITER
ADMISSION_PENDING -> ADMISSION_OUTCOME_UNKNOWN
ADMITTED_FOR_SESSION -> SESSION_ENDED_RELEASE_UNOBSERVED
BLOCKED_BY_ACTIVE_WRITER -> SESSION_ENDED_RELEASE_UNOBSERVED
ADMISSION_OUTCOME_UNKNOWN -> SESSION_ENDED_RELEASE_UNOBSERVED
ADMISSION_PENDING -> SESSION_ENDED_RELEASE_UNOBSERVED
```

The exact success transition requires the returned full Thread ID to equal the
requested ID. Session-end evidence must match the observation's generation. A
new WorkspaceSession generation begins at `NOT_OBSERVED` and inherits no prior
admission evidence.

## Session-lifecycle invalidation

Phase 3.5.2b.3 connects lifecycle invalidation only to direct evidence that an
app-server generation ended:

- an app-server child exit status observed by the stdout/process-health path;
- explicit termination of the app-server process after the last shared
  Workspace route is removed; or
- explicit replacement of that process generation during runtime-argument
  respawn.

Each retained observation for the ended generation becomes
`SESSION_ENDED_RELEASE_UNOBSERVED`. Session-end evidence records the previous
state, the ended generation, observation time, evidence kind, and a
non-sensitive diagnostic. A pending attempt keeps its attempt provenance and
is marked unresolved; this records that the session ended without claiming an
admission outcome. Historical ended-generation evidence may remain in that
generation's runtime, but a replacement `WorkspaceSession` has a new generation
and starts with no current observation (`NOT_OBSERVED`).

Workspace IDs may share one `Arc<WorkspaceSession>`. Removing one route only
unregisters that route while another route still references the same session;
it neither ends the generation nor changes admission evidence. The generation
ends only after the last route is removed and process termination is observed.
Storage reconciliation follows this route-aware teardown path instead of
killing a session merely because one alias became stale.

Daemon hard exit has no reliable per-session acknowledgement path after the
process is gone. It therefore does not synthesize per-Thread session-end
observations. Such evidence remains unavailable/stale rather than being
reported as release, availability, or global freedom.

## Events that do not transition writer admission

`thread/read`, `turn/completed`, Turn idle, ordinary refresh, polling, Remote
TCP disconnect, Remote client close, Mobile page close, focus loss, and the
current synthetic `thread/unsubscribe` do not alter admission evidence or imply
release. A disconnect after the explicit resume request was dispatched is
different: it changes that pending attempt to `ADMISSION_OUTCOME_UNKNOWN`, not
to a released or free state.

## Resume-boundary instrumentation

Each explicit `resume_thread_core` invocation creates a UUID-backed attempt ID
and records `ADMISSION_PENDING` before writing `thread/resume`. The observation
key is the current session generation plus the canonical
`(codexHomeIdentity, fullThreadId)` key; no Remote-client identity participates.
Concurrent attempts remain correlated by attempt ID while the session exposes
the most recently observed outcome for the Thread.

The same dispatch returns the following direct observations:

- A successful response with an exact `result.thread.id` records
  `ADMITTED_FOR_SESSION` and `exactIdMatch = true`.
- Error `-32600` whose message reports an active writer records
  `BLOCKED_BY_ACTIVE_WRITER`; the existing typed response classification is
  preserved and no absence or creation fallback is inferred.
- A mismatched, missing, malformed, or otherwise unclassified response records
  `ADMISSION_OUTCOME_UNKNOWN`, because the frozen model has no direct evidence
  for admission or active-writer blocking.
- A timeout, response-channel disconnect, or cancellation after the dispatch
  boundary records `ADMISSION_OUTCOME_UNKNOWN` with its non-sensitive error
  kind. Cancellation is guarded at the in-flight future boundary so dropping a
  dispatched request cannot leave a false pending observation.

`resume_thread` remains excluded from automatic Remote retry. `thread/read`,
ordinary refresh, polling, unsubscribe, and Remote-client disconnect do not
enter this instrumentation path.

## Read-only App/daemon exposure

`WriterAdmissionObservationSnapshot` is the sole serialized read model. It
contains the canonical `CodexThreadKey`, current WorkspaceSession generation,
the frozen state string, observation timestamp, attempt/request identity,
minimal response or error evidence, and optional session-end evidence. It has
no field for global freedom or availability, release, writer/lease identity,
or Remote-client ownership.

The App command and daemon RPC are both named
`get_writer_admission_observation`. Both accept `workspaceId` and `threadId`,
resolve the current WorkspaceSession generation and canonical Thread key, and
delegate to the same `get_writer_admission_observation_core` query. The
frontend wrapper only exposes this read contract; no UI consumes it in this
slice.

The query has four distinct boundaries:

- an unknown Workspace returns `workspace not found`;
- a known Workspace without a live WorkspaceSession returns
  `workspace session unavailable`;
- a live current generation with no observation for the canonical Thread key
  returns `not_observed`; and
- a live current generation with evidence returns that generation's snapshot.

Historical `SESSION_ENDED_RELEASE_UNOBSERVED` evidence remains attached to its
ended generation and cannot be returned as the current state of a replacement
session. Multiple Remote clients querying one daemon see the same
WorkspaceSession-scoped snapshot; the read result does not attribute ownership
to either client.

The query only reads Workspace/session maps, the session's canonical Codex home
identity, and the process-local observation runtime. It does not connect a
Workspace, dispatch an app-server request, call `thread/read`, `thread/resume`,
`thread/start`, or `turn/start`, change the generation, or hydrate writer state.

## Release evidence boundary

Turn completion, idle state, client disconnect, unsubscribe, WorkspaceSession
teardown, app-server exit, and daemon exit do not directly prove writer release.
Observed app-server generation termination may only produce
`SESSION_ENDED_RELEASE_UNOBSERVED`. Daemon hard exit produces no new per-session
observation. Any future claim of release requires new direct upstream evidence;
inference from local lifecycle is prohibited.

## Protocol compatibility freeze

Phase 3.5.2b.5 freezes sanitized protocol fixtures under
`docs/fixtures/app-server/writer-admission-observation/`. The fixtures cover
exact resume acceptance, active-writer rejection, ambiguous dispatch outcome,
session-end release-unobserved evidence, current-generation `NOT_OBSERVED`,
unknown Workspace, and unavailable WorkspaceSession. They contain the request,
minimal response/error evidence, normalized snapshot or query error,
WorkspaceSession generation, canonical Thread key, and attempt provenance.
They contain no authentication secret, writer/lease identity, global freedom,
or Remote-client ownership field.

The frozen serialized state strings are:

```text
not_observed
admission_pending
admitted_for_session
blocked_by_active_writer
admission_outcome_unknown
session_ended_release_unobserved
```

The frozen snapshot fields are `threadKey`, `workspaceSessionGeneration`,
`state`, `observedAt`, `attemptId`, `requestedFullThreadId`, `evidence`, and
`sessionEndEvidence`. App and daemon deserialize and serialize this same shared
Rust snapshot; daemon fixture tests also exercise the real
`get_writer_admission_observation` RPC route. A new generation reads
`not_observed` instead of inheriting old-generation evidence. Workspace absence
and WorkspaceSession unavailability remain query errors and are never encoded
as `not_observed`.

CodexMonitor's current `thread_live_unsubscribe` operation is synthetic local
subscription bookkeeping. It does not dispatch upstream `thread/unsubscribe`
and does not acknowledge writer release. In both the bundled Codex app-server
and the checked upstream protocol, real `thread/unsubscribe` manages the
calling connection's subscription lifecycle and delayed idle unload. Its
`notLoaded`, `notSubscribed`, or `unsubscribed` response status is not writer
release evidence and cannot produce `FREE`, `AVAILABLE`, or `RELEASED`.

The protocol provenance is frozen in
`docs/fixtures/app-server/writer-admission-observation/protocol-provenance.json`:

- bundled Windows x64 `codex-cli 0.153.4`, executable SHA-256
  `444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B`,
  official tag `rust-v0.153.4`, commit
  `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`;
- upstream `openai/codex` `main` checked on 2026-09-14 at commit
  `e9633d7a0226eac91c7a791dc4f92cf8f25df2ae`.

This is a compatibility snapshot, not a claim that future upstream revisions
retain the same behavior. A later protocol upgrade must re-run the fixture and
provenance checks before changing the frozen observation contract.

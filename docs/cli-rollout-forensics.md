# External Codex CLI rollout forensics and Source Adapter contract

## Scope and evidence boundary

This document defines the Phase 2 source contract and records real external Codex
CLI rollout evidence. Phase 1 app-server Runtime semantics remain unchanged.

Evidence was captured on 2026-08-25 with Codex CLI `0.147.0`, Windows, timezone
Asia/Shanghai, and the default `C:\Users\DeanX\.codex` CODEX_HOME. Tests covered:

- one single-Agent CLI Session;
- an explicit `gpt-5.6-terra` Turn;
- a second Turn resumed in a new CLI process;
- one Main plus three direct Sub-Agents;
- one exact app-server LIVE plus rollout NEAR-LIVE paired Turn;
- file-length sampling with a target interval of 50 ms.

Canonical sanitized evidence is under `docs/fixtures/cli-rollout/`. OpenAI's
documented `codex exec --json` stream is a separate stdout JSONL stream. This
forensics work concerns the persisted `sessions/**/rollout-*.jsonl` files, whose
internal schema and persistence behavior are established from local captures.

## Temporal source classes

| Class | Definition | May drive active Runtime state |
| --- | --- | --- |
| `LIVE` | Direct events from the active runtime transport, before relying on persisted artifacts. Current implementation: Monitor-owned app-server connection. | Yes, with the existing Phase 1 protocol rules. |
| `NEAR_LIVE` | Complete records observed after an active runtime appends and flushes them to a local artifact. Delivery is subject to write batching, watcher latency, partial records, process crashes, and source staleness. | Only from explicit rollout lifecycle records and with NEAR-LIVE provenance. |
| `HISTORICAL` | A scan or snapshot that does not establish that its producer is currently active. | No. It may populate History only. |

Authority is fixed as:

```text
app-server LIVE > rollout NEAR_LIVE > HISTORICAL
```

Authority is applied per field and per source lane. A lower source never
overwrites a non-null higher-authority observation. If a higher source has no
value, a lower source may remain available as a separately labelled
supplemental observation; it must not be relabelled as LIVE.

## Source Envelope contract

The adapter boundary should emit one envelope per complete source record:

```ts
type SourceTemporalClass = "LIVE" | "NEAR_LIVE" | "HISTORICAL";

type SourceKind =
  | "monitor-app-server"
  | "codex-cli-rollout"
  | "historical-rollout-scan";

type EvidenceConfidence = "confirmed" | "derived" | "unknown";

interface SourceEnvelope<TRecord = unknown> {
  envelopeVersion: 1;
  observationId: string;
  sourceKind: SourceKind;
  temporalClass: SourceTemporalClass;
  sourceInstanceId: string;
  codexHome: {
    normalizedPath: string;
    identity: string;
  } | null;
  sourceFile: {
    normalizedPath: string;
    filesystemId: string | null;
    generation: string;
    sessionMetaId: string | null;
  } | null;
  cursor: {
    byteStart: number;
    byteEnd: number;
    recordOrdinal: number;
    lineHash: string;
  } | null;
  timestamps: {
    sourceTimestampMs: number | null;
    sourceTimestampKind: "record" | "lifecycle" | "filesystem" | "none";
    observedTimestampMs: number;
    lagMs: number | null;
  };
  freshness: {
    state: "fresh" | "stale" | "settled" | "unknown";
    lastCompleteRecordObservedAtMs: number | null;
    reason: string;
  };
  schema: {
    producer: "codex-app-server" | "codex-rollout";
    producerVersion: string | null;
    recordSchema: string;
  };
  confidence: {
    level: EvidenceConfidence;
    basis: string[];
  };
  record: TRecord;
}
```

Contract rules:

- `sourceInstanceId` identifies a producer/adapter instance, not a Codex Thread.
  App-server connections and rollout tailers therefore have different source
  instance IDs even when they observe the same Thread.
- `codexHome.identity` is derived from the normalized real path and filesystem
  namespace. Display casing, environment-variable spelling, and symlink spelling
  are not identities.
- File cursors count UTF-8 bytes, not characters. `byteEnd` advances only past a
  complete LF-terminated JSON record.
- `generation` changes when filesystem identity changes, the file is replaced, or
  observed length becomes smaller than the committed cursor.
- `sourceTimestampMs` is never replaced by `observedTimestampMs`. The latter is
  local observation evidence only.
- Confidence is evidence metadata, not permission to invent a value.
- Producer schema version is represented by `cli_version` plus a tested record
  fingerprint; no standalone rollout schema version field was observed.

## Rollout identity and hierarchy

### Root Session

The root file begins with `type="session_meta"`. In every root CLI capture:

```text
payload.id == payload.session_id == filename UUID suffix
```

`payload.id` is the canonical Thread identity. The path date/time uses local time;
the record's top-level ISO timestamp is UTC and is the source timestamp.

### Sub-Agent

Each direct Sub-Agent created a separate rollout file. Its first record showed:

```text
payload.id          = child Thread ID
payload.session_id  = root Main Session ID
payload.source.subagent.thread_spawn.parent_thread_id = direct parent Thread ID
payload.source.subagent.thread_spawn.depth             = 1
payload.source.subagent.thread_spawn.agent_path         = /root/<task>
payload.thread_source                                   = subagent
```

The parent `spawn_agent` function-call output contained the task path but did not
contain the child Thread ID. A failed spawn request also existed without a child
file. Assignment creation must therefore use the child `session_meta` parent edge,
not the parent's spawn request or task name.

`payload.session_id` must not be used as the child Thread ID and must not be used
as a direct-parent field: all three children shared the root Session ID.

## Model evidence

`session_meta` contained `model_provider` but no model name. The resolved per-Turn
model appeared in:

```text
type="turn_context" -> payload.turn_id, payload.model, payload.effort
```

The explicit-model run recorded `gpt-5.6-terra`. The three child files recorded
`gpt-5.6-luna`. This is confirmed rollout execution-context evidence, but its
temporal class remains NEAR LIVE and its provenance must remain distinct from the
Phase 1 app-server observed-model sources.

The environment variable `CODEX_INTERNAL_ORIGINATOR_OVERRIDE=Codex Desktop` was
inherited by the PowerShell-launched CLI, so `session_meta.originator` was `Codex
Desktop` even when `source` was `exec`. `originator` cannot classify CLI versus
Desktop by itself.

## Token evidence

Rollout Token records use:

```text
type="event_msg"
payload.type="token_count"
payload.info.total_token_usage
payload.info.last_token_usage
payload.info.model_context_window
```

Observed fields were `input_tokens`, `cached_input_tokens`,
`cache_write_input_tokens`, `output_tokens`, `reasoning_output_tokens`, and
`total_tokens`.

Evidence confirms the Phase 1 arithmetic rules:

- `total_tokens = input_tokens + output_tokens` in all retained samples;
- cached input is a subset of input and is not added again;
- reasoning output is a subset of output and is not added again;
- the second resumed Turn changed the Thread cumulative total from `18,320` to
  `38,243`, while its `last` value was `19,923`;
- Main and child files had independent snapshots. The Main total was `125,521`;
  child totals were `16,629`, `16,601`, and `16,594`. They are not merged.

## Turn lifecycle and status evidence

| Runtime meaning | Rollout evidence | Boundary |
| --- | --- | --- |
| Thread discovered | `session_meta.payload.id` | Discovery alone does not prove the producer remains active. |
| Turn Running | `event_msg.payload.type="task_started"` with `turn_id` and `started_at` | Explicit NEAR-LIVE lifecycle evidence. |
| Turn Waiting | Pending `response_item.function_call` named `wait_agent`, followed by matching output | Confirmed for CLI `0.147.0`; adapter-specific mapping requires the call ID. |
| Turn Completed | `event_msg.payload.type="task_complete"` with `completed_at` and `duration_ms` | Completes the Turn, not the reusable Thread. |
| Turn Failed | Not captured | Must remain unimplemented until captured. |
| Thread idle | No direct rollout status record captured | A completed latest Turn is evidence of no later known active Turn, not proof that the process remains attached. |

Top-level record timestamps are ISO UTC. Lifecycle payloads additionally provided
epoch-second `started_at`/`completed_at` and millisecond `duration_ms`. The
lifecycle timestamp has semantic priority; the top-level timestamp is the record
write timestamp; local observation time remains separate.

## File append behavior

- A new `codex exec` created a new date-partitioned rollout file.
- `codex exec resume <SESSION_ID>` in a new process appended to the original file,
  retained the Thread ID, and created a new Turn ID.
- No in-Session rotation was observed.
- No successful run or resume truncated its file.
- Byte length changed in batches, including a `18,713 -> 70,916` jump. A tailer
  must parse zero or more complete records per wakeup.
- Across 50 ms target polling, every non-empty sampled state ended in LF. No
  partial line was observed, but this does not prove partial writes impossible.
  The adapter must buffer an incomplete final line.
- On Windows, `LastWriteTimeUtc` remained at file creation time through observed
  active appends and changed after the writer closed. Polling mtime alone is not a
  valid tail strategy.
- Opening one actively written rollout with the default exclusive StreamReader
  failed. Opening with read sharing succeeded. The Rust adapter must use sharing
  compatible with active writers and file moves/deletes.
- Natural truncation was not observed. `length < committed byteOffset` remains a
  required defensive reset boundary, not a claimed Codex behavior.

## Exact app-server and rollout pairing

The paired probe produced one app-server Turn and its rollout. The following were
exact matches:

```text
app-server thread.id          == rollout session_meta.payload.id
app-server thread.sessionId   == rollout session_meta.payload.session_id
app-server thread.path        == observed rollout canonical path
app-server turn.id            == rollout task_started/task_complete turn_id
app-server settings.model     == rollout turn_context.model
app-server token total/last   == rollout token_count total/last
app-server turn.durationMs    == rollout task_complete.duration_ms
```

This establishes deterministic cross-source identity for a common CODEX_HOME.

## Cross-source deduplication and authority

The canonical entity key is:

```text
CodexThreadKey = (codexHome.identity, full threadId)
CodexTurnKey   = (CodexThreadKey, full turnId)
```

The strongest pairing evidence is exact canonical `thread.path` plus exact Thread
ID. The rollout filename UUID suffix is validation only; `session_meta.payload.id`
is authoritative. Sources lacking a trustworthy CODEX_HOME identity must not be
silently merged only because display paths or titles match.

Each source maintains an independent observation lane:

```text
Thread entity
├── app-server LIVE lane
├── rollout NEAR-LIVE lane
└── HISTORICAL lane
```

Fusion rules:

1. Do not convert cross-source records into one shared additive event stream.
2. Never add app-server and rollout Token values. They are duplicate snapshots of
   the same model usage when Thread and Turn IDs match.
3. While a matching app-server LIVE lane is healthy, it drives Runtime lifecycle,
   model, and Token. The rollout lane continues advancing its cursor but cannot
   overwrite LIVE state.
4. A lower lane may supply a missing value only as a value bearing its original
   temporal class and provenance. It is never relabelled LIVE.
5. On failover from LIVE to rollout, select a rollout cumulative snapshot; do not
   apply its delta to the last LIVE snapshot. If the rollout snapshot lags behind
   the last LIVE total, retain the last LIVE value as stale until rollout catches
   up rather than moving backwards.
6. Within one rollout lane, idempotence uses file generation plus byte range and
   line hash. Across sources, authority selection replaces event-level deduplication
   because app-server and rollout records are not guaranteed one-to-one.
7. HISTORICAL never drives Running, Waiting, or active Session selection.

## Reuse boundary with Phase 1 Runtime

Can be normalized into the existing concepts with rollout provenance:

- `session_meta.payload.id` -> Thread identity/discovery;
- child `source.subagent.thread_spawn` -> AgentAssignment;
- `task_started` -> Turn running;
- pending/completed `wait_agent` call -> Turn waiting/running transition;
- `task_complete` -> Turn completed;
- `turn_context.model` -> rollout-confirmed model observation;
- `token_count.total_token_usage` -> Thread cumulative Token snapshot;
- `token_count.last_token_usage` -> Turn Token increment;
- lifecycle and record timestamps -> source timestamps.

Must remain outside Live Runtime or separately labelled:

- prompts, response text, reasoning, world state, developer instructions, and
  shell snapshots;
- model totals derived by historical scans;
- file mtime-based activity guesses;
- stale unfinished Turns after a crash;
- inferred titles or parentage;
- rate limits and account data;
- any `originator`-based CLI/Desktop classification.

The existing `src-tauri/src/shared/local_usage_core.rs` is a complete-file History
scanner. Its parsing knowledge may be factored into shared rollout record helpers,
but its historical aggregation must not become the Tail Adapter or feed Live state.

## Recommended next implementation boundary

Backend shared core first:

```text
src-tauri/src/shared/global_sources_core.rs
src-tauri/src/shared/global_sources_core/source_envelope.rs
src-tauri/src/shared/global_sources_core/rollout_record.rs
src-tauri/src/shared/global_sources_core/rollout_identity.rs
src-tauri/src/shared/global_sources_core/rollout_tail.rs
src-tauri/src/shared/global_sources_core/source_registry.rs
```

Suggested shared API:

```rust
discover_rollout_sources(codex_homes) -> Vec<SourceDescriptor>
read_rollout_delta(file_identity, cursor) -> RolloutDelta
normalize_rollout_record(envelope) -> Vec<ExternalRuntimeObservation>
reconcile_source_authority(thread_key, observations) -> AuthoritativeThreadView
```

The first coding slice should implement the contract, parser, byte cursor, partial
line buffer, file-generation reset, and fixture tests without an OS watcher. A
watcher can then call the same `read_rollout_delta` API. App/Tauri, daemon RPC,
frontend IPC, and event fanout should be added only when the shared core contract
is stable, preserving app/daemon parity.

No Desktop source, UI change, full watcher, Phase 1 Runtime semantic change, or
release packaging belongs in this slice.

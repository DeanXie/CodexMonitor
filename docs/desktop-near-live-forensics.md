# Codex Desktop Near-Live Forensics

Status: **DESKTOP FORENSICS PASS — SLICE 1 PASS — SLICE 2 PASS — REAL E2E A/B/C/D PASS — FINAL UI PASS — PHASE 2.5 PASS**
Captured: 2026-08-27 (Asia/Shanghai)
Design amendment: 2026-08-29 — Desktop projection authority and stale-orphan admission
Real E2E closeout: 2026-08-31 — Gates A/B/C/D PASS

Implementation status: Desktop forensics **PASS**; File Owner / Replay
Guard / Child Execution Boundary **PASS**; Desktop Metadata + Producer Surface
Classifier **PASS**; Desktop Near-Live Real E2E **PASS**; Final Agent Monitor UI
**PASS**; Phase 2.5 **PASS**; Phase 2 Global Sources **COMPLETE**.

This report contains protocol and metadata facts only. Raw prompts, reasoning,
agent messages, credentials, and unrestricted diagnostics are excluded.

## Executive conclusion

Codex Desktop writes its Main and Sub-Agent sessions into the same default
`CODEX_HOME/sessions/**/rollout-*.jsonl` layout already watched by Global Source
Core. The standard identity, parent relation, model, token, lifecycle, cursor,
checkpoint, and authority machinery is reusable.

Direct reuse originally was not safe for every Desktop Sub-Agent rollout. A real child
spawned from the long/compacted Desktop Thread contained a file-owner child
`session_meta`, followed by replayed parent history containing a second parent
`session_meta`. Slice 1 now fixes the first valid generation owner and guards the
replayed prefix until a child execution boundary is confirmed. Slice 2 adds the
read-only Desktop metadata, Producer Surface, workspace mapping, and stale-orphan
authority gates required before Real E2E.

Real E2E then confirmed both lifecycle orders, stale-orphan isolation, and
Desktop/CLI separation against real Desktop tasks, real rollouts, read-only
Desktop metadata, and the current canonical projection. The durable accepted
evidence is indexed by `docs/evidence/phase-2-5-real-e2e-summary.md`.

Desktop sidebar visibility and `local_thread_catalog` membership are not
canonical Thread-existence evidence. They are supplemental projection metadata
only.

## Evidence set

Real root Thread:

- `01a02b9e-d94a-7610-a244-630973ba2d02`
- rollout first created 2026-08-23 local time and still appended during this
  probe, proving multi-Turn/resume-on-one-file behavior.
- `session_meta.source = vscode`
- `thread_source = user`
- `cwd = F:\AI\CodexMonitor`
- producer version at file creation: `0.149.0-alpha.4.1`

Real direct children:

| Agent path | Child Thread | Parent evidence | Final observed model | Final direct token snapshot |
| --- | --- | --- | --- | ---: |
| `/root/desktop_files` | `01a03f2b-caa9-76f1-830c-809502517430` | child header `thread_spawn.parent_thread_id` | `gpt-5.6-luna` | 199,140 |
| `/root/desktop_runtime` | `01a03f2b-d9a2-73b2-8f08-c9695a37787e` | child header `thread_spawn.parent_thread_id` | `gpt-5.6-luna` | 496,765 |
| `/root/desktop_workspace` | `01a03f2b-eab0-71e0-aeeb-47d9ec909cbe` | child header `thread_spawn.parent_thread_id` | `gpt-5.6-luna` | 1,134,811 |

Each child had a separate rollout file and an independent child Turn. Each
contained real `task_started`, `turn_context`, cumulative `token_count`, and
`task_complete` records after the replayed prefix.

Sanitized fixtures live in `docs/fixtures/desktop-rollout/`.

## 1. Actual CODEX_HOME

- The Desktop process had no explicit `CODEX_HOME` environment value.
- The active root and all three new children were written under
  `C:\Users\DeanX\.codex\sessions\YYYY\MM\rollout-*.jsonl`.
- Desktop and local CLI records coexist in that sessions tree.
- No workspace-specific CODEX_HOME was observed for this Desktop project.
- Global Source discovery must continue to support multiple configured homes;
  this probe confirms only the active default home, not that Desktop can never
  use another home.

## 2. Desktop versus CLI rollout schema

Confirmed common records:

- `session_meta`
- `turn_context`
- `event_msg.task_started`
- `event_msg.task_complete`
- `event_msg.token_count`
- Sub-Agent `session_meta.source.subagent.thread_spawn`
- response-item wait/resume records where the relevant tool is used

Root-source observations differ by launch surface in this dataset:

- Desktop Main: string source `vscode`.
- external CLI/exec samples: string source `cli` or `exec`.
- Sub-Agent: structured `subagent.thread_spawn` for both surfaces.

These values are evidence, not a sufficient source classifier. IDE Codex may
also use `vscode`, and a CLI can inherit
`CODEX_INTERNAL_ORIGINATOR_OVERRIDE=Codex Desktop`.

### Compacted child prefix boundary

The captured Desktop child ordering was:

1. child file-owner `session_meta` with confirmed parent and `agent_path`;
2. replayed parent `session_meta`;
3. inherited parent `task_started` / `turn_context`;
4. compaction/history records;
5. `thread_settings_applied`;
6. actual child `task_started` / `turn_context`;
7. child token updates and completion.

The existing adapter currently allows item 2 to replace the owner established
by item 1. Its checkpoint then contains the parent Thread key, no parent key,
no agent path, and the child's token/model/lifecycle under the parent lane.

Required rule: source-file ownership is pinned by the first complete
`session_meta` in a generation. Later replayed `session_meta` records must not
replace it. Events from the replayed prefix must not drive the child lifecycle
or token snapshot until the child execution boundary is proven.

The exact generic execution-boundary rule needs fixture-driven implementation.
`thread_settings_applied` followed by a new `task_started` is present in all
three captured children, but it is not yet declared a universal protocol
guarantee.

## 3. Runtime identity

The existing keys remain valid:

```text
CodexThreadKey = (codexHome.identity, fullThreadId)
CodexTurnKey   = (CodexThreadKey, fullTurnId)
```

For a child file, `fullThreadId` is the first file-owner
`session_meta.payload.id`. `payload.session_id` remains the root Main Session
ID and must not replace the child Thread ID.

Parent/child evidence remains exclusively:

```text
session_meta.source.subagent.thread_spawn.parent_thread_id
```

`agent_path` comes from the same confirmed structure. Titles, cwd adjacency,
file adjacency, spawn requests, and inherited history are not parent evidence.

## 4. Model, token, lifecycle, and timestamps

- Model: available from the child execution's `turn_context.model`.
- Token: available as an independent cumulative snapshot from
  `token_count.info.total_token_usage`.
- Lifecycle: actual child `task_started` and `task_complete` were present.
- Turn IDs: present on actual child start/context/complete records.
- Source timestamps: RFC3339 record timestamps.
- Observed timestamps: assigned by the watcher and kept separate.

Token remains per Thread. Parent and child snapshots must not be added, and
rollout snapshots must not be added to a matching app-server LIVE lane.

The Desktop state database also retained model and parent/child metadata, but
its spawn-edge status remained `open` after child completion. Therefore that
status is Thread-container metadata, not proof of active lifecycle.

## 5. Near-Live lag and append behavior

The existing Watch Service was already started at
`2026-08-27T01:36:58.278+08:00`. New child file headers were observed as:

| Child | Source timestamp | Monitor observed | Initial lag |
| --- | --- | --- | ---: |
| desktop_files | 01:43:49.759 | 01:43:50.158 | 399 ms |
| desktop_runtime | 01:43:53.588 | 01:43:54.658 | 1,070 ms |
| desktop_workspace | 01:43:57.955 | 01:43:58.606 | 651 ms |

Across all allow-listed observations from the three files:

- minimum: 8 ms
- per-file p50: 398–815 ms
- per-file p95: 860–1,227 ms
- maximum: 1,334 ms

The producer emitted complete records in batches: an initial group followed by
individual model/token/lifecycle records over approximately 21–98 seconds.
Watcher reconciliation observed each completed record within the distribution
above. This is NEAR LIVE, not LIVE or per-token streaming.

No naturally partial final JSON line or file truncation occurred in this probe.
Existing UTF-8 byte cursor, partial-line buffering, generation/reset, retry,
and checkpoint rules remain required defenses.

## 6. Multi-Turn and startup timing

- Multi-Turn/resume: confirmed. The same Main rollout created on 2026-08-23
  continued to append during this 2026-08-27 probe; no new Main file was made.
- Monitor first, Desktop activity second: confirmed. The watcher was started
  before the three child rollouts were created and observed each automatically.
- Desktop Thread first, Monitor second: existing Main identity/cursor recovery
  and later append were observed after a Monitor restart, using the persisted
  checkpoint. This probe did not strictly prove that a Desktop Turn was already
  in Running state before that particular service start. File existence alone
  must not be used to infer Running. A strict active-before-Monitor E2E remains
  an acceptance test for the eventual adapter, not a blocker to its TDD work.

## 7. Workspace and project mapping

Evidence sources and authority:

1. Rollout `cwd`: protocol-backed and sufficient for a best-effort match to a
   configured CodexMonitor workspace, using normalized paths and longest-root
   matching. It is not Thread identity.
2. Desktop `.codex-global-state.json`:
   - `thread-project-assignments` mapped the real root Thread to a local project;
   - `local-projects` supplied the project name and two root paths;
   - `thread-writable-roots` supplied the Thread's writable roots.
3. `state_5.sqlite`:
   - `threads` supplied cwd, model, agent path, and root metadata;
   - `thread_spawn_edges` confirmed the three parent/child edges;
   - its current Main `project_id` was null, so it is not sufficient alone.
4. `session_index.jsonl`: supplied an ID/title record but its timestamp stayed
   at Thread creation, so it is title/history metadata, not a live workspace or
   lifecycle source.

Presence in, or absence from, `session_index.jsonl` is not a necessary
condition for canonical Thread existence. Valid Threads may be absent from that
index, so it remains supplemental evidence only.

Recommended mapping:

- primary: normalized rollout cwd to configured workspace roots;
- supplemental: Desktop project assignment and local-project roots, with
  explicit private-schema provenance;
- children: inherit a confirmed parent's workspace only when the child has a
  confirmed parent relation; prefer the child's own cwd when present;
- unresolved: show an unassigned/unknown workspace rather than guess.

Desktop state files are private, mutable implementation details. Readers must
be defensive and optional; their absence or schema drift must not stop rollout
observation.

## 8. Desktop source identity

`originator == Codex Desktop` is insufficient and must remain weak supporting
evidence only.

Recommended classifier, in descending authority:

1. A matching fresh app-server LIVE lane owned by CodexMonitor means
   Monitor-owned, regardless of rollout originator.
2. A root rollout with `source=vscode` plus membership in Desktop-maintained
   project/thread state is strong Desktop evidence. Direct children inherit
   that surface classification only through a confirmed parent edge.
3. `source=cli` or `source=exec` without Desktop membership is CLI-like evidence.
4. `source=vscode` without corroborating Desktop membership remains ambiguous
   because a later IDE source may use the same value.
5. Conflicting or missing evidence remains unknown/ambiguous.

The current `SourceKind` enum has no Desktop or ambiguous rollout value. Formal
coding therefore needs an explicit contract decision: add
`codex-desktop-rollout` and `ambiguous-rollout`, or add a separate producer
surface classification to canonical snapshots. Continuing to label confirmed
Desktop data as `codex-cli-rollout` would be misleading.

## 9. Canonical existence, deduplication, and authority

The adapter must keep these concepts separate:

```text
canonical Thread existence
!= Desktop local_thread_catalog membership
!= Desktop sidebar visibility
```

`local_thread_catalog`, `.codex-global-state.json`, project membership, Desktop
sidebar state, and Desktop WebView/cache data are Desktop-owned supplemental
projection metadata. None may independently create a canonical Thread, a
Registry lane, an Agent Runtime, or an Agent Monitor node.

The evidence hierarchy is:

```text
Monitor deletion tombstone
>
confirmed rollout identity
>
authoritative app-server/persisted Thread state
>
Desktop projection metadata
```

A Monitor deletion tombstone is final for the same `CodexThreadKey`: later
Desktop catalog/sidebar observations must not resurrect it. Title equality is
not identity; a different full thread id remains an independent Thread.

### `DESKTOP_STALE_ORPHAN`

`DESKTOP_STALE_ORPHAN` means Desktop catalog/sidebar projection still references
a complete full thread id while the canonical Thread no longer exists. It is a
diagnostic/Desktop-projection observation only:

- it does not enter `LIVE`;
- it does not enter `NEAR_LIVE`;
- it does not enter the canonical `HISTORICAL` Registry;
- it does not create an Agent Runtime or Agent Monitor node.

If a Monitor deletion tombstone exists for the same `CodexThreadKey`, canonical
ingest is rejected immediately and a matching Desktop projection is stale.
Without a tombstone, `DESKTOP_STALE_ORPHAN` requires all of the following:

- Desktop catalog/sidebar contains the exact full thread id;
- no confirmed rollout identity exists;
- authoritative persisted Thread state is absent;
- `thread/read` explicitly reports nonexistent or an equivalent not-found result.

Absence from `session_index.jsonl` is not part of this test. Incomplete or
contradictory evidence remains `AMBIGUOUS`; the adapter must not guess canonical
existence or ingest it.

All observations of the same
`(codexHome.identity, fullThreadId)` share one canonical Thread with separate
lanes:

```text
Thread
├── monitor app-server LIVE
├── rollout NEAR_LIVE (surface: Desktop / CLI / ambiguous)
├── Desktop metadata supplemental lane
└── HISTORICAL
```

- LIVE remains authoritative while fresh.
- rollout continues advancing its cursor without entering the additive LIVE
  event stream.
- LIVE-to-rollout fallback uses cumulative snapshots and cannot reduce tokens.
- Desktop metadata may supplement workspace/title/source evidence, but cannot
  override lifecycle, model, or token facts from a higher-authority lane.
- Historical data never drives Running/Waiting or Current Session.

This works only after the file-owner/replayed-history gate is fixed; otherwise
different child files can collapse into the parent key before reconciliation.

## 10. Reusable modules

Directly reusable after the parser gate:

- CODEX_HOME discovery and stable identity
- rollout file discovery
- UTF-8 byte cursor and partial-line buffer
- generation/reset handling
- checkpoint persistence
- filesystem wake-up plus periodic reconciliation
- Windows shared-read retry/backoff
- record parser for confirmed record families
- `CodexThreadKey` / `CodexTurnKey`
- Source Authority Registry and cumulative token rules
- backend snapshot/update API
- Global Source View Store and canonical selector

No second watcher or Phase 1 Runtime path is needed.

## 11. Required additions

Minimal formal implementation slices:

1. File-owner/replay guard in the rollout adapter, driven by the compacted-child
   fixture. It must pin the first `session_meta` and suppress inherited prefix
   state from the child lane until a child execution boundary is confirmed.
2. Read-only Desktop metadata adapter for `.codex-global-state.json` and,
   optionally, `state_5.sqlite` or `codex-dev.db` / `local_thread_catalog`,
   isolated from Source Core protocol parsing.
3. Source-surface classifier with evidence/confidence/provenance and an explicit
   unknown/ambiguous outcome.
4. Contract/snapshot representation for Desktop versus ambiguous rollout
   source, requiring approval because the current `SourceKind` enum is closed.
5. TDD fixtures for Desktop root, compacted child, source ambiguity, metadata
   absence/schema drift, workspace matching, dedup, and authority isolation.
6. Desktop stale-orphan admission fixtures:

   | Fixture | Expected result |
   |---|---|
   | `local_thread_catalog` contains full id; rollout absent; authoritative Thread absent; `thread/read = nonexistent` | `DESKTOP_STALE_ORPHAN`; canonical ingest rejected; Agent Monitor node not created |
   | stale catalog row plus matching Monitor deletion tombstone | tombstone wins; canonical ingest rejected |
   | legitimate Desktop Thread plus catalog row plus valid confirmed rollout | eligible for normal canonical classification; not stale orphan |
   | catalog-only evidence with insufficient authority evidence | `AMBIGUOUS`; no guessed canonical ingest |
   | same title but different full thread id | independent Thread; unrelated tombstone has no effect |

## 12. Coding gate

**Formal TDD Slice 1 and Slice 2 are PASS. Desktop Near-Live Real E2E Gates
A/B/C/D and Final Agent Monitor UI are PASS. Phase 2.5 is PASS and Phase 2
Global Sources is COMPLETE.**

Completed backend order:

1. `Desktop Compacted Child Rollout -> File Owner -> Replay Guard -> Child Execution Boundary`;
2. Desktop metadata/workspace tests, stale-orphan admission gate, and read-only
   adapter;
3. source-surface contract decision and classifier;
4. canonical registry integration;
5. real A/B/C/D Desktop E2E.

Final Agent Monitor UI delivers the Desktop producer-surface filter/label,
canonical Main/Sub-Agent hierarchy and fields, projection-only exclusion,
focused UI coverage, and Current Session isolation. This closeout does not
start Phase 3.

## 13. Remaining boundaries

- Gate B confirmed Desktop-active-before-Monitor cold discovery, catch-up,
  continued Main tail, and completion observation. The Child completed before
  Monitor start, so no Child continued-tail sample is claimed.
- `thread_settings_applied` was observed as the child execution boundary but is
  not yet proven universal across versions and launch surfaces.
- Desktop private JSON/SQLite schemas are not stable public protocol contracts.
- `source=vscode` is not uniquely Desktop.
- Natural partial-line, truncate, and rotation behavior did not occur in this
  probe; existing defensive watcher tests remain the evidence for those cases.
- Desktop metadata cannot drive Running/Waiting/Completed by itself.
- Do not modify `codex-dev.db`, `local_thread_catalog`, `state_5.sqlite`,
  `.codex-global-state.json`, or Desktop WebView/cache data. Phase 2.5 observes,
  classifies, and diagnoses only.
- Stale-orphan admission is implemented in Slice 2 as a diagnostic projection
  gate and cannot create a canonical Registry node.

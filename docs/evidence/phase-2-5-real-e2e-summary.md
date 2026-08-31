# Phase 2.5 Desktop Near-Live Real E2E Summary

Captured: 2026-08-31 (Asia/Shanghai)

```text
Phase 2.5 Desktop forensics = PASS
Slice 1 File Owner / Replay Guard = PASS
Slice 2 Metadata + Producer Surface = PASS
Real E2E A/B/C/D = PASS
Final Agent Monitor UI = PASS
Phase 2.5 = PASS
Phase 2 Global Sources = COMPLETE
```

**Desktop Near-Live Real E2E = PASS**

| Gate | Result | Durable conclusion |
| --- | --- | --- |
| Gate A — Monitor First | PASS | A real Desktop Main and Sub-Agent were captured as distinct canonical `NEAR_LIVE` Threads with confirmed parent edge, `DESKTOP` producer classification, authoritative model/lifecycle/cumulative Token, workspace assignment, and continued tail. |
| Gate B — Desktop First | PASS | Monitor cold-discovered a running Desktop Main and its completed Child, reconstructed current state from backlog, retained the parent edge, continued tailing the Main at 406–803 ms, and observed Main completion at 293 ms. |
| Gate C — Stale Orphan | PASS | The confirmed stale Desktop projection was classified `DESKTOP_STALE_ORPHAN`, admitted no canonical lane, created zero Registry or Agent Monitor nodes, and did not modify Desktop private databases. |
| Gate D — Surface Separation | PASS | A real Desktop Thread remained `DESKTOP`; a real external exec Thread remained `CLI` even with weak `originator=Codex Desktop` evidence. |

## Evidence set

- Gate A Main: `docs/evidence/phase-2-5-gate-a-main.json`
- Gate A Child: `docs/evidence/phase-2-5-gate-a-child.json`
- Gate B Main: `docs/evidence/phase-2-5-gate-b-main.json`
- Gate B Child: `docs/evidence/phase-2-5-gate-b-child.json`
- Gate C stale orphan: `docs/evidence/phase-2-5-gate-c-stale-orphan.json`
- Gate D CLI separation: `docs/evidence/phase-2-5-gate-d-cli.json`

## Gate B accepted measurements

- Main: `01a057a5-8e59-7ae2-8688-6a829779f4f3`; `DESKTOP / confirmed`; `gpt-5.6-sol`; Completed; 303,697 cumulative tokens.
- Child: `01a057a6-5aac-7b71-9a94-ce38dcd8e246`; `DESKTOP / inferred from confirmed parent`; `gpt-5.6-luna`; Completed; 280,015 cumulative tokens.
- Parent edge: Child → Main, confirmed by `session_meta.source.subagent.thread_spawn.parent_thread_id`.
- Workspace: `cb31e463-fe8f-46cc-b6f7-7c0bd7820c74` / `F:\AI\CodexMonitor` for both Threads.
- Monitor process start: `2026-08-31T11:49:46.918Z`; Global Source service start: `2026-08-31T11:49:47.330Z`.
- First canonical observation: Main 503 ms after process start; Child 648 ms after process start.
- Catch-up observation range: Main 5.157–121.040 seconds; Child 5.429–72.215 seconds.
- Continued-tail latency: Main 406–803 ms; Child had completed before Monitor start and therefore has no claimed continued-tail sample.
- Completion latency: Main 293 ms; Child completion was reconstructed during catch-up with 5.429 seconds observation lag.
- Canonical result: one `NEAR_LIVE` lane per Thread, no parent/child collapse, no duplicate node, no Token double count.

## Closeout boundary

The four Real E2E defects found before Gate B were fixed and independently committed as `d617b7f fix: close phase 2.5 real e2e gaps`. Gate B itself was read-only and produced no source modification. The final closeout does not reopen or redesign Global Source Core and does not start Phase 3.

## Final Agent Monitor UI delivery

The final Agent Monitor UI is complete:

1. Desktop producer-surface label/filter remains distinct from `LIVE` / `NEAR_LIVE` transport class.
2. Desktop Main/Sub-Agent hierarchy shows workspace, authoritative model, lifecycle, cumulative Token, freshness, and latest activity from the canonical projection.
3. Projection-only `DESKTOP_STALE_ORPHAN` and `AMBIGUOUS` observations remain absent from Agent Monitor nodes.
4. Focused selector/component/page tests cover Desktop filtering, labels, hierarchy, deduplication, and stale-orphan exclusion.
5. External Desktop Threads remain ineligible for Current Session and cannot pollute Current Session selection.

## Locale baseline waiver

Known non-blocking test debt: 6 pre-existing zh-CN locale/date assertions.

- Full frontend: 1088 / 1094 PASS.
- New Phase 2.5 regressions: 0.
- The six failures existed before Phase 2.5, are unrelated to Phase 2.5 files and behavior, and are not a Phase 2.5 blocker.
- This closeout does not fix those six assertions and does not start Phase 3.

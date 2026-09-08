# Phase 3.4.5 Focused Cross-Surface E2E

Status: **PASS / COMPLETE**. Phase 3.4 Cross-Surface Projection Reconciliation is **PASS / COMPLETE**. Phase 3.5.0 Remote / Mobile Interoperability Forensics & Contract is **FORENSICS COMPLETE**; Phase 3.5.1 is **GO / NOT STARTED**.

## Final gate matrix

| Gate | Result | Authoritative evidence |
| --- | --- | --- |
| A — Monitor-created Standard Session | PASS | Monitor created Thread `01a07d70-8fb7-7bf0-ae46-a0bae5f2f826`; First Turn `01a07d70-9060-7501-b16b-3af0af8ccc11` completed; no duplicate Thread was created. |
| B — Cross-Surface continuation | PASS | Monitor → Desktop → CLI used the same full Thread ID and independent Turns. Desktop loaded/open correctly blocked CLI with active-writer code `-32600`; after Desktop released the writer, CLI exact-ID resume succeeded and retained Monitor/Desktop history. |
| C — Canonical PRESENT + Desktop negative projection | PASS | Thread `01a080bd-66b1-7e82-8c02-3759c32c4283`, Codex home `codex-home:76c92e1eaaa451ef02c92f75f12738ea3337e69b890333a72cd14ec778cac75f`: canonical PRESENT, one Agent node, zero duplicates; matching-home COMPLETE Desktop Catalog exact miss produced ABSENT and UI `Not present in this surface`; no Sidebar or Project projection was inferred. |
| D — Real Desktop stale orphan | PASS | Official Thread deletion left an exact Desktop Catalog projection. Projection Issues showed the retained deleted-Thread projection, `Waiting for Desktop to refresh`, and `Observe only`; canonical Agent/runtime nodes remained zero and no repair action existed. |
| E — Restart persistence | PASS | After Monitor restart the same stale/pending Projection Issue was restored while canonical Agent/runtime nodes remained zero. |
| F — Natural Desktop reconciliation | NOT OBSERVED IN THIS E2E WINDOW | Desktop did not naturally remove the stale projection during the bounded window. Desktop remains OBSERVE_ONLY / UNSUPPORTED; no private-state write was used to manufacture RECONCILED. COMPLETE absence → RECONCILED remains covered by engine/integration tests. |
| G — UI truthfulness | PASS | Canonical status, projection status, reconciliation, and capability remained independent. PRESENT, ABSENT, and STALE paths used their frozen text; pending observe-only Desktop state never claimed active repair and exposed no Repair / Refresh / Invalidate action. |
| H — Activity timestamp | PASS | UI showed `Latest activity: 2026/9/8 05:21:10` and retained `Created: 2026/9/8 05:21:02`, proving reliable activity and creation time remain separate. |
| I — Isolation / no regression | PASS | E2E node/dedup observations and automated regressions preserved canonical identity, tombstones, active-writer protection, Workspace/Project independence, Sidebar/Project non-inference, Current Session, Producer/Source, and token/runtime isolation. |

Gate F is an allowed observed boundary and does not block acceptance. Therefore Phase 3.4.5 is **PASS / COMPLETE**, and Phase 3.4 is **PASS / COMPLETE**.

## Corrective slice

Phase 3.4.5a is **PASS / FROZEN**. Desktop Catalog reports inventory and coverage per actually observed Codex home, and the backend joins canonical PRESENT keys only by `(codexHomeIdentity, fullThreadId)`. A matching-home COMPLETE miss produces ABSENT; FAILED / NOT_OBSERVED / BOUNDED / PARTIAL miss produces UNKNOWN; no matching-home report produces no observation. Empty COMPLETE inventory is valid negative evidence. The production singleton-ID + hardcoded-COMPLETE shortcut is removed. Canonical authority and stale-orphan semantics are unchanged.

## Environment and diagnostic boundaries

Development instances must be started through the Tauri dev harness so Vite is started before the shell:

```powershell
npm --prefix "F:\AI\CodexMonitor\.worktrees\<worktree>" run tauri:dev:win
```

Directly launching `target\debug\codex-monitor.exe` does not start Vite and can produce `localhost:1420` connection refusal; an unrelated worktree owning that port can also serve the wrong frontend. This is a dev harness / launch-path issue, not a projection product-logic failure, and is not classified as a confirmed single-instance bug.

The observed `state_5.sqlite: private-schema-drift: threads.project_id is missing` message remains a Desktop private-schema diagnostic. It does not change Desktop Catalog COMPLETE coverage, canonical identity, Desktop Project assignment, or Gate C.

Desktop private databases, catalog, sidebar, global state, and Project assignments remained read-only throughout acceptance. Phase 3.5.0 forensics are complete; Phase 3.5.1 product development has not started.

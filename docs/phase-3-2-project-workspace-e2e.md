# Phase 3.2 Project / Workspace Interoperability E2E

Phase 3.2.5 is PASS. Phase 3.2 Project / Workspace Interoperability is PASS / COMPLETE.

The acceptance set combines executable shared-core fixtures with isolated CLI,
Monitor-project, and direct app-server Threads. The frozen identity boundary held:

```text
Thread identity != Workspace identity != Desktop Project identity
Workspace = execution environment + normalized root
Desktop Project = Desktop surface organization projection
```

## Gate results

| Gate | Creator | Workspace state | Desktop Project state | Basis | Result |
| --- | --- | --- | --- | --- | --- |
| A | Monitor / app-server / CLI | ASSIGNED | direct evidence only | unique root | PASS |
| B | fixture | ASSIGNED | NOT OBSERVED | nested longest root | PASS |
| C | fixture | ASSIGNED | NOT OBSERVED | duplicate same key | PASS |
| D | fixture | AMBIGUOUS | NOT OBSERVED | distinct equal-longest keys | PASS |
| E | CLI / app-server | ASSIGNED | UNASSIGNED | one Workspace root, two Project roots | PASS |
| F | Desktop / Monitor project | independent | ASSIGNED | explicit assignment and alias | PASS |
| G | cross-surface | ORIGIN A, Turn B | independent | scoped cwd evidence | PASS |
| H | child | ASSIGNED / UNKNOWN | independent | direct / confirmed-parent fallback | PASS |
| I | CLI | ASSIGNED | UNASSIGNED | rollout cwd | PASS |
| J | Monitor / app-server | ASSIGNED | direct assignment only | thread/start.cwd | PASS |
| K | fixture | UNKNOWN / UNASSIGNED | NOT OBSERVED | missing-invalid / valid no-match | PASS |
| L | Desktop | unaffected | AMBIGUOUS / UNKNOWN | conflict / schema drift | PASS |

The sanitized machine-readable matrix is
[`docs/evidence/phase-3-2-5/gate-matrix.json`](evidence/phase-3-2-5/gate-matrix.json).

## Real observations

- One configured Monitor WorkspaceEntry uniquely matched `F:\AI\CodexMonitor`.
- Two Desktop Projects configured that exact root. The isolated CLI and direct
  app-server Threads remained Desktop Project UNASSIGNED because neither had a
  direct assignment.
- A Monitor project task had a direct legacy assignment and a confirmed
  app-server alias. Those records produced one canonical Project candidate.
- A historical direct legacy assignment remained ASSIGNED while its persisted
  SQLite Project ID was null.
- The current `state_5.sqlite` lacks `threads.project_id`. The reader emitted a
  `private-schema-drift` diagnostic while Workspace routing and Thread lifecycle
  continued normally.
- Phase 3.1 A2 retained one fullThreadId with an origin cwd under `C:` and a later
  Turn cwd under `F:\AI\CodexMonitor\.worktrees`; executable relation fixtures
  verified independent ASSIGNED ORIGIN and TURN_EXECUTION WorkspaceKeys.

No Phase 3.2 implementation defect was found, so no frozen production model or
runtime behavior was changed.

## Restart and reconstruction boundary

After the isolated app-server process ended, the app catalog read the same
fullThreadId in `notLoaded` state with the same cwd and existing Turn. The
runtime reconstruction fixture proved deterministic ORIGIN selection from
`thread/read` / `thread/list` cwd.

The current relation store is intentionally runtime-only. Historical
TURN_EXECUTION observations cannot be reconstructed from inputs that expose only
Thread cwd, so this is recorded as `NOT RECOVERABLE BY CURRENT CONTRACT`.
Reconstruction does not invent a historical Turn relation. A second standalone
process could not initialize the actual Codex-home SQLite runtime under the
restricted test account; that attempt is retained as `TEST LIMITATION`, not PASS.

## Read-only evidence

The Desktop metadata adapter probe was bracketed by SHA-256 hashes of Desktop
global state, both SQLite inputs, and the Monitor workspace store. All before
and after values were byte-identical. The isolated product task itself created
its normal explicit assignment before that bracket; the adapter made no write.

Evidence contains identifiers, locators, state, basis, and hashes only. User and
agent text, reasoning, credentials, cookies, and private file content are not
retained.

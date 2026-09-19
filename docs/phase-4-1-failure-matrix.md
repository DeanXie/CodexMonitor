# P4.1 Failure Matrix

Status: **PASS / COMPLETE / FROZEN** at `v0.7.68 · Build 8 · development`. It is checked by
`docs/fixtures/phase-4-1-closeout/contract.json` and
`npm run test:phase-4-1-closeout`.

| Case | Allowed result | Fail-closed boundary | Recovery | Evidence |
|---|---|---|---|---|
| Fresh profile | Explicit fresh activation | No normal load before activation | Prepare, commit, typed runtime validation | Production entry test |
| Valid activated profile | Current-process runtime validation required | No persisted-history readiness bypass or legacy fallback | Strict initialization, then ready | Production entry test |
| Legacy migration | Explicit preview and confirmed migration | No implicit or credential migration | Controlled P4.1c/P4.1d flow | Production entry test |
| Corrupt profile | Blocked corrupt | No normal load or automatic repair | Operator inspection | Deterministic fixture |
| Unknown/future schema | Blocked corrupt | No downgrade or best-effort load | Compatible application version | Deterministic fixture |
| Target conflict | Preserve and block | No overwrite or delete | Resolve ownership conflict | Deterministic fixture |
| Alias/junction/reparse path | Reject path | No alias following or cross-root write | Canonical non-reparse roots | Native temporary process |
| Staging tamper | Blocked inconsistent | No commit of changed staging | Explicit safe restage | Deterministic fixture |
| Preview/source change | Preview invalidated | No stale-preview commit | New preview | Deterministic fixture |
| Interruption before identity retirement | Continue before retirement | No completion claim | Recover with stop evidence | Deterministic fixture |
| Interruption after identity retirement | Continue same transaction | No rollback or new UUID | Protected recovery evidence | Native temporary process |
| Interruption after target commit | Classify a consistent lagging journal as recovery-required and converge only the journal | No runtime-valid claim, repeated migration, or repeated identity retirement | Revalidate bindings/material, converge to `target_committed`, then perform current-process runtime validation | Production entry test |
| Concurrent activation | One committer | No second successful commit | Re-inspect journal | Native temporary process |
| Duplicate daemon | Second start rejected | No second service owner | Stop existing daemon | Native temporary process |
| Missing token or pin | Confirmation required | No auto-connect or silent trust | Explicit credential/confirmation | Deterministic fixture |
| Legacy loader re-entry | Load rejected | No v2/retired reinterpretation as v1 | Activated-profile loader | Native temporary process |
| Runtime initialization failure | Remain target committed | No `runtime_validated` journal advance | Correct inputs and recover | Production entry test |
| Activated profile missing identity | Blocked corrupt | No implicit identity generation | Operator-guided recovery | Production entry test |
| daemonctl explicit data dir | Supported absolute root or failure | No ambient-root or cwd fallback | Valid absolute activated explicit root | Production entry test |
| Native non-READY entry gate | Recovery-only native surface | No normal IPC, menus, tray actions, or background business work | Complete or recover activation, restart, and validate current process | Production entry test |
| Restart-required activation boundary | Persist `target_committed` and request restart | No same-process READY after activation commit | Restart into strict current-process validation | Production entry test |
| Isolated child-process environment | Bounded temporary environment only | No real AppData, `CODEX_HOME`, credentials, or user services | Destroy temporary root and process | Native temporary process |

The JSON fixture is authoritative for exact case names and machine assertions;
this table is the human navigation view.

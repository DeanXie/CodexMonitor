# P4.1 Failure Matrix

Status: **FROZEN** by `docs/fixtures/phase-4-1-closeout/contract.json` and
`npm run test:phase-4-1-closeout`.

| Case | Allowed result | Fail-closed boundary | Recovery |
|---|---|---|---|
| Fresh profile | Explicit fresh activation | No normal load before activation | Prepare, commit, typed runtime validation |
| Valid activated profile | Current-process runtime validation required | No persisted-history readiness bypass or legacy fallback | Strict initialization, then ready |
| Legacy migration | Explicit preview and confirmed migration | No implicit or credential migration | Controlled P4.1c/P4.1d flow |
| Corrupt profile | Blocked corrupt | No normal load or automatic repair | Operator inspection |
| Unknown/future schema | Blocked corrupt | No downgrade or best-effort load | Compatible application version |
| Target conflict | Preserve and block | No overwrite or delete | Resolve ownership conflict |
| Alias/junction/reparse path | Reject path | No alias following or cross-root write | Canonical non-reparse roots |
| Staging tamper | Blocked inconsistent | No commit of changed staging | Explicit safe restage |
| Preview/source change | Preview invalidated | No stale-preview commit | New preview |
| Interruption before identity retirement | Continue before retirement | No completion claim | Recover with stop evidence |
| Interruption after identity retirement | Continue same transaction | No rollback or new UUID | Protected recovery evidence |
| Interruption after target commit | Validate committed target | No runtime-valid claim from file commit | Typed runtime validation |
| Concurrent activation | One committer | No second successful commit | Re-inspect journal |
| Duplicate daemon | Second start rejected | No second service owner | Stop existing daemon |
| Missing token or pin | Confirmation required | No auto-connect or silent trust | Explicit credential/confirmation |
| Legacy loader re-entry | Load rejected | No v2/retired reinterpretation as v1 | Activated-profile loader |
| Runtime initialization failure | Remain target committed | No `runtime_validated` journal advance | Correct inputs and recover |
| Activated profile missing identity | Blocked corrupt | No implicit identity generation | Operator-guided recovery |
| daemonctl explicit data dir | Exact root or failure | No ambient-root fallback | Valid activated explicit root |

The JSON fixture is authoritative for exact case names and machine assertions;
this table is the human navigation view.

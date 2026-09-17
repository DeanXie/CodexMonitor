# Phase 3.5.4 compatibility fixtures

These sanitized fixtures aggregate the already-frozen Phase 3.5.4a-d
contracts. `authority-contract.json` fixes the stable vocabularies, generation
envelope, recovery order, telemetry classification, zero-replay policy,
authority precedence, and remaining NOT PROVEN boundary.
`fixture-family-manifest.json` points to the existing a-d fixtures rather than
copying them. `current-actionable-approval.json` supplies the one closeout-only
positive approval UI scenario.

The fixtures contain no credentials, real Thread content, client identity,
ownership, lease, or mutation authorization. They do not establish a telemetry
database, persisted freshness, event replay, or daemon event-stream
completeness.

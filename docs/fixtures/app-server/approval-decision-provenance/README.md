# Approval decision provenance fixtures

These deterministic fixtures freeze the Phase 3.5.3b Remote approval-decision
contract against fake authority only. They contain no real approval request,
command, file modification, permission grant, credential, client identity,
owner, or lease.

The response fixtures mirror the bundled Codex 0.153.4 response schemas. A
successful app-server stdin write proves only `decision_dispatched`; it does
not prove which decision won or that an action was approved or applied.

`dispatch-boundaries.json` freezes the zero-retry distinction between loss
before the app-server write boundary and an unobserved caller outcome after
the write boundary. `generation-and-duplicate-boundaries.json` freezes exact
generation/identity matching and the single-admitted-attempt gate.

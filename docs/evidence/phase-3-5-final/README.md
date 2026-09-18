# Phase 3.5 Final Acceptance Evidence

This directory contains the sanitized, canonical closeout record for Phase 3.5.

- `final-acceptance.json` preserves the authenticated isolated read-only E2E,
  authoritative rehydration, zero forbidden real mutations, credential cleanup,
  and fresh regression counts. Its correction/provenance block explicitly
  classifies the stale-generation assertion as deterministic fixture evidence.
- The temporary authentication file was deleted after the run. Its contents,
  token, hash, and any private Thread contents are not evidence and are not
  stored here.
- The deterministic contract fixtures are under
  `docs/fixtures/phase-3-5-final-acceptance/`.
- The executable harness is `scripts/phase-3-5-final-acceptance.mjs`.

Evidence classes are not interchangeable:

- real daemon/Node integration proves authentication, daemon/Host validation,
  Workspace connection, exact list/read, generation-tagged observation,
  reconnect generation renewal, rehydration, and zero forbidden mutation;
- production-function regression proves the implemented stale-delivery gate;
- deterministic fixture/contract evidence supplies
  `staleOldGenerationRejected` in the final Node acceptance;
- a real delayed old-socket notification scenario and installed-app/manual UI
  acceptance were **NOT_EXECUTED** in this lane.

The following remain explicitly **NOT PROVEN**:

1. daemon broadcast/event-stream completeness;
2. exact upstream behavior for duplicate or late approval responses;
3. exact upstream ordering or winner semantics for concurrent duplicate deletes.

These limits are not rewritten as successful guarantees by the Phase 3.5
closeout.

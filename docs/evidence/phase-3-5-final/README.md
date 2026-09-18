# Phase 3.5 Final Acceptance Evidence

This directory contains the sanitized, canonical closeout record for Phase 3.5.

- `final-acceptance.json` records the authenticated isolated read-only E2E,
  reconnect/stale-generation rejection, authoritative rehydration, zero
  forbidden real mutations, credential cleanup, and fresh regression counts.
- The temporary authentication file was deleted after the run. Its contents,
  token, hash, and any private Thread contents are not evidence and are not
  stored here.
- The deterministic contract fixtures are under
  `docs/fixtures/phase-3-5-final-acceptance/`.
- The executable harness is `scripts/phase-3-5-final-acceptance.mjs`.

The following remain explicitly **NOT PROVEN**:

1. daemon broadcast/event-stream completeness;
2. exact upstream behavior for duplicate or late approval responses;
3. exact upstream ordering or winner semantics for concurrent duplicate deletes.

These limits are not rewritten as successful guarantees by the Phase 3.5
closeout.

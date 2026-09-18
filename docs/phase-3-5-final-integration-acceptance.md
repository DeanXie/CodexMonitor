# Phase 3.5 Final Integration Acceptance

Status: **PASS / COMPLETE / FROZEN**. Phase 4 is **IN PROGRESS**; its current
authority is `docs/phase-4-0-truth-release-boundary.md`.

## Acceptance boundary

Phase 3.5 final acceptance combines the frozen Phase 3.5.1-3.5.4 authorities
with one authenticated, isolated, non-destructive, read-only Remote E2E. The
real lane used a disposable ignored `CODEX_HOME`, daemon data directory,
workspace, loopback endpoint, and existing disposable exact Thread. It did not
operate on a user Thread or production project data.

The credential boundary is frozen as follows:

- credential preflight: **PASS**;
- authentication: **PASS**;
- temporary `auth.json` copy deleted after success or failure: **PASS**;
- secret contamination audit: **PASS**;
- credential contents, hashes, token material, and private Thread contents are
  never evidence.

## Real daemon/Node integration evidence

The authenticated lane proved:

- exact-ID list/read against the disposable Thread;
- generation-tagged event observation;
- reconnect creates a new Remote transport generation;
- authoritative projection hydration and rehydration return current coverage;
- forbidden real mutation counts remain zero for `thread/resume`, approval
  decision, `thread/delete`, upstream `thread/unsubscribe`, and force takeover.

The local synthetic live attach used to observe delivery is not an upstream
unsubscribe or writer mutation. No disposable Thread or Turn was created during
the authenticated run.

## Evidence classification correction

The original sanitized result is preserved. Its
`isolatedE2e.staleOldGenerationRejected` value was populated by
`loadStaleDeliveryFixture`, not by injecting a delayed notification from the old
socket into the real daemon/Node run. The corrected classification is:

- real daemon/Node integration: **PROVEN** for the items above;
- production-function stale-generation isolation regression: **PASS**;
- deterministic stale-delivery fixture/contract assertion: **PASS**;
- real transport late old-generation notification scenario: **NOT_EXECUTED**;
- installed-app/manual UI acceptance: **NOT_EXECUTED** for this final lane.

This provenance correction does not classify stale-generation isolation as a
product failure and does not reopen A3.

## Frozen authority

The aggregate fixtures preserve the identity and generation hierarchy,
zero-retry/replay mutation policy, writer/subscription/runtime observation
boundaries, exact approval/delete authority, projection freshness, stale-event
isolation, and honest UI availability mapping. Transport availability and UI
projection do not replace canonical Thread truth.

## Explicit limits

The following remain **NOT PROVEN**:

- daemon broadcast/event-stream completeness;
- exact upstream behavior for duplicate or late approval responses;
- exact upstream ordering or winner semantics for concurrent duplicate deletes.

No inference, waiver, or acceptance result converts those items into proven
behavior.

## Verification

The closeout gate requires the focused Node, frontend, and Rust acceptance
suites; Rust all-targets tests and check; Rust formatting; TypeScript checking;
the complete frontend suite; and `git diff --check`. Only the existing six
zh-CN/date-label frontend baseline failures are waived. No new waiver is
permitted.

Canonical machine-readable evidence is
`docs/evidence/phase-3-5-final/final-acceptance.json`.

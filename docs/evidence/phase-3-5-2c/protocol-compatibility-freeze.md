# Phase 3.5.2c.5 protocol compatibility freeze evidence

Status: **PASS / COMPLETE / FROZEN**.

## Fixture authority

The sanitized protocol fixtures are in
`docs/fixtures/app-server/thread-lifecycle-observation/`. They freeze existing
Phase 3.5.2c.1-c.4 behavior only; no fixture test sends a real upstream
unsubscribe or writer mutation.

The fixture set covers all six subscription states, all three runtime states,
synthetic local detach, upstream unsubscribe outcomes, delayed
`thread/closed`, connection and WorkspaceSession generation reset, reconnect,
and multi-subscriber connection isolation. Each scenario carries non-sensitive
scope, identity, timestamp, attempt/request evidence when applicable, response
or error evidence, and separate runtime evidence.

## RED evidence

Before the authoritative fixture directory existed, the new compatibility
suite compiled and ran 15 tests. All 15 failed at fixture loading with the
expected missing-path error. No production reducer or RPC behavior was changed
to obtain GREEN. Three additional review-hardening assertions were added after
that initial RED: production `thread/status/changed:notLoaded` ingestion, an
actual App-core/daemon-RPC parity check, and a fixture-driven ambiguous
post-dispatch path that proves one dispatch with no retry. They are not counted
in the original 15-test RED record.

## Frozen boundaries

- `thread_live_unsubscribe` is local synthetic detach only and dispatches zero
  upstream requests.
- `thread_upstream_unsubscribe` is the explicit real upstream mutation.
- Ambiguous post-dispatch outcomes record `unsubscribe_outcome_unknown` and
  are not retried.
- `thread/closed` and `notLoaded` are runtime evidence and do not rewrite an
  unsubscribe outcome.
- Connection and WorkspaceSession generations do not inherit current truth.
- Subscription/runtime events do not transition writer admission.
- Missing Workspace, unavailable WorkspaceSession, subscription
  `not_observed`, and runtime `unknown` remain distinct.

## Provenance

The fixture provenance pins bundled Windows x64 `codex-cli 0.153.4`, executable
SHA-256 `444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B`,
official source commit `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`, and the separately checked
upstream `main` reference `e9633d7a0226eac91c7a791dc4f92cf8f25df2ae` as of
2026-09-14.

## Fresh verification

The closeout verification recorded zero failures:

- protocol compatibility fixtures: 17 passed;
- actual App-core/daemon-RPC fixture parity: 1 passed, covering all three
  successful upstream unsubscribe outcomes with one dispatch and zero retry;
- subscription/runtime observation: 22 passed;
- upstream unsubscribe instrumentation: 20 passed;
- synthetic live detach: 14 passed;
- lifecycle reconciliation: 15 passed;
- writer non-transition coverage: 67 passed;
- Rust all-targets: 792 library tests passed with 4 ignored, 681 daemon tests
  passed with 3 ignored, 29 daemon-control tests passed, and 1 Tauri
  configuration test passed;
- `cargo check --all-targets`, `cargo fmt --all -- --check`,
  `npm run typecheck`, and `git diff --check` passed.

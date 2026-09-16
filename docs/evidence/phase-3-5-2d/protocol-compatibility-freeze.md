# Phase 3.5.2d Protocol Compatibility Freeze

Status: Phase 3.5.2d.1-d.5 and Phase 3.5.2d overall are
**PASS / COMPLETE / FROZEN**.

## Fixture authority

The sanitized compatibility fixtures are:

- `docs/fixtures/remote-transport-coordination/generation-hierarchy.json`
- `docs/fixtures/remote-transport-coordination/request-provenance.json`
- `docs/fixtures/remote-transport-coordination/stale-delivery.json`
- `docs/fixtures/remote-transport-coordination/daemon-restart.json`
- `docs/fixtures/remote-transport-coordination/protocol-provenance.json`

They freeze the generation hierarchy, compound transport request key,
writer-admission and upstream-unsubscribe attempt correlation, reconnect and
stale-delivery isolation, daemon-restart session reset, multi-client
shared-session behavior, availability/Thread-truth separation, and zero
retry/replay policy. They contain no credentials or product data.

## RED to GREEN

Before the fixtures existed, the shared compatibility suite compiled and ran
but failed `0 passed / 12 failed`; every failure named its missing fixture.
After adding the five fixtures, the same suite passed in both targets:

- App/lib target: `12 passed / 0 failed`
- daemon target: `12 passed / 0 failed`

The suite is `src-tauri/src/shared/remote_transport_compatibility_tests.rs` and
is included from the shared module in both targets. Existing d.1-d.4 behavior
tests remain the authority for runtime implementation.

## Frozen boundaries

```text
RemoteHostIdentity                 host identity
DaemonProcessGeneration           daemon process continuity
RemoteTransportGeneration         TCP transport continuity
WorkspaceSessionGeneration        WorkspaceSession continuity
AppServerConnectionGeneration     app-server connection continuity
```

The values are not interchangeable. A transport generation is not a stable
client identity, writer owner, subscription owner, or lease. Request evidence
is transport-scoped; writer/runtime truth is WorkspaceSession-scoped;
subscription truth is app-server-connection-scoped.

Stale response, notification, disconnect, EOF, and read-error evidence cannot
mutate current-generation state. Resume retry, upstream-unsubscribe retry,
reconnect replay, and daemon-restart replay remain zero. Availability cannot
delete a canonical Thread or create writer/subscription release evidence.

## Fresh verification

The closeout verification completed with zero failures:

- d.1 generation/provenance: `17 passed`
- d.2 dispatch correlation: `9 passed`
- d.3 stale delivery: `6 passed`
- d.4 daemon restart: `16 passed`
- d.4 daemon-process continuity: `3 passed`
- writer-admission non-regression: `67 passed`
- subscription/runtime non-regression: `22 passed`
- d.5 compatibility, App/lib target: `12 passed`
- d.5 compatibility, daemon target: `12 passed`
- Rust all-targets: `1604 passed`, `7 ignored`, `0 failed`
- `cargo check --all-targets`: pass
- `cargo fmt --all -- --check`: pass
- `npm run typecheck`: pass
- `git diff --check`: pass

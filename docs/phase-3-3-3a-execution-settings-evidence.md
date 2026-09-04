# Phase 3.3.3a — Execution Settings Evidence Model

Status: **PASS / FROZEN**. Phase 3.3.3 Forensics / Contract is **COMPLETE**. Phase 3.3.3b Execution Settings Evidence Ingestion is **PASS**. Phase 3.3.3c Focused Reconciliation / Effective Settings Acceptance is **GO / NOT STARTED**.

## Contract

Execution settings evidence is a pure shared-core model. It does not ingest Monitor requests, app-server responses or notifications, rollout records, or UI state in this slice.

Each observation is keyed by the canonical `CodexThreadKey` plus one scope:

```text
THREAD_DEFAULT
TURN_EXECUTION { fullTurnId }
```

The field key is separate from the Thread identity. Supported fields are model, effort, approvalPolicy, sandboxPolicy, networkAccess, writableRoots, cwd, and collaborationMode. Role is not part of this contract.

Each field retains three independent evidence layers:

```text
requested
serverEffective
persistedObserved
```

Requested evidence never implies server-effective state. Server-effective evidence never implies persisted observation. A later layer does not erase an earlier layer or its provenance.

## Assessment

The frozen assessment states are:

```text
UNKNOWN
REQUESTED_ONLY
EFFECTIVE_CONFIRMED
OBSERVED_CONFIRMED
MATCH
MISMATCH
CONFLICT
```

`OVERRIDDEN` is not an assessment state. It may be retained only as a `MISMATCH` reason when authoritative causal evidence establishes an override.

Within one comparable group, requested evidence is compared with persisted observation when available, otherwise with server-effective evidence. Equal canonical values produce `MATCH`; different values produce `MISMATCH`. Different server-effective and persisted-observed values produce `CONFLICT`. Evidence from different scopes or comparison groups is never compared.

## Comparison correlation boundary

`comparisonId` is evidence correlation identity. It is not Thread identity and is not Turn identity itself.

Future ingestion must derive a comparison group from an authoritative request, Turn, or settings correlation. It must not infer correlation from:

- time proximity;
- equal model or other setting values;
- equal cwd;
- equal prompt content;
- the most recent settings event.

For `TURN_EXECUTION`, the real full Turn ID is the preferred correlation evidence. A `thread/settings/updated` event containing only a Thread ID is a `THREAD_DEFAULT` / Thread settings snapshot; it must not be assigned to a Turn without authoritative Turn correlation.

The selector chooses comparison groups deterministically from recorded correlation and observation time. It never uses Vec insertion order. A later Thread-default snapshot cannot conflict with an older Turn observation because their scopes differ.

## History and provenance

Raw evidence is append-only. Exact duplicate records are idempotent. The same canonical value from a different source or observation time retains both raw records and both provenance entries.

Each provenance entry retains source, comparisonId, observedAt, confidence, and optional reason. The selected field view is distinct from raw history: canonical evidence value does not equal raw evidence record count.

## Frozen boundaries

Execution settings evidence does not change CreationIntent, Thread acknowledgement, FirstTurnIntent, `CodexThreadKey`, `WorkspaceKey`, or `ThreadWorkspaceRelation`. cwd can be connected to the frozen Workspace contract by later ingestion, but this model does not implement that connection.

No Monitor outgoing request capture, app-server ingestion, rollout ingestion, ThreadCodexParams change, eventNormalizer change, current/default fix, access-mode change, UI change, Desktop private-state write, or Phase 3.3.3b implementation is included.

## Verification

- Phase 3.3.3a focused: 20 passed.
- Phase 3.3.2 creation coordination: 26 passed.
- Phase 3.3.1 acknowledgement: 18 passed.
- Phase 3.2 workspace interoperability: 77 passed / 1 ignored.
- Phase 3.1 exact-ID: 5 passed.
- `cargo test --lib`: 449 passed / 3 ignored.
- `cargo check --all-targets`, `cargo fmt --check`, `npm run typecheck`, and `git diff --check`: passed.

Implementation commit: `bf2f930 feat: model execution settings evidence`.

Phase 3.3.3b now supplies the bounded authoritative ingestion paths while preserving this frozen model. Phase 3.3.3c is the next sole development starting point and remains not started.

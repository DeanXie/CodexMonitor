# E2E acceptance evidence

This directory stores durable, sanitized summaries for formal E2E gates. It
does not replace the rotating diagnostic journal and must not contain prompts,
reasoning, message content, account data, or raw diagnostic records.

Create a summary from a reviewed JSON candidate with:

```powershell
node scripts/e2e-evidence-summary.mjs candidate.json docs/evidence/phase-x-y-scenario.json
```

The writer accepts only the documented identity, hierarchy, source file,
producer classification, workspace assignment, source lane, observed model,
cumulative Token, lifecycle, freshness, lag, cursor, result, and sanitized notes
fields. Files are formatted as stable JSON and written through a temporary file
before rename. Preserve the 16 MiB diagnostic journal rotation independently.

The canonical Phase 2.5 Real E2E set is listed in
`phase-2-5-real-e2e-summary.md`. Gate B startup, catch-up, continued-tail, and
completion measurements are retained in the allow-listed `notes` field; they
must not include prompt, reasoning, message, account, or raw diagnostic content.

Do not reconstruct missing protocol evidence. If an earlier raw journal has
already rotated out, retain the human acceptance result in project status docs
and start durable summaries with the next gate that has verifiable identities.

The Phase 3.2.5 Project / Workspace interoperability evidence is stored in
`phase-3-2-5/`. Its dedicated contract fixture suite and evidence validator keep
`UNKNOWN`, `NOT OBSERVED`, `NOT TESTED`, and
`NOT RECOVERABLE BY CURRENT CONTRACT` explicit rather than inferring missing
runtime observations.

Phase 3.5.2b writer-admission compatibility evidence is stored as sanitized,
reviewable protocol fixtures in
`../fixtures/app-server/writer-admission-observation/`. The accepted and blocked
fixtures reference the frozen Phase 3.5.2a evidence, while the provenance file
pins the bundled Codex executable and checked upstream source revision. Rust
fixture tests drive the shared observation core and the real daemon read RPC;
they do not execute a new writer mutation.
The Phase 3.5.2b.5 RED/GREEN and regression index is
`phase-3-5-2b/protocol-compatibility-freeze.md`.

Phase 3.5.2c subscription/runtime lifecycle compatibility fixtures are stored
in `../fixtures/app-server/thread-lifecycle-observation/`. They freeze the
existing c.1-c.4 reducers and lifecycle reconciliation without executing a new
unsubscribe or writer mutation. The c.5 RED/GREEN and regression index is
`phase-3-5-2c/protocol-compatibility-freeze.md`.

Phase 3.5.2d Remote transport/request coordination compatibility fixtures are
stored in `../fixtures/remote-transport-coordination/`. They freeze generation
separation, transport-scoped request provenance, stale-delivery isolation,
daemon-restart session reset, multi-client shared-session semantics, and the
zero retry/replay contract without executing a writer or unsubscribe mutation.
The d.5 RED/GREEN and regression index is
`phase-3-5-2d/protocol-compatibility-freeze.md`.

Phase 3.5.3a approval-request observation fixtures are stored in
`../fixtures/app-server/approval-request-observation/`. They freeze the three
supported request families, exact resolution/completion correlation, upstream
auto-review annotation, generation isolation, and the no-decision/no-delete
boundary. The implementation evidence index is
`phase-3-5-3a/approval-observation-freeze.md`.

Phase 3.5.3b approval-decision fixtures are stored in
`../fixtures/app-server/approval-decision-provenance/`. They freeze typed
bundled response validation, exact current-generation identity admission,
transport-to-session attempt correlation, dispatch-boundary ambiguity, and
zero retry/replay using fake app-server and daemon transport authority only.
The implementation evidence index is
`phase-3-5-3b/approval-decision-provenance-freeze.md`.

Phase 3.5.3c delete-authority fixtures are stored in
`../fixtures/app-server/delete-mutation-observation/`. They freeze exact-ID
admission, current-generation confirmation, active-writer rejection,
post-dispatch unknown outcome, session-end ambiguity, zero retry/replay, and
the confirmed-only tombstone gate using fake app-server authority. The
implementation evidence index is
`phase-3-5-3c/delete-authority-freeze.md`.

Phase 3.5.3d delete-isolation fixtures are stored in
`../fixtures/app-server/delete-mutation-isolation/`. They freeze same-key local
single dispatch, different-key concurrency, generation/session isolation,
pre/post-write transport loss, direct-evidence precedence, and zero replay using
fake authority only. The implementation evidence index is
`phase-3-5-3d/delete-isolation-freeze.md`.

Phase 3.5.3e compatibility fixtures are stored in
`../fixtures/app-server/phase-3-5-3-compatibility/`. They freeze the aggregate
approval/delete schema, generation hierarchy, multi-client scoping,
unknown/stale/direct-evidence precedence, zero retry/replay, and confirmed-only
tombstone contract without executing a real mutation. The closeout evidence
index is `phase-3-5-3e/compatibility-closeout.md`.

Phase 3.5.4a projection-freshness compatibility fixtures are stored in
`../fixtures/projection-freshness/`. They freeze status, coverage, generation,
source, partial-hydration, invalidation, and forbidden-semantics contracts
without performing recovery or mutation. The implementation evidence index is
`phase-3-5-4a/projection-freshness-authority.md`.

Phase 3.5.4b generation-tagged event fixtures are stored in
`../fixtures/generation-tagged-events/`. They freeze local/Remote envelopes,
current and stale generation delivery, missing-generation failure, hydration
boundaries, same-payload provenance, and multi-client transport binding without
adding mutation replay. The implementation evidence index is
`phase-3-5-4b/generation-tagged-event-delivery.md`.

Phase 3.5.4c authoritative recovery fixtures are stored in
`../../src-tauri/tests/fixtures/phase-3-5-4c-authoritative-hydration/`. They
freeze reload, reconnect, WorkspaceSession replacement, daemon restart,
selected-thread and observation hydration, stale-result isolation, partial
failure, event interleaving, and multi-client convergence without replaying a
mutation. The implementation evidence index is
`phase-3-5-4c/authoritative-recovery-hydration.md`.

Phase 3.5.4d offline/stale UI fixtures are stored in
`../../src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/`. They freeze
coverage-specific display states, layered availability, ephemeral gap evidence,
duplicate/out-of-order handling, reconnect/restart cache behavior,
multi-client isolation/convergence, and approval/delete stale safety. The
implementation evidence index is `phase-3-5-4d/offline-stale-ui.md`.

Phase 3.5.4e aggregate compatibility fixtures are stored in
`../fixtures/phase-3-5-4-compatibility/`. They freeze the complete
ProjectionFreshness, generation-event, authoritative recovery, offline/stale
UI, multi-client, approval/delete safety, telemetry, no-persistence,
authority-precedence, and zero mutation replay contract. The closeout evidence
index is `phase-3-5-4e/compatibility-closeout.md`.

Phase 3.5 final integration acceptance fixtures are stored in
`../fixtures/phase-3-5-final-acceptance/`. They aggregate the frozen Phase
3.5.1-3.5.4 authority contracts. The sanitized authenticated isolated
read-only E2E and closeout record is `phase-3-5-final/final-acceptance.json`;
its scope and explicit NOT PROVEN boundaries are documented in
`phase-3-5-final/README.md`. Its stale-old-generation boolean is explicitly
classified as deterministic fixture/contract evidence, with production-function
regression coverage PASS and the real delayed old-socket scenario NOT_EXECUTED.

P4.1b version/identity contract fixtures are stored in
`../fixtures/phase-4-1b-release-version-authority/`. They cover canonical
version projection, deterministic drift repair, monotonic Build bumps, target
DeanX identity, unchanged legacy runtime identity, and stable inactive Windows
installer identity. The authority and closeout record is
`../phase-4-1b-release-version-authority.md`.

P4.1c whitelist-migration fixtures are stored in
`../fixtures/phase-4-1c-whitelist-migration/`. They freeze explicit-root,
source-read-only discovery, field allowlisting, credential and canonical-data
exclusion, workspace identity preservation, deferred RemoteHostIdentity,
staging ownership, validation, interruption recovery, and rollback without
activation. The evidence index is `phase-4-1c/README.md`.

P4.1d-1 activation-safety fixtures are stored in
`../fixtures/phase-4-1d-1-migration-activation-foundation/`. They freeze the
inactive bootstrap classifier, transaction/root binding, v1/v2 identity
compatibility target, Windows protected replacement, interruption recovery,
and local service-lifetime mutex limits. The evidence index is
`phase-4-1d-1/README.md`.

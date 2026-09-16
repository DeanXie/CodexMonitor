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

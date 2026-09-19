# P4.1 — Release Identity / Version / Migration / Update Safety Closeout

Status: **PASS / COMPLETE / FROZEN**. P4.2 is not started.

P4.1 freezes the first Windows daily-use release safety boundary without
performing a real user cutover. Its machine-readable authority is
`docs/fixtures/phase-4-1-closeout/contract.json`; the required failure behavior
is summarized in `docs/phase-4-1-failure-matrix.md`.

## Frozen authority

- `VERSION.json` is the sole software version authority at
  `v0.7.68 · Build 5 · development`. Build and configuration schema remain
  independent.
- Desktop product identity is `CodexMonitor DeanX` /
  `io.github.deanxie.codexmonitor`; iOS retains the legacy identifier pending a
  separately authorized platform migration.
- Updater artifacts, updater runtime surfaces, and third-party Sentry remain
  disabled.
- Migration is explicit-root, source-read-only, staging-only, and allowlist
  based. Credentials, `CODEX_HOME`, canonical Thread/rollout data, unknown
  required fields, and runtime mutation state never migrate.
- Missing Remote credentials or trusted-host confirmation remains untrusted and
  cannot auto-connect.
- App, daemon, and daemonctl share the activated-profile gate and fail closed on
  corrupt, conflicting, recovery-required, or unsupported profiles.

## Activation boundary

Activation separates filesystem commitment, persisted runtime history, and the
current process's business readiness:

```text
prepared → target_committed → runtime_validated (persisted history)
                            ↘ current process: blocked → validating → ready|failed
```

`target_committed` proves only the statically validated, transaction-bound target.
It does not make an App or daemon process ready. Each process must perform its
own strict initialization, including typed settings/workspace loading and the
required identity, root, transaction, listener, and service checks. Only a
successful current-process validation may record `runtime_validated` history and
publish that process as `ready`. A historical `runtime_validated` journal never
bypasses validation in a new process. Normal business IPC/RPC remains closed
until the current process is ready.

## Evidence boundary

All migration and activation verification uses synthetic roots, fake identities,
fake settings, and controlled test processes. The aggregate compatibility suite
freezes P4.1a-d3 contracts and the complete failure-case inventory. Fresh closeout
results are recorded in `docs/evidence/phase-4-1e/README.md`.

Real user settings reads, workspace migration, HostIdentity reads or retirement,
daemon stop, installation, and profile cutover are **NOT_EXECUTED**. Installed
package acceptance is **NOT_EXECUTED**. Sudden power-loss durability is
**NOT_PROVEN**. Those limits are not implied by unit, fixture, source-build, or
temporary-root evidence.

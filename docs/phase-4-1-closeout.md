# P4.1 — Release Identity / Version / Migration / Update Safety Closeout

Status: **CLOSEOUT IN PROGRESS**. P4.1e is **PAUSED / RESUME PENDING** and
P4.2 is not started.

This document is the P4.1 closeout candidate for the first Windows daily-use
release safety boundary; it is not a final P4.1 completion record. P4.1d-4a is
**PASS / COMPLETE / FROZEN** and resolves compliance items R02, R05, and R07.
R01, R03, R04, R06, and R11 remain open before P4.1 can close. Its
machine-readable authority is
`docs/fixtures/phase-4-1-closeout/contract.json`; the required failure behavior
is summarized in `docs/phase-4-1-failure-matrix.md`.

## Frozen authority

- `VERSION.json` is the sole software version authority at
  `v0.7.68 · Build 6 · development`. Build and configuration schema remain
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
freezes the accepted slice contracts and the current failure-case inventory.
The Build 5 P4.1e run in `docs/evidence/phase-4-1e/README.md` is historical
closeout-attempt evidence, not proof that the remaining compliance items are
closed. P4.1d-4a evidence is recorded separately under
`docs/evidence/phase-4-1d-4a/`.

Real user settings reads, workspace migration, HostIdentity reads or retirement,
daemon stop, installation, and profile cutover are **NOT_EXECUTED**. Installed
package acceptance is **NOT_EXECUTED**. Sudden power-loss durability is
**NOT_PROVEN**. Those limits are not implied by unit, fixture, source-build, or
temporary-root evidence.

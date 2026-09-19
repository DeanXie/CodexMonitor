# P4.1e Closeout Evidence

Status: **PASS / COMPLETE / FROZEN** at `v0.7.68 · Build 8 · development`.

This directory preserves the Build 5 aggregate closeout attempt as historical
evidence and records the fresh Build 8 aggregate closeout separately. P4.1d-4a
resolved R02, R05, and R07 at Build 6; P4.1d-4b resolved R01 and R03 at Build 7
for its supported Windows scope; P4.1d-4c resolved R04, R06, and R11 at Build 8.
P4.1d, P4.1e, and P4.1 are **PASS / COMPLETE / FROZEN**. P4.2 is
**NOT_STARTED**.

## Authorities

- Aggregate closeout candidate: `docs/fixtures/phase-4-1-closeout/contract.json`
- Current closeout candidate: `docs/phase-4-1-closeout.md`
- Failure matrix: `docs/phase-4-1-failure-matrix.md`
- Runtime boundary: target commit remains `target_committed`; only successful
  current-process initialization can record `runtime_validated` history and open
  that process's business gate. Historical runtime success cannot ready a new
  process.

## Historical Build 5 verification

- Original failing Rust regression: 1 passed, 0 failed.
- Shared Rust library: 1,133 passed, 0 failed, 4 ignored.
- Production-entry runtime handshake: 6 passed, 0 failed.
- Aggregate P4.1 compatibility: 8 passed, 0 failed.
- Rust check and Rust formatting: PASS.
- TypeScript typecheck and production frontend build: PASS.
- Frontend full suite: 1,235 passed; exactly the six previously frozen
  locale/date baseline assertions failed, with no new waiver.
- Version authority: `v0.7.68 · Build 5 · development`, PASS.
- Legacy target remained exactly 5,373,664,261 bytes / 5,061 files / latest
  write `2026-09-19T03:28:19.6058902Z` across governed builds. The managed
  external target alone advanced to 8,072,461,406 bytes / 8,806 files / latest
  write `2026-09-19T11:08:41.5690517Z`.
- Pre-commit storage report: phase managed target `ACTIVE` at 7.52 GiB; phase
  legacy target `LEGACY-IN-TREE` at 5.00 GiB; Agent targets 11.19 GiB; free
  space 418.03 GiB; scan 9,856 ms. No target was deleted.

## Explicit limits

- Real user cutover: **NOT_EXECUTED**
- Installed-package acceptance: **NOT_EXECUTED**
- Sudden power-loss durability: **NOT_PROVEN**
- P4.2: **NOT_STARTED**

## Fresh Build 8 closeout

The final closeout reran the machine-readable compatibility contract, focused
R01–R12 production-entry and deterministic fixture coverage, the complete Rust
and frontend suites, formatting/type/version checks, and the production Windows
bundle build. No Build 5 result was reused as fresh evidence.

- Aggregate P4.1 compatibility: 11 passed, 0 failed.
- Focused Rust coverage: startup activation 9 passed; controlled legacy
  migration entry 8 passed; activation foundation 23 passed; legacy process
  stop 3 passed and 1 helper ignored; runtime-validation production entry 11
  passed. No focused test failed.
- Complete Rust all-targets: 1,092 passed, 0 failed, 4 ignored.
- Rust check and Rust formatting: PASS.
- Frontend bootstrap-boundary focus: 4 passed, 0 failed.
- Frontend full suite: 1,237 passed; exactly the six previously frozen
  locale/date baseline assertions failed, with no new waiver.
- TypeScript typecheck: PASS.
- Version authority: `v0.7.68 · Build 8 · development`, PASS.
- Production Windows source build: PASS. It produced the MSI and NSIS bundles
  under the governed external Cargo target. Neither bundle was installed or
  launched.

The final failure matrix contains 22 cases. Each case records its allowed and
forbidden actions, resulting state, recovery path, and evidence classification.
The added Build 8 boundaries are the native non-READY entry gate, the
restart-required activation boundary, and the isolated child-process
environment.

## Limits retained after closeout

- Real user cutover: **NOT_EXECUTED**
- Installed-package acceptance: **NOT_EXECUTED**
- macOS installed acceptance: **NOT_EXECUTED**
- iOS migration/build/device acceptance: **NOT_EXECUTED**
- Sudden power-loss durability: **NOT_PROVEN**

Source-build MSI/NSIS generation is not installation or installed-app launch
acceptance. No real user profile, credential, HostIdentity, service, or
application instance is touched by this closeout.

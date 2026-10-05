# D First-Launch Production Correction

Source correction verification: **PASS** at `v0.7.68 · Build 10 · development`.
This is not installed acceptance. P4.2c remains **IN PROGRESS / BLOCKED pending
a corrected artifact and fresh acceptance**. Build 9 Case D remains **FAIL**;
Case C remains **BLOCKED / EVIDENCE INCOMPLETE**. No dependent case resumed.

## Proven defect and minimal correction

At source authority `d1718b38eaa479495ea280f2fa9e35124b2f3cab`, the installed
Build 9 application stayed responsive but displayed the pre-READY IPC rejection
twice without usable setup controls. `BootstrapBoundary` first calls the real
`isMobileRuntime` IPC wrapper; only success permits `get_bootstrap_status`.
The desktop bootstrap allowlist omitted `is_mobile_runtime`, so the platform
probe was rejected and status remained null. The missing fresh activation UI
was reproduced without installing or starting the application again.

The production change adds only `is_mobile_runtime` to that allowlist in
`src-tauri/src/lib.rs`. It reads a compile-time platform flag and does not
initialize business state. No frontend production code or daemon code changes.

Normal business IPC stays denied before current-process READY. Fresh activation
still commits only `TARGET_COMMITTED` and requires a restart. Historical
`runtime_validated` cannot ready a new process; activation and daemon lifecycle
remain unchanged. Daemon absence before activation was expected, not evidence
of a discovery defect. Missing optional tools were not the cause of this defect.

## Verification evidence and coverage limits

- Production Rust gate RED: 3 passed, 1 failed; the new platform prerequisite
  assertion failed on `business_invoke_allowed("is_mobile_runtime", false)`.
- Same Rust gate GREEN: 4 passed, 0 failed. Registered business reads, workspace
  connection, resume, approval, delete, upstream unsubscribe, and daemon start
  remain denied before READY.
- Frontend focused: 81 passed, 0 failed (existing bootstrap 4, new native-boundary
  contract 3, production IPC wrappers 74).
- The new frontend tests keep the component and service wrappers real and fake
  only the native boundary. They cover safe platform probe followed by fresh
  status, explicit fresh intent, restart-required behavior without business UI,
  and platform rejection without a bypass. They are not installed WebView/Tauri
  E2E or a second execution of the native Rust gate; Rust verifies that gate.
- Rust all-targets: 2,257 passed, 0 failed, 9 existing ignored helper/tests.
  Production-entry runtime-validation tests: 11 passed within that total.
- Version/P4.0/P4.1/P4.2 Node contracts: 51 passed, 0 failed. Build increment
  initially exposed two Build 9-only assertions; the canonical assertion now
  expects Build 10 and the packaging check permits monotonic source revisions
  after the frozen Build 9 package. Historical artifact fixtures are unchanged.
  The current-state document guard also now requires P4.2c IN PROGRESS / BLOCKED
  rather than the obsolete NOT STARTED claim; P4.1's frozen history is preserved.
- Frontend full: 1,240 passed, 6 failed, 0 skipped. Exactly the existing frozen
  zh-CN/date expectations remain: five `useTraySessionUsage` tests and one `Home`
  test. This is not an unqualified full-suite green result; no waiver was added.
- Typecheck, Rust all-targets check, Rust formatting, version projections, and
  `git diff --check`: PASS.

The six retained frontend failures are:

1. `useTraySessionUsage builds the current session usage summary from workspace rate limits`
2. `useTraySessionUsage supports remaining mode to match the sidebar setting`
3. `useTraySessionUsage includes weekly usage when the workspace has a secondary window`
4. `useTraySessionUsage syncs only when the derived usage changes`
5. `useTraySessionUsage retries the same usage payload after a transient sync failure`
6. `Home renders expanded token stats and account limits`

Non-sensitive full frontend JSON and Rust logs are retained in
`F:\AI\CodexMonitor-Acceptance\p4-2c-first-launch-correction-build-10-20261005`.
Retained evidence SHA-256:

- `frontend-full.json`: `4FE1F88739875B235F53488F799B3F4F766159F2D4FE97CDFCA0D97AFCB64350`
- `rust-gate-green.log`: `09305A5F17985A01F3B288C5DFB391D06FB9D7F1AD9FCA5020E860161C989919`
- `rust-all-targets.log`: `41854DDF917606530844D1B69326AF141B2B7F9A1028003FF11F18BF2C0B9ABF`

Frozen Guest D evidence remains in
`F:\AI\CodexMonitor-Acceptance\p4-2c-manual-setup-20261005-01\EvidenceOutput`;
its eight-file Case D manifest was revalidated with zero mismatches.

## Version, artifact, and next authorization boundary

`VERSION.json` was bumped once using `version:bump -- build` followed by
`version:sync`. SemVer remains `0.7.68`; status remains `development`. The Apple
build projection advances to 10 without claiming iOS build/device acceptance.

Retained Build 9 failed-D-acceptance NSIS:

- Path: `F:\AI\CodexMonitor-Artifacts\build-9-reconstituted-20261003\CodexMonitor DeanX_0.7.68_x64-setup.exe`
- Size: 227,650,080 bytes
- SHA-256: `461F776068DECF620010CC3C325674B0A43F69ACD16427BFB84657942822BC1F`

It remains historical evidence and was not overwritten, deleted, or relabeled.
The earlier `F9A4B935...2304B81` artifact remains historical/lost/unrecoverable,
not the current failed-acceptance artifact. No Build 10 installer was produced.

After source closeout, a separately authorized Build 10 packaging slice must
freeze a new artifact identity. A subsequent separately authorized fresh
installed acceptance may rerun the affected path. Source correction cannot
rewrite Build 9 D FAIL to PASS or close C's non-admin evidence gap.

## Safety

This correction used an independent worktree, existing dependencies with an
identical lockfile, fake native boundaries, temporary profiles, and controlled
test processes. Cargo storage wrappers used the unique external target; no new
in-tree Cargo target was created. No Sandbox, installer, installed application,
real credential, real profile migration, or real HostIdentity operation was
performed. No real Thread mutation was sent. The four protected main-workspace
files remain outside the correction diff.

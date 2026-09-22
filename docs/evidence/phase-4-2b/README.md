# P4.2b Evidence

Status: **PASS / COMPLETE / FROZEN** at
`v0.7.68 · Build 9 · development`.

Authority:

- `../../phase-4-2b-windows-packaging-corrections.md`;
- `../../fixtures/phase-4-2b-windows-packaging-corrections/contract.json`;
- `../../../scripts/phase-4-2b-windows-packaging-corrections.test.mjs`;
- `../../../src-tauri/windows/nsis/installer.nsi`;
- dictation resolver tests in `../../../src-tauri/src/dictation/mod.rs`.

The accepted static evidence is canonical NSIS per-user packaging, embedded
offline WebView2, executable-relative daemon payload, no cwd dictation fallback,
SemVer installer identity, Build 9 repository revision, and unchanged
updater/Sentry disablement. MSI is retained as a non-canonical artifact.

No installer or installed binary was executed. P4.2c installed acceptance is
not implied by this slice.

## RED to GREEN and fresh verification

The initial P4.2b contract run had one pass and three real failures: no explicit
offline WebView2 mode, no P4.2b fixture, and Build 8 instead of Build 9. The
dictation regression failed to compile because the fail-closed resolver did
not exist. A second strict RED proved that no audited custom NSIS template was
pinned. After the minimal corrections:

- P4.1/P4.2/version Node contract tests: 48 passed, 0 failed;
- dictation focused Rust tests: 2 passed, 0 failed;
- Tauri config Rust tests: 3 passed, 0 failed;
- Rust all-targets: 2,256 passed, 0 failed, 9 ignored;
- frontend: 1,237 passed; the same six frozen zh-CN/date assertions failed and
  no waiver was added;
- typecheck, Rust check, Rust fmt check, version check, production Windows
  build, and diff check: passed.

The generated NSIS source has zero online bootstrapper references and contains
the offline WebView2 payload plus `codex_monitor_daemon.exe`.

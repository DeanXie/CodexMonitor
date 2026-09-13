# Phase 3.5.2a — Exact-ID Remote Read / Admission Separation

Status: **PASS / COMPLETE / FROZEN**. Gates A1, A2, and A3 are **PASS**. Phase 3.5.2b.1 shared observation model is **PASS / FROZEN**; Phase 3.5.2b.2 runtime wiring is **NOT STARTED**.

## Frozen authority contract

- `thread/read` is reader-only observation. Selection, focus, polling, and ordinary refresh use this path and never implicitly resume a Thread.
- `thread/resume` is the explicit writer-admission boundary. It validates the exact returned full Thread ID, never falls back to `thread/start`, and does not create a Turn.
- `turn/start` is a separate explicit mutation.
- An active upstream writer produces typed `BLOCKED_BY_ACTIVE_WRITER`; it is not `NotFound`, Thread absence, or permission to create a replacement Thread.
- `thread not loaded` is `NOT_LOADED / UNAVAILABLE`. It is not `NotFound` or authoritative Thread absence, and it must not trigger an automatic resume.
- Writer authority remains in the upstream Codex app-server/core. CodexMonitor does not introduce force takeover or a second writer authority.

## Acceptance gates

| Gate | Evidence | Result |
| --- | --- | --- |
| A1 | Remote-created Thread `01a08c97-20ce-71c1-82ae-5a0f57413801` accepted an exact `thread/read`; the returned full ID matched, Turn count remained `1 -> 1`, no resume was dispatched, and no Thread or Turn was created. | PASS |
| A2 | While Desktop owned Thread `01a08c05-7880-75a2-976c-2a5895b58723`, Remote exact `thread/resume` returned app-server error `-32600 already has an active writer`, classified as `BLOCKED_BY_ACTIVE_WRITER`. No fallback or Turn was created. | PASS |
| A3 | After Desktop writer release, one exact Remote `thread/resume` succeeded for `01a08c05-7880-75a2-976c-2a5895b58723`. The returned full ID matched; canonical identity, rollout, and Turn count remained stable. | PASS |

## Gate A3 canonical evidence

The formal invocation executed exact `resume_thread` once and returned the requested full Thread ID. It invoked no `thread/start`, `turn/start`, or `connect_workspace`, used no RPC retry, and requested no force takeover. No retest resume was performed.

The target-scoped canonical gate is authoritative; global totals are observational telemetry only:

```text
CodexThreadKey =
  (codex-home:76c92e1eaaa451ef02c92f75f12738ea3337e69b890333a72cd14ec778cac75f,
   01a08c05-7880-75a2-976c-2a5895b58723)

target rows: 1 -> 1
target Turn count: 1 -> 1
target rollout: unchanged
canonical duplicate: 0 -> 0
```

The original post-resume snapshot attempt encountered a sharing violation after the resume had already succeeded. The writer-compatible read discriminator proved the acceptance reader's sharing-mode incompatibility: `File.ReadLines(path, UTF8)` failed while a UTF-8 `StreamReader` over `FileStream(Open, Read, FileShare.ReadWrite | FileShare.Delete)` succeeded. The canonical reader now uses the explicit shared UTF-8 stream contract. A later read-only snapshot completed without another resume and established the target invariants above.

Global canonical growth is permitted when it is unrelated to the target, introduces no duplicate, and preserves the target key, row, rollout, and Turn invariants. Historical absolute totals are not acceptance gates.

## Evidence authority

The sanitized committed manifest is `docs/evidence/phase-3-5-2a/a3-acceptance-summary.json`. Raw runtime evidence is retained outside Git under:

```text
src-tauri/target/phase-3-5-2a-a3/history/
  resume-success-20260913T200545-runner-9152/
```

The raw archive contains the original `result.json`, `runner-process.json`, stdout/stderr, armed state, and GO signal. Its manifest records SHA-256 values, canonical-before and canonical-after evidence, the sharing-mode RED/GREEN result, and the final runner SHA-256. It contains no authentication token.

The archived and committed copies of `a3-acceptance-summary.json` both have SHA-256 `8A464701873F4B1B05E33C8D5B7E672CFEDBCB91DE172DED23EE51EFCE0C0C1D`.

## Closeout verification

- Windows PowerShell 5.1 runner/full-lifecycle: 160 passed, 0 failed.
- PowerShell 7 runner/full-lifecycle: 160 passed, 0 failed.
- Windows PowerShell 5.1 supervisor lifecycle: 38 passed, 0 failed.
- PowerShell 7 supervisor lifecycle: 38 passed, 0 failed.
- UTF-8 no-BOM and concurrent shared-writer canonical snapshot regressions pass in both PowerShell runtimes.
- Phase-scoped frontend tests: 125 passed, 0 failed.
- Rust library tests: 631 passed, 0 failed, 4 ignored.
- TypeScript typecheck, Rust all-target check, Rust formatting check, and Git whitespace check pass.

The repository-wide frontend suite retains six locale-sensitive date-label failures that reproduce unchanged on the baseline branch. They are not in the Phase 3.5.2a files; Phase-scoped frontend regression remains green.

## Resolved acceptance-harness findings

The AppData dual-context visibility boundary, host-visible settings boundary, PowerShell closure-resolution defects, Windows PowerShell supervisor exit-code telemetry, UTF-8 no-BOM decoding, historical absolute-total gate, and rollout sharing-mode incompatibility are resolved or frozen harness findings. None is a current Phase 3.5.2a blocker.

Current blocker for Phase 3.5.2a: **NONE**.

# Phase 3.5.3a approval observation evidence

Status: **PASS / COMPLETE / FROZEN**.

The shared approval model owns a current-generation pending registry plus
historical terminal evidence. App and daemon sessions instantiate the same
runtime and ingest upstream messages before event projection. The frontend
actionable queue removes only the exact workspace/request/Thread identity from
`serverRequest/resolved`.

RED was observed in both runtime layers: the Rust focused suite failed to
compile because the shared approval-observation module did not exist, and the
frontend event test observed zero resolved callbacks where one exact callback
was required.

Fresh GREEN evidence on 2026-09-17:

- approval-observation Rust tests: 15 passed, 0 failed;
- focused frontend event/reducer tests: 41 passed, 0 failed;
- Rust all-targets suites: 850 + 754 + 29 + 1 passed, 0 failed, 7 ignored;
- `cargo check --all-targets`, `cargo fmt --all -- --check`,
  `npm run typecheck`, and `git diff --check`: exit 0;
- full frontend suite: 1127 passed with the six pre-existing zh-CN locale
  baseline failures unchanged and outside this slice.

No approval decision mutation, retry, Thread deletion, Remote-client ownership,
approver ownership, or lease field is part of this slice. Phase 3.5.3b remains
not started.

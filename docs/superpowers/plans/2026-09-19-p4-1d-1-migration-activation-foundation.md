# P4.1d-1 Implementation Plan

1. Add RED tests for independent source schema detection, fixed projections,
   path aliases/reparse points, pure bootstrap classification, and stale
   preparation detection.
2. Add RED tests for activation journal/file-state recovery, recovery-material
   cleanup protection, fresh activation ordering, and concurrent commit locks.
3. Freeze the baseline v1 identity loader as a test fixture; add v2 active and
   retired candidate formats plus real Windows child-process/`ReplaceFileW`
   tests.
4. Add a new-service lifetime lock and prove cross-process exclusion for one
   controlled profile without claiming legacy or distributed exclusion.
5. Keep all new production modules unregistered from App, daemon, and
   daemonctl startup. Verify active identifiers and data roots are unchanged.
6. Run focused tests, P4.1a/b/c regression, Rust all-targets/check/fmt,
   frontend typecheck/build, version check, and diff check.
7. Bump the canonical Build exactly once from 2 to 3, explicitly stage only
   P4.1d-1 files, commit, ff-only merge, push, and verify refs.

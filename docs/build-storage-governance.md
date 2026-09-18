# Build Storage Governance

CodexMonitor separates ephemeral Worktree Cargo output from source trees and bounds its lifetime. External placement alone is not cleanup: the supported lifecycle combines per-Worktree isolation, disabled incremental compilation, build-time disk guards, explicit closeout, and conservative orphan cleanup.

## Target classes

- `MAIN`: `src-tauri/target` in the main checkout. Existing incremental behavior is unchanged.
- `RELEASE`: release output below the main target.
- `ACTIVE`: a registered linked Worktree with a managed external target.
- `CLOSEOUT-ELIGIBLE`: a clean registered Worktree whose HEAD is merged into `main`; deletion still requires explicit acceptance.
- `ORPHAN`: a managed target whose Worktree is no longer registered. It becomes cleanup-eligible only after the configured TTL.
- `LEGACY-IN-TREE`: `src-tauri/target` found inside a linked Worktree, normally produced by unsupported bare Cargo commands.

Managed targets are unique direct children of `<build-root>/agents/`. Each contains `.codexmonitor-target.json`, which binds it to a repository and Worktree. On Windows the default build root is `_cargo-target/CodexMonitor` beside the main checkout. `CODEXMONITOR_CARGO_TARGET_ROOT` provides a process-local override for controlled tests or alternate storage.

## Supported commands

```bash
npm run storage:report
npm run storage:prepare
npm run rust:build
npm run rust:check
npm run rust:test
npm run tauri:dev
npm run tauri:build
npm run storage:closeout -- --accepted
npm run storage:closeout -- --apply --accepted
npm run storage:clean-agents
npm run storage:clean-agents -- --apply
```

In a linked Worktree, the Rust and Tauri wrappers inject a unique `CARGO_TARGET_DIR` and `CARGO_INCREMENTAL=0`. The same commands in the main checkout retain `src-tauri/target` and Cargo's normal incremental setting.

Bare `cargo build`, `cargo check`, and `cargo test` are prohibited for ephemeral Agent Worktrees. Cargo configuration discovery depends on the invocation directory, so the project deliberately uses command-local environment injection instead of global Cargo configuration. `storage:report` detects resulting in-tree targets as `LEGACY-IN-TREE`.

## Disk guard

Thresholds live in `config/storage-governance.json`. The initial policy warns above 80 GiB total managed Agent output or 25 GiB for one target, and blocks new heavy ephemeral builds above 120 GiB total, above 40 GiB for one target, or below 50 GiB free space. A deliberate `--allow-low-space` override records the current measurements, thresholds, target root, and override in command output. Main interactive development is not blocked by the ephemeral guard.

## Cleanup safety

`storage:closeout` is dry-run by default. Apply requires all of the following: a linked Worktree, a clean Git state, `--accepted`, a schema-valid managed target, canonical containment directly below the allowlisted `agents` root, and no symlink/junction anywhere in the candidate. It deletes only the external target; it never deletes the Worktree, source, branch, or main/release output.

Heavy wrappers and cleanup atomically contend for the same operation lock outside the target directory. The winner holds it for the full build or validated deletion, so cleanup cannot pass a check and then race a newly started Cargo or Tauri process. A lock whose owner process is dead becomes recoverable only after `operationLockStaleMinutes` (default 5); a live owner is never reclaimed by age alone. An accepted or closed Agent phase must run the dry-run closeout followed by the explicit apply command as its formal lifecycle completion.

Orphans are marked immediately but are not cleanup-eligible until `orphanMinimumAgeDays` (default 7). `storage:clean-agents` remains dry-run after the TTL unless `--apply` is supplied. `--repair-acl` is an opt-in Windows recovery path scoped to an already validated target.

Reports traverse each target once, show scan time and truncation state, and place a time/entry budget on legacy scans so historical million-file trees cannot stall the command indefinitely.

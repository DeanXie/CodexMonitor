# Worktree Build Storage Governance Design

## Goal

Prevent CodexMonitor's ephemeral Rust/Tauri Worktrees from accumulating unbounded Cargo build artifacts while preserving independent Git Worktrees, parallel agents, main development, release builds, CI, and version authority.

## Architecture

The solution combines five required controls: an external target directory per ephemeral Worktree, `CARGO_INCREMENTAL=0` for ephemeral builds, an explicit closeout lifecycle, orphan detection with a seven-day eligibility delay, and a pre-build disk guard. External target isolation is only the storage boundary; cleanup and guards provide the capacity bound.

Formal project commands inject `CARGO_TARGET_DIR` and `CARGO_INCREMENTAL` into child Cargo or Tauri processes. A Worktree-local Cargo config is not used because an empirical fixture showed that Cargo invoked from the repository root with `--manifest-path` did not discover `src-tauri/.cargo/config.toml`, while invocation from `src-tauri` did.

## Target classes

- `MAIN`: the primary checkout's existing `src-tauri/target`; never cleaned by this system.
- `ACTIVE`: an external ephemeral target whose canonical Worktree remains registered.
- `CLOSEOUT-ELIGIBLE`: the current registered Worktree's external target when the checkout is clean and `--accepted` is supplied.
- `ORPHAN`: an external target whose manifest no longer maps to a registered Worktree. It is deletion-eligible only after seven days.
- `LEGACY-IN-TREE`: a registered linked Worktree containing `src-tauri/target`; reported but never batch-deleted.
- `RELEASE`: release/production artifacts identified outside the ephemeral agents root; never cleaned by agent cleanup.

## Paths and identity

`CODEXMONITOR_CARGO_TARGET_ROOT` overrides the build root. Otherwise Windows uses a directory adjacent to the canonical main checkout on the same volume; macOS uses `~/Library/Caches/CodexMonitor/cargo-target`; Linux uses `$XDG_CACHE_HOME/codexmonitor/cargo-target` or `~/.cache/codexmonitor/cargo-target`.

Each external target is `agents/<sanitized-worktree-name>-<sha256-prefix>`, where the hash covers the canonical Git common directory and canonical Worktree path. Each directory contains a schema-versioned manifest with repository identity, Worktree path, target path, and timestamps.

An existing non-empty deterministic directory without that manifest is never adopted as managed storage. An existing manifest must pass the full repository, Worktree, target, and deterministic-path identity checks before its usage timestamp can be refreshed.

## Build entrypoints

`storage:prepare`, `rust:check`, `rust:test`, ephemeral `tauri:dev`, and ephemeral `tauri:build` all run the same idempotent ensure and guard logic. Main commands preserve existing Cargo behavior. `AGENTS.md` prohibits bare Cargo commands in ephemeral Worktrees because a global Cargo interception would be invasive and cross-project unsafe.

The guard warns when total agent targets exceed 80 GiB or one target exceeds 25 GiB. It fails closed before a heavy ephemeral build when total targets exceed 120 GiB, one target exceeds 40 GiB, or target-volume free space is below 50 GiB. Free space below 100 GiB warns. An explicit override is logged with the observed values.

## Report and cleanup

`storage:report` performs one traversal per discovered directory, reports elapsed scan time, and avoids rescanning. Controlled external targets are scanned first. Legacy targets are a labeled slow path.

`storage:closeout` defaults to dry-run. Apply requires the current checkout to be a clean linked Worktree, `--accepted`, an exact manifest match, an allowlisted external target, and a non-main/non-release classification. It deletes only the target; the Git Worktree, source, and branch remain.

`storage:clean-agents` reports all orphan candidates by default. `--apply` may delete only allowlisted orphans older than the configured TTL; registered Worktrees use `storage:closeout`. Newly orphaned targets are reported immediately but are not deletable.

All deletion re-resolves canonical paths, rejects root deletion, rejects `.git` and Worktree roots, and rejects symlink/junction/reparse traversal. Windows ACL recovery is explicit, candidate-scoped, logged, and never uses repository-wide `takeown` or ACL changes.

Build and cleanup acquire one atomic sibling operation lock before using or deleting a target. Dead-owner locks become recoverable only after the configured stale interval; live-owner locks are not age-expired.

## Configuration

`config/storage-governance.json` is the sole threshold authority. Initial values are 80/120 GiB total warning/critical, 25/40 GiB per-target warning/critical, 100/50 GiB free-space warning/block, seven orphan days, and a five-minute dead-owner lock recovery delay.

## Migration and rollback

Existing real Worktrees and build artifacts are never deleted during installation or validation. Legacy in-tree targets are reported. New formal builds use external targets. Rollback removes the package scripts and governance files; Cargo then resumes its original workspace-local target behavior. External targets remain intact until explicitly cleaned.

## Validation

Automated tests cover classification, unique keys, thresholds, clean/dirty closeout, root and `.git` rejection, symlink/reparse rejection, orphan TTL, dry-run immutability, apply deletion, and injected low-space failures. Local acceptance creates two temporary Worktrees, runs real Cargo check/test through wrappers, validates distinct external targets, closes one target while preserving its Worktree and branch, simulates an under-age orphan, and verifies main metadata, version gates, diff, and final report.

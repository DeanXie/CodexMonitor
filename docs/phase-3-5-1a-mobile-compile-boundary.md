# Phase 3.5.1a — Mobile Compile Boundary

Status: **PASS / FROZEN (Windows implementation scope)**.

Deferred external gate: **Real iOS target/artifact validation — NOT YET EXECUTED**.

Next development start point: **Phase 3.5.1b — RemoteHostIdentity: GO / NOT STARTED**.

## Frozen implementation contract

- `CodexHomeIdentity`, `CodexThreadKey`, and `CodexTurnKey` have one canonical definition in the platform-neutral `shared/codex_identity.rs` module.
- Existing `global_sources_core` type paths are compatibility re-exports only.
- Mobile may receive, deserialize, compare, and carry `CodexHomeIdentity` as opaque host-provided evidence. Host identity construction from a local `CODEX_HOME` remains host/desktop-only.
- Desktop-only Global Source runtime, rollout watcher, Desktop metadata/projection adapters, tombstone runtime, and local `CODEX_HOME` construction remain outside the mobile target graph.
- Mobile `read_thread` routes through `remote_backend` to the host daemon and host app-server. Remote unavailability is an error; there is no local app-server, local Registry, or local `CODEX_HOME` fallback.
- Mobile `delete_thread` routes through `remote_backend` to the host official `thread/delete`. Mobile does not compile or execute local tombstone reconciliation.
- Host-authoritative tombstone persistence remains **NOT IMPLEMENTED / NOT PROVEN** and is not part of this slice.
- Canonical Thread identity remains `CodexThreadKey = (codexHomeIdentity, fullThreadId)`.

Implementation validation commit: `2864a077941740602a539909e2df889a31502976` (`refactor: isolate mobile-safe codex identity`).

## Windows validation

The implementation passed the Windows development gates:

- neutral identity focused tests and serde/equality/hash regression;
- Phase 3.1 exact-ID regression;
- Phase 3.2 Workspace/Project regression;
- Phase 3.3 creation/settings regression;
- Phase 3.4 projection/tombstone regression;
- remote backend and read/delete RPC shape regression;
- `cargo test --lib`;
- daemon tests;
- `cargo check --all-targets`;
- `cargo fmt --all -- --check`;
- `npm run typecheck`;
- `git diff --check`.

## Deferred Mobile Artifact Validation

The following are deliberately not claimed by Phase 3.5.1a Windows completion:

- `cargo check --target aarch64-apple-ios`;
- `cargo check --target aarch64-apple-ios-sim`;
- Tauri iOS project generation, Xcode build, signing, installation, or artifact output;
- physical-device Mobile E2E.

These gates require a real Mac/iOS toolchain and are deferred until the future Mobile build/device E2E stage. Their absence does not block the Windows development mainline, and this document must never be interpreted as evidence that an iOS artifact was validated.

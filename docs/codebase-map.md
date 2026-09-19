# Codebase Map (Task-Oriented)

Canonical navigation guide for CodexMonitor. Use this as: "if you need X, edit Y".

Related docs:

- Setup/build/release: `README.md`
- Phase 4 product/release contract: `docs/phase-4-0-truth-release-boundary.md`
- Version authority/tooling: `VERSION.json`, `release-identity.json`,
  `scripts/version-authority.mjs`
- Worktree build storage policy/tooling: `config/storage-governance.json`,
  `scripts/storage-governance.mjs`, `scripts/storage-governance-core.mjs`,
  `docs/build-storage-governance.md`
- Controlled whitelist migration preparation: `src-tauri/src/shared/migration_core.rs`,
  `src-tauri/src/shared/migration_core_tests.rs`,
  `docs/phase-4-1c-whitelist-migration.md`
- Inactive migration/activation safety foundation:
  `src-tauri/src/shared/activation_foundation.rs`,
  `src-tauri/src/shared/remote_host_identity_activation.rs`,
  `src-tauri/src/shared/activation_foundation_tests.rs`,
  `docs/phase-4-1d-1-migration-activation-foundation.md`
- Creation-intent / first-Turn coordination: `docs/phase-3-3-2-creation-coordination.md`
- iOS remote over Tailscale (TCP): `docs/mobile-ios-tailscale-blueprint.md`

## Start Here: How Changes Flow

For backend behavior, follow this path in order:

1. Frontend callsite: `src/features/**` hooks/components
2. Frontend IPC API: `src/services/tauri.ts`
3. Tauri command registration: `src-tauri/src/lib.rs` (`invoke_handler`)
4. App adapter: `src-tauri/src/{codex,workspaces,git,files,settings,prompts}/*`
5. Shared core source of truth: `src-tauri/src/shared/*`
6. Daemon RPC method parity: `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`
7. Daemon state/wiring implementation: `src-tauri/src/bin/codex_monitor_daemon.rs`
8. Standalone daemon lifecycle CLI: `src-tauri/src/bin/codex_monitor_daemonctl.rs`

If a behavior must work in both app and daemon, implement it in `src-tauri/src/shared/*` first.

## If You Need X, Edit Y

| Need | Primary files to edit |
| --- | --- |
| App-level UI composition/layout wiring | `src/App.tsx`, `src/features/app/components/AppLayout.tsx`, `src/features/app/bootstrap/*`, `src/features/app/orchestration/*`, `src/features/app/hooks/*` |
| Add/change Tauri IPC methods used by frontend | `src/services/tauri.ts`, `src-tauri/src/lib.rs`, matching backend adapter module |
| Add/change app-server event handling in UI | `src/services/events.ts`, `src/features/app/hooks/useAppServerEvents.ts`, `src/utils/appServerEvents.ts`, `src/features/threads/utils/threadNormalize.ts` |
| Change thread state transitions | `src/features/threads/hooks/useThreadsReducer.ts`, `src/features/threads/hooks/threadReducer/*`, `src/features/threads/hooks/useThreads.ts`, focused thread hooks under `src/features/threads/hooks/*` |
| Change workspace lifecycle/worktree behavior | `src/features/workspaces/hooks/useWorkspaces.ts`, `src-tauri/src/workspaces/commands.rs`, `src-tauri/src/shared/workspaces_core.rs`, `src-tauri/src/shared/workspaces_core/*`, `src-tauri/src/shared/worktree_core.rs` |
| Change Cargo target isolation, disk guard, report, closeout, or orphan cleanup | `config/storage-governance.json`, `scripts/storage-governance.mjs`, `scripts/storage-governance-core.mjs`, matching tests, `docs/build-storage-governance.md` |
| Change settings model/load/update | `src/features/settings/components/SettingsView.tsx`, `src/features/settings/hooks/useAppSettings.ts`, `src/services/tauri.ts`, `src-tauri/src/settings/mod.rs`, `src-tauri/src/shared/settings_core.rs`, `src-tauri/src/types.rs`, `src/types.ts` |
| Change Git/GitHub backend behavior | `src/features/git/hooks/*`, `src/services/tauri.ts`, `src-tauri/src/git/mod.rs`, `src-tauri/src/shared/git_ui_core.rs`, `src-tauri/src/shared/git_ui_core/*`, `src-tauri/src/shared/git_core.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc/git.rs` |
| Change prompts CRUD/listing behavior | `src/features/prompts/hooks/useCustomPrompts.ts`, `src/features/prompts/components/PromptPanel.tsx`, `src/services/tauri.ts`, `src-tauri/src/prompts.rs`, `src-tauri/src/shared/prompts_core.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc.rs` |
| Change file read/write for Agents/config | `src/services/tauri.ts`, `src-tauri/src/files/mod.rs`, `src-tauri/src/shared/files_core.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc.rs` |
| Add/change daemon JSON-RPC surface | `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc/*`, `src-tauri/src/bin/codex_monitor_daemon.rs`, matching shared core |
| Change writer-admission observation/query compatibility | `src-tauri/src/shared/codex_core/writer_admission_observation.rs`, `src-tauri/src/shared/codex_core/writer_admission_*_tests.rs`, `src-tauri/src/shared/codex_core/writer_admission_protocol_fixture_tests.rs`, `docs/fixtures/app-server/writer-admission-observation/*`, `docs/phase-3-5-2b-host-session-writer-admission-observation.md` |
| Change Thread subscription/runtime observations or synthetic live detach | `src-tauri/src/shared/codex_core/thread_lifecycle_observation.rs`, `src-tauri/src/shared/codex_core/thread_lifecycle_observation_tests.rs`, `src-tauri/src/shared/codex_core/thread_lifecycle_protocol_fixture_tests.rs`, `src-tauri/src/shared/codex_core/synthetic_live_detach_tests.rs`, `src-tauri/src/codex/mod.rs`, `src-tauri/src/bin/codex_monitor_daemon.rs`, `src-tauri/src/remote_backend/mod.rs`, `docs/fixtures/app-server/thread-lifecycle-observation/*`, `docs/phase-3-5-2c-subscription-release-lifecycle.md` |
| Change Remote transport generation, daemon-process restart continuity, stale notification delivery, request provenance, session-attempt dispatch correlation, or their frozen compatibility fixtures | `src-tauri/src/shared/remote_host_identity.rs`, `src-tauri/src/shared/remote_request_provenance.rs`, `src-tauri/src/shared/remote_request_provenance_tests.rs`, `src-tauri/src/shared/remote_transport_compatibility_tests.rs`, `src-tauri/src/remote_backend/mod.rs`, `src-tauri/src/remote_backend/transport.rs`, `src-tauri/src/remote_backend/tcp_transport.rs`, `src-tauri/src/bin/codex_monitor_daemon.rs`, `src-tauri/src/bin/codex_monitor_daemon/daemon_restart_tests.rs`, `src-tauri/src/bin/codex_monitor_daemon/transport.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`, `src-tauri/src/bin/codex_monitor_daemon/remote_dispatch_correlation_tests.rs`, `docs/fixtures/remote-transport-coordination/*`, `docs/phase-3-5-2d-remote-transport-coordination.md` |
| Change approval request observation or Remote approval decision correlation | `src-tauri/src/shared/codex_core/approval_observation.rs`, `src-tauri/src/shared/codex_core/approval_decision_provenance.rs`, their focused tests, `src-tauri/src/backend/app_server.rs`, `src-tauri/src/bin/codex_monitor_daemon.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`, `src-tauri/src/bin/codex_monitor_daemon/remote_dispatch_correlation_tests.rs`, `docs/fixtures/app-server/approval-request-observation/*`, `docs/fixtures/app-server/approval-decision-provenance/*`, `docs/phase-3-5-3-approval-delete-authority.md` |
| Change exact Thread delete authority, active-delete isolation, transport-loss correlation, or confirmed-delete reconciliation | `src-tauri/src/shared/codex_core/delete_mutation_observation.rs`, `src-tauri/src/shared/codex_core/delete_mutation_*_tests.rs`, `src-tauri/src/shared/codex_core/creation_coordination.rs`, `src-tauri/src/backend/app_server.rs`, `src-tauri/src/codex/mod.rs`, `src-tauri/src/bin/codex_monitor_daemon.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`, `src-tauri/src/bin/codex_monitor_daemon/remote_dispatch_correlation_tests.rs`, `src-tauri/src/shared/remote_request_provenance.rs`, `docs/fixtures/app-server/delete-mutation-observation/*`, `docs/fixtures/app-server/delete-mutation-isolation/*`, `docs/phase-3-5-3-approval-delete-authority.md` |
| Change the frozen Phase 3.5.3 approval/delete compatibility contract | `src-tauri/src/shared/codex_core/phase_3_5_3_compatibility_tests.rs`, `docs/fixtures/app-server/phase-3-5-3-compatibility/*`, `docs/phase-3-5-3-approval-delete-authority.md`, `docs/evidence/phase-3-5-3e/*` |
| Change projection-freshness status, generation invalidation, authoritative read instrumentation, or App/daemon query parity | `src-tauri/src/shared/projection_freshness.rs`, `src-tauri/src/shared/projection_freshness_tests.rs`, `src-tauri/src/shared/projection_freshness_fixture_tests.rs`, `src-tauri/src/shared/codex_core.rs`, `src-tauri/src/codex/mod.rs`, `src-tauri/src/bin/codex_monitor_daemon.rs`, `src-tauri/src/bin/codex_monitor_daemon/rpc/codex.rs`, `src/services/tauri.ts`, `src/types.ts`, `docs/fixtures/projection-freshness/*`, `docs/phase-3-5-4-projection-recovery-telemetry.md` |
| Change the frozen Phase 3.5.4 projection/recovery/telemetry compatibility contract | `src-tauri/src/shared/phase_3_5_4_compatibility_tests.rs`, `src/features/app/orchestration/phase354Compatibility.test.ts`, `docs/fixtures/phase-3-5-4-compatibility/*`, `docs/phase-3-5-4-projection-recovery-telemetry.md`, `docs/evidence/phase-3-5-4e/*` |
| Change the frozen Phase 3.5 final integration acceptance contract or isolated read-only harness | `src-tauri/src/shared/phase_3_5_final_acceptance_tests.rs`, `src/features/app/orchestration/phase35FinalAcceptance.test.ts`, `scripts/phase-3-5-final-acceptance.mjs`, `scripts/phase-3-5-final-acceptance.test.mjs`, `docs/fixtures/phase-3-5-final-acceptance/*`, `docs/phase-3-5-final-integration-acceptance.md`, `docs/evidence/phase-3-5-final/*` |
| Change authoritative reload/reconnect/session-replacement hydration, aggregate observation reads, or generation-safe frontend apply gates | `src/features/app/orchestration/authoritativeRecovery.ts`, `src/features/app/hooks/useMainAppWorkspaceLifecycle.ts`, workspace/Remote refresh hooks, `src/services/tauri.ts`, `src-tauri/src/shared/codex_core/authoritative_recovery.rs`, `src-tauri/src/shared/codex_core.rs`, App/daemon adapters, `src-tauri/tests/fixtures/phase-3-5-4c-authoritative-hydration/*`, `docs/phase-3-5-4-projection-recovery-telemetry.md` |
| Change Remote offline/stale UI, coverage-specific status mapping, event-gap invalidation, or approval/delete stale projection safety | `src/features/app/orchestration/projectionStatusModel.ts`, `src/features/app/hooks/useRemoteProjectionStatus.ts`, `src/features/app/components/ProjectionStatusIndicator.tsx`, `src/features/app/components/ApprovalToasts.tsx`, `src/services/events.ts`, `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`, `src-tauri/src/remote_backend/transport.rs`, `src-tauri/tests/fixtures/phase-3-5-4d-offline-stale-ui/*`, `docs/phase-3-5-4-projection-recovery-telemetry.md` |

## Frontend Navigation

- Composition root: `src/App.tsx`
- App bootstrap orchestration: `src/features/app/bootstrap/*`
- App layout/thread/workspace orchestration: `src/features/app/orchestration/*`
- Tauri IPC wrapper: `src/services/tauri.ts`
- Tauri event hub (single-listener fanout): `src/services/events.ts`
- Event subscription hook: `src/features/app/hooks/useTauriEvent.ts`
- App-server event router: `src/features/app/hooks/useAppServerEvents.ts`
- Shared frontend types: `src/types.ts`

### Import Aliases

Use TS/Vite aliases for refactor-safe imports:

- `@/*` -> `src/*`
- `@app/*` -> `src/features/app/*`
- `@settings/*` -> `src/features/settings/*`
- `@threads/*` -> `src/features/threads/*`
- `@services/*` -> `src/services/*`
- `@utils/*` -> `src/utils/*`

### Threads

- Orchestrator: `src/features/threads/hooks/useThreads.ts`
- Reducer composition entrypoint: `src/features/threads/hooks/useThreadsReducer.ts`
- Reducer slices: `src/features/threads/hooks/threadReducer/*`
- Event-focused handlers: `src/features/threads/hooks/useThreadEventHandlers.ts`, `src/features/threads/hooks/useThreadTurnEvents.ts`, `src/features/threads/hooks/useThreadItemEvents.ts`, `src/features/threads/hooks/useThreadApprovalEvents.ts`, `src/features/threads/hooks/useThreadUserInputEvents.ts`
- Message send/steer/interrupt: `src/features/threads/hooks/useThreadMessaging.ts`
- Explicit creation/first-Turn action tokens: `src/features/threads/hooks/creationAction.ts`; process-local dispatch ownership: `src-tauri/src/shared/codex_core/creation_coordination.rs`
- Persistence/local thread metadata: `src/features/threads/hooks/useThreadStorage.ts`, `src/features/threads/utils/threadStorage.ts`

### Workspaces

- Workspace state and lifecycle: `src/features/workspaces/hooks/useWorkspaces.ts`
- Workspace home behavior: `src/features/workspaces/hooks/useWorkspaceHome.ts`
- Workspace file list and reads in app layer: `src/features/app/hooks/useWorkspaceFileListing.ts`, `src/features/workspaces/hooks/useWorkspaceFiles.ts`

### Settings

- Main settings surface: `src/features/settings/components/SettingsView.tsx`
- Settings state + persistence flow: `src/features/settings/hooks/useAppSettings.ts`, `src/features/app/hooks/useAppSettingsController.ts`
- Typed settings contracts: `src/types.ts`

### Git

- Git UI hooks: `src/features/git/hooks/*`
- Git panel components: `src/features/git/components/*`
- Branch workflows: `src/features/git/hooks/useGitBranches.ts`, `src/features/git/hooks/useBranchSwitcher.ts`

### Prompts

- Prompt UI and workflow: `src/features/prompts/components/PromptPanel.tsx`, `src/features/prompts/hooks/useCustomPrompts.ts`

## Backend App (Tauri) Navigation

- Command registry (what frontend can invoke): `src-tauri/src/lib.rs`
- Codex adapters: `src-tauri/src/codex/mod.rs`
- Workspace/worktree adapters: `src-tauri/src/workspaces/commands.rs`
- Git adapters: `src-tauri/src/git/mod.rs`
- Settings adapters: `src-tauri/src/settings/mod.rs`
- Prompts adapters: `src-tauri/src/prompts.rs`
- File adapters: `src-tauri/src/files/mod.rs`
- Event emission implementation: `src-tauri/src/event_sink.rs`
- Event payload definitions: `src-tauri/src/backend/events.rs`

## Daemon Navigation

- Daemon entrypoint and state/wiring: `src-tauri/src/bin/codex_monitor_daemon.rs`
- Daemon lifecycle CLI (headless start/stop/status): `src-tauri/src/bin/codex_monitor_daemonctl.rs`
- Daemon JSON-RPC dispatcher/router: `src-tauri/src/bin/codex_monitor_daemon/rpc.rs`
- Daemon domain handlers: `src-tauri/src/bin/codex_monitor_daemon/rpc/*`
- Daemon transport: `src-tauri/src/bin/codex_monitor_daemon/transport.rs`

When adding a new method, keep method names and payload shape aligned with `src/services/tauri.ts` and app commands in `src-tauri/src/lib.rs`.

## Shared Cores (Source of Truth)

All cross-runtime domain behavior belongs in `src-tauri/src/shared/*`:

- Codex threads/approvals/account/skills/config: `src-tauri/src/shared/codex_core.rs`
- Approval request observation authority: `src-tauri/src/shared/codex_core/approval_observation.rs`
- Approval/delete compatibility freeze: `src-tauri/src/shared/codex_core/phase_3_5_3_compatibility_tests.rs`
- Approval actionable projection and exact resolved reconciliation:
  `src/features/app/hooks/useAppServerEvents.ts` and
  `src/features/threads/hooks/threadReducer/threadQueueSlice.ts`
- Codex helper commands: `src-tauri/src/shared/codex_aux_core.rs`
- Codex update/version helpers: `src-tauri/src/shared/codex_update_core.rs`
- Workspaces/worktrees: `src-tauri/src/shared/workspaces_core.rs`, `src-tauri/src/shared/workspaces_core/*`, `src-tauri/src/shared/worktree_core.rs`
- Settings model/update: `src-tauri/src/shared/settings_core.rs`
- Files read/write: `src-tauri/src/shared/files_core.rs`
- Git and GitHub logic: `src-tauri/src/shared/git_core.rs`, `src-tauri/src/shared/git_ui_core.rs`, `src-tauri/src/shared/git_ui_core/*`
- Prompts CRUD/listing: `src-tauri/src/shared/prompts_core.rs`
- Usage snapshot and aggregation: `src-tauri/src/shared/local_usage_core.rs`
- External Codex source envelopes, rollout discovery/tailing, checkpoints, and source authority: `src-tauri/src/shared/global_sources_core.rs`, `src-tauri/src/shared/global_sources_core/*`
- Process helpers: `src-tauri/src/shared/process_core.rs`
- Remote host and daemon-process identity: `src-tauri/src/shared/remote_host_identity.rs`
- Remote TCP request correlation and reconnect isolation: `src-tauri/src/shared/remote_request_provenance.rs`
- Remote coordination compatibility fixtures: `docs/fixtures/remote-transport-coordination/*`; shared App/daemon fixture tests: `src-tauri/src/shared/remote_transport_compatibility_tests.rs`

## Events Map (Backend -> Frontend)

- Backend emits through sink: `src-tauri/src/event_sink.rs`
- App-server event name: `app-server-event`
- Terminal event names: `terminal-output`, `terminal-exit`
- Frontend fanout hubs: `src/services/events.ts`
- Frontend routing into thread state: `src/features/app/hooks/useAppServerEvents.ts` -> thread hooks/reducer under `src/features/threads/hooks/*`
- Shared generation-tagged envelope: `src-tauri/src/backend/events.rs`
- Workspace/app-server generation binding: `src-tauri/src/backend/app_server.rs`
- Daemon process/transport delivery binding: `src-tauri/src/bin/codex_monitor_daemon/rpc.rs` + `transport.rs`
- Frontend current-generation admission gate: `src/services/events.ts`
- Read-only freshness-context capture: `src/services/tauri.ts`
- Generation delivery fixtures: `docs/fixtures/generation-tagged-events/`

If the raw app-server message format changes, update parser/guards first in
`src/utils/appServerEvents.ts`. If the CodexMonitor delivery envelope changes,
keep Rust/daemon/TypeScript generation fields and `src/services/events.ts`
admission tests in parity.

## Type Contract Files

Keep Rust and TypeScript contracts in sync:

- Rust backend types: `src-tauri/src/types.rs`
- Frontend types: `src/types.ts`

This is required for settings, workspace metadata, app-server payload handling, and RPC response decoding.

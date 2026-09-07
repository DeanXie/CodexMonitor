pub(crate) mod app_server_live;
pub(crate) mod diagnostics;
pub(crate) mod runtime;
pub(crate) mod snapshot;

use crate::codex::home::{resolve_default_codex_home, resolve_workspace_codex_home};
use crate::shared::global_sources_core::desktop_projection::DesktopProjectionState;
use crate::shared::global_sources_core::desktop_projection::WorkspaceRoot;
use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchService;
use crate::shared::global_sources_core::rollout_watcher::{
    RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy,
};
use crate::shared::global_sources_core::runtime_config::{
    discover_runtime_codex_homes, GlobalSourceRuntimePaths,
};
use crate::shared::surface_projection_core::CanonicalThreadProjectionState;
use crate::shared::surface_projection_engine::{
    desktop_inventory_observation, ProjectionObservationEngine,
};
use crate::state::AppState;
use diagnostics::DiagnosticJournal;
use serde_json::json;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let workspaces =
        tauri::async_runtime::block_on(async { state.workspaces.lock().await.clone() });
    let default_home = resolve_default_codex_home();
    let mut workspace_paths = Vec::new();
    let mut workspace_path_by_id = HashMap::new();
    let workspace_roots = workspaces
        .iter()
        .map(|(workspace_id, entry)| WorkspaceRoot::new(workspace_id.clone(), entry.path.clone()))
        .collect::<Vec<_>>();
    for (workspace_id, entry) in &workspaces {
        let parent = entry
            .parent_id
            .as_ref()
            .and_then(|parent_id| workspaces.get(parent_id));
        if let Some(home) = resolve_workspace_codex_home(entry, parent) {
            workspace_paths.push(home.clone());
            workspace_path_by_id.insert(workspace_id.clone(), home);
        }
    }
    let homes = discover_runtime_codex_homes(default_home, workspace_paths);
    let identities_by_path = homes
        .iter()
        .map(|home| (path_key(&home.root), home.codex_home.clone()))
        .collect::<HashMap<_, _>>();
    let workspace_homes = workspace_path_by_id
        .into_iter()
        .filter_map(|(workspace_id, path)| {
            identities_by_path
                .get(&path_key(&path))
                .cloned()
                .map(|identity| (workspace_id, identity))
        })
        .collect::<Vec<_>>();
    let app_data_root = state
        .storage_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| state.storage_path.clone());
    let paths = GlobalSourceRuntimePaths::new(&app_data_root);
    let journal = DiagnosticJournal::new(paths.diagnostics_path.clone());
    journal
        .record_service_state(
            "initializing",
            json!({
                "codexHomeCount": homes.len(),
                "checkpointPath": paths.checkpoint_path,
                "deletionTombstonesPath": paths.deletion_tombstones_path,
            }),
        )
        .map_err(|error| error.to_string())?;
    let config = RolloutWatcherConfig {
        homes: homes.clone(),
        checkpoint_path: paths.checkpoint_path.clone(),
        deletion_tombstones_path: paths.deletion_tombstones_path.clone(),
        retry: WatcherRetryPolicy {
            max_attempts: 5,
            initial_backoff_ms: 50,
        },
        fresh_window_ms: 5_000,
        settled_after_ms: 2_000,
        reconciliation_interval_ms: 500,
    };
    let watcher = RolloutTailWatcher::new(config).with_workspace_roots(workspace_roots);
    let service = RolloutWatchService::new(watcher).map_err(|error| error.to_string())?;
    let source_instance_id = format!("monitor-app-server:{}", uuid::Uuid::new_v4());
    state
        .global_rollout_runtime
        .configure_live_sources(source_instance_id.clone(), workspace_homes);
    let snapshot_app = app.clone();
    let started = state
        .global_rollout_runtime
        .start(move |shutdown, commands| async move {
            let _ = journal.record_service_state(
                "started",
                json!({
                    "sourceInstanceId": source_instance_id,
                    "codexHomes": homes.iter().map(|home| &home.codex_home).collect::<Vec<_>>(),
                    "checkpointPath": paths.checkpoint_path,
                }),
            );
            let result = service
                .run_until(shutdown, commands, |mut event, registry| {
                    let app_state = snapshot_app.state::<AppState>();
                    if let crate::shared::global_sources_core::rollout_watch_service::RolloutWatchEvent::Reconciled(report) = &event {
                        app_state
                            .execution_settings_evidence
                            .observe_rollout_observations(
                                report.execution_settings_turn_contexts.clone(),
                            );
                    }
                    let runtime = &app_state.global_rollout_runtime;
                    let generated_at_ms = chrono::Utc::now().timestamp_millis();
                    let projection_updates = match &event {
                        crate::shared::global_sources_core::rollout_watch_service::RolloutWatchEvent::Reconciled(report) => {
                            desktop_projection_updates(report, generated_at_ms.max(0) as u64)
                        }
                        _ => Vec::new(),
                    };
                    let published_snapshot = runtime.publish_snapshot(
                        registry.snapshot(),
                        projection_updates,
                        generated_at_ms,
                    );
                    if let crate::shared::global_sources_core::rollout_watch_service::RolloutWatchEvent::DeletionReconciled(report) = &mut event {
                        report.snapshot_publication_revision = Some(runtime.snapshot().revision);
                    }
                    let _ = journal.record_watch_event(&event, registry);
                    if let Some(snapshot) = published_snapshot {
                        let _ = snapshot_app
                            .emit(snapshot::GLOBAL_SOURCE_SNAPSHOT_UPDATED_EVENT, snapshot);
                    }
                })
                .await;
            if let Err(error) = result {
                let _ =
                    journal.record_service_state("failed", json!({ "message": error.to_string() }));
            }
            let _ = journal.record_service_state("stopped", json!({}));
        });
    if started {
        Ok(())
    } else {
        Err("global rollout watch service is already running".to_string())
    }
}

fn desktop_projection_updates(
    report: &crate::shared::global_sources_core::rollout_watcher::ReconcileReport,
    observed_at: u64,
) -> Vec<crate::shared::surface_projection_core::SurfaceProjectionObservation> {
    report
        .desktop_projection_observations
        .iter()
        .filter_map(|reported| {
            let engine = ProjectionObservationEngine::default();
            let observation = desktop_inventory_observation(
                reported.thread_key.clone(),
                crate::shared::surface_projection_core::SurfaceProjectionKind::Catalog,
                std::slice::from_ref(&reported.thread_key.thread_id),
                crate::shared::surface_projection_core::ObservationCoverage::Complete,
                observed_at,
            );
            engine.observe(observation);
            let canonical_state = match reported.assessment.state {
                DesktopProjectionState::CanonicalSupplement => {
                    CanonicalThreadProjectionState::Present
                }
                DesktopProjectionState::DesktopStaleOrphan => {
                    CanonicalThreadProjectionState::Absent
                }
                DesktopProjectionState::Ambiguous => CanonicalThreadProjectionState::Unknown,
            };
            let key = crate::shared::surface_projection_core::SurfaceProjectionKey::new(
                reported.thread_key.clone(),
                crate::shared::surface_projection_core::SurfaceProjectionSurface::Desktop,
                crate::shared::surface_projection_core::SurfaceProjectionKind::Catalog,
            );
            engine.effective(&key, canonical_state)
        })
        .collect()
}

pub(crate) async fn shutdown(app: &AppHandle) {
    app.state::<AppState>()
        .global_rollout_runtime
        .shutdown()
        .await;
}

fn path_key(path: &std::path::Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::global_sources_core::desktop_projection::{
        DesktopProjectionAssessment, DesktopProjectionState,
    };
    use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
    use crate::shared::global_sources_core::rollout_watcher::{
        DesktopProjectionObservation, ReconcileReport,
    };
    use crate::shared::surface_projection_core::{
        ProjectionReconciliationState, SurfaceProjectionState, DESKTOP_STALE_ORPHAN_DIAGNOSTIC,
    };

    #[test]
    fn desktop_stale_assessment_is_exported_as_engine_determined_stale_pending() {
        let report = ReconcileReport {
            desktop_projection_observations: vec![DesktopProjectionObservation {
                thread_key: CodexThreadKey::new("home-1", "deleted-thread"),
                assessment: DesktopProjectionAssessment {
                    state: DesktopProjectionState::DesktopStaleOrphan,
                    canonical_ingest_allowed: false,
                    evidence: vec!["tombstone".to_string()],
                },
            }],
            ..ReconcileReport::default()
        };

        let observations = desktop_projection_updates(&report, 1_000);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].state, SurfaceProjectionState::Stale);
        assert_eq!(
            observations[0].reconciliation_state,
            ProjectionReconciliationState::Pending
        );
        assert!(observations[0]
            .diagnostics
            .contains(&DESKTOP_STALE_ORPHAN_DIAGNOSTIC.to_string()));
    }
}

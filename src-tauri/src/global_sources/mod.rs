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
                    let canonical_snapshot = registry.snapshot();
                    let projection_updates = match &event {
                        crate::shared::global_sources_core::rollout_watch_service::RolloutWatchEvent::Reconciled(report) => {
                            desktop_projection_updates(
                                report,
                                &canonical_snapshot,
                                generated_at_ms.max(0) as u64,
                            )
                        }
                        _ => Vec::new(),
                    };
                    let published_snapshot = runtime.publish_snapshot(
                        canonical_snapshot,
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
    canonical: &crate::shared::global_sources_core::source_registry::CanonicalSourceSnapshot,
    observed_at: u64,
) -> Vec<crate::shared::surface_projection_core::SurfaceProjectionObservation> {
    let mut keys = canonical
        .threads
        .iter()
        .map(|thread| thread.key.clone())
        .chain(
            report
                .desktop_projection_observations
                .iter()
                .map(|reported| reported.thread_key.clone()),
        )
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        left.codex_home_identity
            .cmp(&right.codex_home_identity)
            .then_with(|| left.thread_id.cmp(&right.thread_id))
    });
    keys.dedup();

    keys.into_iter()
        .filter_map(|thread_key| {
            let inventory = report
                .desktop_catalog_inventories
                .iter()
                .find(|inventory| {
                    inventory.codex_home_identity == thread_key.codex_home_identity
                })?;
            let engine = ProjectionObservationEngine::default();
            let mut observation = desktop_inventory_observation(
                thread_key.clone(),
                crate::shared::surface_projection_core::SurfaceProjectionKind::Catalog,
                &inventory.observed_thread_ids,
                inventory.coverage,
                observed_at,
            );
            for diagnostic in &inventory.diagnostics {
                observation = observation.with_diagnostic(format!(
                    "{}:{}:{}",
                    diagnostic.source, diagnostic.code, diagnostic.message
                ));
            }
            engine.observe(observation);
            let canonical_state = if canonical
                .threads
                .iter()
                .any(|thread| thread.key == thread_key)
            {
                CanonicalThreadProjectionState::Present
            } else {
                match report
                    .desktop_projection_observations
                    .iter()
                    .find(|reported| reported.thread_key == thread_key)?
                    .assessment
                    .state
                {
                    DesktopProjectionState::CanonicalSupplement => {
                        CanonicalThreadProjectionState::Present
                    }
                    DesktopProjectionState::DesktopStaleOrphan => {
                        CanonicalThreadProjectionState::Absent
                    }
                    DesktopProjectionState::Ambiguous => CanonicalThreadProjectionState::Unknown,
                }
            };
            let key = crate::shared::surface_projection_core::SurfaceProjectionKey::new(
                thread_key,
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
        DesktopCatalogInventoryReport, DesktopProjectionObservation, ReconcileReport,
    };
    use crate::shared::global_sources_core::source_registry::{
        CanonicalSourceSnapshot, CanonicalSourceThread,
    };
    use crate::shared::surface_projection_core::{
        ObservationCoverage, ProjectionReconciliationState, SurfaceProjectionKind,
        SurfaceProjectionState, DESKTOP_STALE_ORPHAN_DIAGNOSTIC,
    };

    fn canonical_thread(home: &str, thread_id: &str) -> CanonicalSourceThread {
        CanonicalSourceThread {
            key: CodexThreadKey::new(home, thread_id),
            parent_thread_key: None,
            agent_path: None,
            current_turn: None,
            lifecycle: None,
            observed_model: None,
            token_snapshot: None,
            producer_surface: Default::default(),
            workspace_assignment: None,
            authority_provenance: None,
            live_lane_count: 0,
            near_live_lane_count: 1,
            historical_lane_count: 0,
        }
    }

    fn canonical(home: &str, thread_ids: &[&str]) -> CanonicalSourceSnapshot {
        CanonicalSourceSnapshot {
            threads: thread_ids
                .iter()
                .map(|thread_id| canonical_thread(home, thread_id))
                .collect(),
        }
    }

    fn inventory(
        home: &str,
        thread_ids: &[&str],
        coverage: ObservationCoverage,
    ) -> DesktopCatalogInventoryReport {
        DesktopCatalogInventoryReport {
            codex_home_identity: home.to_string(),
            observed_thread_ids: thread_ids.iter().map(|value| value.to_string()).collect(),
            coverage,
            diagnostics: Vec::new(),
        }
    }

    fn report_with_inventory(inventory: DesktopCatalogInventoryReport) -> ReconcileReport {
        ReconcileReport {
            desktop_catalog_inventories: vec![inventory],
            ..ReconcileReport::default()
        }
    }

    fn stale_assessment(home: &str, thread_id: &str) -> DesktopProjectionObservation {
        DesktopProjectionObservation {
            thread_key: CodexThreadKey::new(home, thread_id),
            assessment: DesktopProjectionAssessment {
                state: DesktopProjectionState::DesktopStaleOrphan,
                canonical_ingest_allowed: false,
                evidence: vec!["tombstone".to_string()],
            },
        }
    }

    #[test]
    fn canonical_present_complete_catalog_hit_is_present() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory(
            "home-1",
            &["thread-1"],
            ObservationCoverage::Complete,
        ));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].state, SurfaceProjectionState::Present);
    }

    #[test]
    fn canonical_present_complete_catalog_miss_is_absent() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory(
            "home-1",
            &["different-thread"],
            ObservationCoverage::Complete,
        ));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].state, SurfaceProjectionState::Absent);
    }

    #[test]
    fn canonical_present_failed_catalog_miss_is_unknown() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Failed));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].state, SurfaceProjectionState::Unknown);
    }

    #[test]
    fn canonical_present_explicit_not_observed_is_unknown() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report =
            report_with_inventory(inventory("home-1", &[], ObservationCoverage::NotObserved));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].state, SurfaceProjectionState::Unknown);
    }

    #[test]
    fn canonical_present_bounded_or_partial_catalog_miss_is_unknown() {
        let canonical = canonical("home-1", &["thread-1"]);

        for coverage in [ObservationCoverage::Bounded, ObservationCoverage::Partial] {
            let report = report_with_inventory(inventory("home-1", &[], coverage));
            let observations = desktop_projection_updates(&report, &canonical, 1_000);

            assert_eq!(observations.len(), 1);
            assert_eq!(observations[0].coverage, coverage);
            assert_eq!(observations[0].state, SurfaceProjectionState::Unknown);
        }
    }

    #[test]
    fn canonical_present_without_matching_home_report_has_no_observation() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = ReconcileReport::default();

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert!(observations.is_empty());
    }

    #[test]
    fn different_codex_home_inventory_is_not_used_as_evidence() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory(
            "home-2",
            &["thread-1"],
            ObservationCoverage::Complete,
        ));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert!(observations.is_empty());
    }

    #[test]
    fn empty_complete_catalog_generates_absent_for_present_thread() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Complete));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations[0].state, SurfaceProjectionState::Absent);
        assert_eq!(observations[0].coverage, ObservationCoverage::Complete);
    }

    #[test]
    fn tombstoned_catalog_hit_remains_stale_pending() {
        let report = ReconcileReport {
            desktop_catalog_inventories: vec![inventory(
                "home-1",
                &["deleted-thread"],
                ObservationCoverage::Complete,
            )],
            desktop_projection_observations: vec![stale_assessment("home-1", "deleted-thread")],
            ..ReconcileReport::default()
        };

        let observations =
            desktop_projection_updates(&report, &CanonicalSourceSnapshot::default(), 1_000);

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

    #[test]
    fn catalog_miss_does_not_create_sidebar_projection() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Complete));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert!(observations
            .iter()
            .all(|value| value.key.projection_kind == SurfaceProjectionKind::Catalog));
    }

    #[test]
    fn catalog_miss_does_not_change_project_relation() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Complete));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert!(observations
            .iter()
            .all(|value| value.key.projection_kind != SurfaceProjectionKind::Project));
    }

    #[test]
    fn catalog_miss_does_not_change_canonical_identity() {
        let canonical = canonical("home-1", &["thread-1"]);
        let original = canonical.clone();
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Complete));

        let _ = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(canonical, original);
    }

    #[test]
    fn multiple_threads_mixed_hit_miss_are_deterministic() {
        let canonical = canonical("home-1", &["thread-b", "thread-a"]);
        let report = report_with_inventory(inventory(
            "home-1",
            &["thread-b"],
            ObservationCoverage::Complete,
        ));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(
            observations
                .iter()
                .map(|value| (value.key.thread_key.thread_id.as_str(), value.state))
                .collect::<Vec<_>>(),
            vec![
                ("thread-a", SurfaceProjectionState::Absent),
                ("thread-b", SurfaceProjectionState::Present),
            ]
        );
    }

    #[test]
    fn insertion_order_does_not_change_output() {
        let first_canonical = canonical("home-1", &["thread-b", "thread-a"]);
        let second_canonical = canonical("home-1", &["thread-a", "thread-b"]);
        let first_report = report_with_inventory(inventory(
            "home-1",
            &["thread-b", "thread-a"],
            ObservationCoverage::Complete,
        ));
        let second_report = report_with_inventory(inventory(
            "home-1",
            &["thread-a", "thread-b"],
            ObservationCoverage::Complete,
        ));

        assert_eq!(
            desktop_projection_updates(&first_report, &first_canonical, 1_000),
            desktop_projection_updates(&second_report, &second_canonical, 1_000)
        );
    }

    #[test]
    fn production_join_uses_inventory_coverage_not_single_element_complete_shortcut() {
        let canonical = canonical("home-1", &["thread-1"]);
        let report = report_with_inventory(inventory("home-1", &[], ObservationCoverage::Failed));

        let observations = desktop_projection_updates(&report, &canonical, 1_000);

        assert_eq!(observations[0].coverage, ObservationCoverage::Failed);
        assert_eq!(observations[0].state, SurfaceProjectionState::Unknown);
    }
}

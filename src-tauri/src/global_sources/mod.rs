pub(crate) mod app_server_live;
pub(crate) mod diagnostics;
pub(crate) mod runtime;
pub(crate) mod snapshot;

use crate::codex::home::{resolve_default_codex_home, resolve_workspace_codex_home};
use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchService;
use crate::shared::global_sources_core::rollout_watcher::{
    RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy,
};
use crate::shared::global_sources_core::runtime_config::{
    discover_runtime_codex_homes, GlobalSourceRuntimePaths,
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
            }),
        )
        .map_err(|error| error.to_string())?;
    let config = RolloutWatcherConfig {
        homes: homes.clone(),
        checkpoint_path: paths.checkpoint_path.clone(),
        retry: WatcherRetryPolicy {
            max_attempts: 5,
            initial_backoff_ms: 50,
        },
        fresh_window_ms: 5_000,
        settled_after_ms: 2_000,
        reconciliation_interval_ms: 500,
    };
    let watcher = RolloutTailWatcher::new(config);
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
                .run_until(shutdown, commands, |event, registry| {
                    let _ = journal.record_watch_event(&event, registry);
                    if let Some(snapshot) = snapshot_app
                        .state::<AppState>()
                        .global_rollout_runtime
                        .publish_canonical_snapshot(
                            registry.snapshot(),
                            chrono::Utc::now().timestamp_millis(),
                        )
                    {
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

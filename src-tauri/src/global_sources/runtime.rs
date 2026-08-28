use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, RwLock};

use crate::global_sources::app_server_live::normalize_app_server_live;
use crate::global_sources::snapshot::GlobalSourceSnapshot;
use crate::shared::global_sources_core::deletion_tombstone::DeletionTombstone;
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchCommand;
use crate::shared::global_sources_core::rollout_watcher::DeletionReconciliationReport;
use crate::shared::global_sources_core::source_envelope::CodexHomeIdentity;
use crate::shared::global_sources_core::source_registry::CanonicalSourceSnapshot;
use crate::shared::global_sources_core::source_registry::SourceLaneUpdate;
use serde_json::Value;

#[derive(Default)]
pub(crate) struct GlobalRolloutRuntime {
    worker: Mutex<Option<RuntimeWorker>>,
    live_sources: RwLock<LiveSourceConfig>,
    snapshot: RwLock<GlobalSourceSnapshot>,
}

#[derive(Default)]
struct LiveSourceConfig {
    source_instance_id: String,
    workspace_homes: HashMap<String, CodexHomeIdentity>,
}

struct RuntimeWorker {
    shutdown: tokio::sync::watch::Sender<bool>,
    commands: tokio::sync::mpsc::UnboundedSender<RolloutWatchCommand>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl GlobalRolloutRuntime {
    pub(crate) fn publish_canonical_snapshot(
        &self,
        canonical: CanonicalSourceSnapshot,
        generated_at_ms: i64,
    ) -> Option<GlobalSourceSnapshot> {
        let workspace_codex_home_identities = self
            .live_sources
            .read()
            .expect("global live source config lock")
            .workspace_homes
            .iter()
            .map(|(workspace_id, home)| (workspace_id.clone(), home.identity.clone()))
            .collect::<HashMap<_, _>>();
        let mut current = self.snapshot.write().expect("global source snapshot lock");
        if current.workspace_codex_home_identities == workspace_codex_home_identities
            && current.threads == canonical.threads
        {
            return None;
        }
        let next = GlobalSourceSnapshot {
            revision: current.revision.saturating_add(1),
            generated_at_ms,
            workspace_codex_home_identities,
            threads: canonical.threads,
        };
        *current = next.clone();
        Some(next)
    }

    pub(crate) fn snapshot(&self) -> GlobalSourceSnapshot {
        self.snapshot
            .read()
            .expect("global source snapshot lock")
            .clone()
    }

    pub(crate) fn start<F, Fut>(&self, worker: F) -> bool
    where
        F: FnOnce(
            tokio::sync::watch::Receiver<bool>,
            tokio::sync::mpsc::UnboundedReceiver<RolloutWatchCommand>,
        ) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut current = self.worker.lock().expect("global rollout runtime lock");
        if current.is_some() {
            return false;
        }
        let (shutdown, receiver) = tokio::sync::watch::channel(false);
        let (commands, command_receiver) = tokio::sync::mpsc::unbounded_channel();
        let task = tauri::async_runtime::spawn(worker(receiver, command_receiver));
        *current = Some(RuntimeWorker {
            shutdown,
            commands,
            task,
        });
        true
    }

    pub(crate) fn ingest_live(&self, update: SourceLaneUpdate) -> bool {
        self.worker
            .lock()
            .expect("global rollout runtime lock")
            .as_ref()
            .is_some_and(|worker| {
                worker
                    .commands
                    .send(RolloutWatchCommand::IngestLive(update))
                    .is_ok()
            })
    }

    pub(crate) async fn reconcile_confirmed_deletion(
        &self,
        workspace_id: &str,
        root_thread_id: &str,
        descendant_thread_ids: impl IntoIterator<Item = String>,
        monitor_delete_operation_id: &str,
        deleted_at_ms: i64,
    ) -> Result<DeletionReconciliationReport, String> {
        uuid::Uuid::parse_str(monitor_delete_operation_id)
            .map_err(|error| format!("invalid monitor delete operation id: {error}"))?;
        let home_identity = self
            .live_sources
            .read()
            .expect("global live source config lock")
            .workspace_homes
            .get(workspace_id)
            .map(|home| home.identity.clone())
            .ok_or_else(|| {
                format!("no canonical CODEX_HOME identity for workspace {workspace_id}")
            })?;
        let root_thread_key = CodexThreadKey::new(&home_identity, root_thread_id);
        let mut seen = std::collections::HashSet::new();
        let mut descendant_thread_keys = descendant_thread_ids
            .into_iter()
            .filter(|thread_id| thread_id != root_thread_id && seen.insert(thread_id.clone()))
            .map(|thread_id| CodexThreadKey::new(&home_identity, thread_id))
            .collect::<Vec<_>>();
        descendant_thread_keys.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
        let tombstone = DeletionTombstone::confirmed(
            monitor_delete_operation_id,
            root_thread_key,
            descendant_thread_keys,
            deleted_at_ms,
        );
        let (response, receiver) = tokio::sync::oneshot::channel();
        let commands = self
            .worker
            .lock()
            .expect("global rollout runtime lock")
            .as_ref()
            .map(|worker| worker.commands.clone())
            .ok_or_else(|| "global rollout watch service is not running".to_string())?;
        commands
            .send(RolloutWatchCommand::ReconcileDeletion {
                tombstone,
                response,
            })
            .map_err(|_| {
                "global rollout watch service stopped before deletion reconciliation".to_string()
            })?;
        receiver.await.map_err(|_| {
            "global rollout watch service dropped deletion reconciliation response".to_string()
        })?
    }

    pub(crate) fn configure_live_sources(
        &self,
        source_instance_id: impl Into<String>,
        workspace_homes: impl IntoIterator<Item = (String, CodexHomeIdentity)>,
    ) {
        *self
            .live_sources
            .write()
            .expect("global live source config lock") = LiveSourceConfig {
            source_instance_id: source_instance_id.into(),
            workspace_homes: workspace_homes.into_iter().collect(),
        };
    }

    pub(crate) fn ingest_app_server_event(
        &self,
        workspace_id: &str,
        message: &Value,
        observed_timestamp_ms: i64,
    ) -> bool {
        let config = self
            .live_sources
            .read()
            .expect("global live source config lock");
        let Some(home) = config.workspace_homes.get(workspace_id) else {
            return false;
        };
        if message.get("method").and_then(Value::as_str) == Some("thread/deleted") {
            let thread_id = message.get("params").and_then(|params| {
                params
                    .get("threadId")
                    .or_else(|| params.get("thread_id"))
                    .and_then(Value::as_str)
                    .or_else(|| {
                        params
                            .get("thread")
                            .and_then(|thread| thread.get("id"))
                            .and_then(Value::as_str)
                    })
            });
            let Some(thread_id) = thread_id else {
                return false;
            };
            let key = CodexThreadKey::new(&home.identity, thread_id);
            drop(config);
            return self
                .worker
                .lock()
                .expect("global rollout runtime lock")
                .as_ref()
                .is_some_and(|worker| {
                    worker
                        .commands
                        .send(RolloutWatchCommand::ConfirmThreadDeleted(key))
                        .is_ok()
                });
        }
        let Some(update) = normalize_app_server_live(
            &config.source_instance_id,
            workspace_id,
            home,
            message,
            observed_timestamp_ms,
        ) else {
            return false;
        };
        drop(config);
        self.ingest_live(update)
    }

    pub(crate) async fn shutdown(&self) {
        let worker = self
            .worker
            .lock()
            .expect("global rollout runtime lock")
            .take();
        let Some(worker) = worker else {
            return;
        };
        let _ = worker.shutdown.send(true);
        let _ = worker.task.await;
    }

    pub(crate) fn is_running(&self) -> bool {
        self.worker
            .lock()
            .expect("global rollout runtime lock")
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global_sources::app_server_live::normalize_app_server_live;
    use crate::shared::global_sources_core::source_envelope::CodexHomeIdentity;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn starts_exactly_one_worker_and_shutdown_joins_it() {
        let runtime = GlobalRolloutRuntime::default();
        let starts = Arc::new(AtomicUsize::new(0));
        let exits = Arc::new(AtomicUsize::new(0));
        let starts_for_worker = starts.clone();
        let exits_for_worker = exits.clone();
        let (received_tx, received_rx) = tokio::sync::oneshot::channel();

        assert!(runtime.start(move |mut shutdown, mut commands| async move {
            starts_for_worker.fetch_add(1, Ordering::SeqCst);
            tokio::select! {
                _ = shutdown.changed() => {}
                command = commands.recv() => {
                    let update = match command {
                        Some(RolloutWatchCommand::IngestLive(update)) => update,
                        Some(_) => panic!("unexpected command"),
                        None => panic!("command channel closed"),
                    };
                    let _ = received_tx.send(update.thread_key.thread_id);
                }
            }
            exits_for_worker.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(!runtime.start(|_, _| async {}));
        let update = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &CodexHomeIdentity {
                normalized_path: "C:\\fixture\\codex-home".to_string(),
                identity: "codex-home:fixture".to_string(),
            },
            &json!({
                "method": "turn/started",
                "params": { "threadId": "thread-live", "turn": { "id": "turn-live" } }
            }),
            1_000,
        )
        .expect("live update");
        assert!(runtime.ingest_live(update));
        assert_eq!(received_rx.await.expect("received command"), "thread-live");
        tokio::task::yield_now().await;
        assert_eq!(starts.load(Ordering::SeqCst), 1);

        runtime.shutdown().await;

        assert_eq!(exits.load(Ordering::SeqCst), 1);
        assert!(!runtime.is_running());
    }

    #[tokio::test]
    async fn shutdown_before_start_is_safe_and_runtime_can_start_afterward() {
        let runtime = GlobalRolloutRuntime::default();

        runtime.shutdown().await;
        assert!(runtime.start(|mut shutdown, _| async move {
            let _ = shutdown.changed().await;
        }));
        runtime.shutdown().await;

        assert!(!runtime.is_running());
    }

    #[tokio::test]
    async fn configured_workspace_routes_confirmed_event_and_ignores_unknown_workspace() {
        let runtime = GlobalRolloutRuntime::default();
        let (received_tx, received_rx) = tokio::sync::oneshot::channel();
        assert!(runtime.start(move |_, mut commands| async move {
            let update = match commands.recv().await {
                Some(RolloutWatchCommand::IngestLive(update)) => update,
                Some(_) => panic!("unexpected command"),
                None => panic!("command channel closed"),
            };
            let _ = received_tx.send(update.thread_key);
        }));
        runtime.configure_live_sources(
            "monitor-process-1",
            [(
                "workspace-1".to_string(),
                CodexHomeIdentity {
                    normalized_path: "C:\\fixture\\codex-home".to_string(),
                    identity: "codex-home:fixture".to_string(),
                },
            )],
        );
        let message = json!({
            "method": "turn/started",
            "params": { "threadId": "thread-1", "turn": { "id": "turn-1" } }
        });

        assert!(!runtime.ingest_app_server_event("workspace-unknown", &message, 1_000));
        assert!(runtime.ingest_app_server_event("workspace-1", &message, 1_000));
        let key = received_rx.await.expect("live update");
        assert_eq!(key.codex_home_identity, "codex-home:fixture");
        assert_eq!(key.thread_id, "thread-1");
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn thread_deleted_notification_routes_exact_identity_as_confirmation_evidence() {
        let runtime = GlobalRolloutRuntime::default();
        let (received_tx, received_rx) = tokio::sync::oneshot::channel();
        assert!(runtime.start(move |_, mut commands| async move {
            let command = commands.recv().await.expect("confirmation command");
            let RolloutWatchCommand::ConfirmThreadDeleted(key) = command else {
                panic!("expected deletion confirmation command");
            };
            let _ = received_tx.send(key);
        }));
        runtime.configure_live_sources(
            "monitor-process-1",
            [(
                "workspace-1".to_string(),
                CodexHomeIdentity {
                    normalized_path: "C:\\fixture\\codex-home".to_string(),
                    identity: "codex-home:fixture".to_string(),
                },
            )],
        );
        assert!(runtime.ingest_app_server_event(
            "workspace-1",
            &json!({
                "method": "thread/deleted",
                "params": { "thread": { "id": "thread-child" } }
            }),
            1_000,
        ));
        assert_eq!(
            received_rx.await.expect("confirmation identity"),
            CodexThreadKey::new("codex-home:fixture", "thread-child")
        );
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn configured_workspace_submits_exact_confirmed_deletion_identity() {
        use crate::shared::global_sources_core::deletion_tombstone::DeletionReconciliationState;
        use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
        use crate::shared::global_sources_core::rollout_watcher::DeletionReconciliationReport;

        let runtime = GlobalRolloutRuntime::default();
        assert!(runtime.start(move |_, mut commands| async move {
            let command = commands.recv().await.expect("deletion command");
            let RolloutWatchCommand::ReconcileDeletion {
                tombstone,
                response,
            } = command
            else {
                panic!("expected deletion command");
            };
            assert_eq!(
                tombstone.root_thread_key,
                CodexThreadKey::new("codex-home:fixture", "root")
            );
            assert_eq!(
                tombstone.descendant_thread_keys,
                vec![CodexThreadKey::new("codex-home:fixture", "child")]
            );
            assert_eq!(
                tombstone.monitor_delete_operation_id,
                "7fa286f5-d496-4345-9280-0daf06cf6e85"
            );
            let _ = response.send(Ok(DeletionReconciliationReport {
                monitor_delete_operation_id: tombstone.monitor_delete_operation_id,
                root_thread_id: "root".to_string(),
                descendant_thread_ids: vec!["child".to_string()],
                tombstone_persisted: true,
                registry_retirement_count: 2,
                watcher_source_retirement_count: 2,
                checkpoint_rewritten: true,
                reconciliation_state: Some(DeletionReconciliationState::Completed),
                desktop_reconciliation: Some(
                    crate::shared::global_sources_core::deletion_tombstone::DesktopReconciliationState::RefreshPending,
                ),
                snapshot_publication_revision: Some(4),
            }));
        }));
        runtime.configure_live_sources(
            "monitor-process-1",
            [(
                "workspace-1".to_string(),
                CodexHomeIdentity {
                    normalized_path: "C:\\fixture\\codex-home".to_string(),
                    identity: "codex-home:fixture".to_string(),
                },
            )],
        );

        let report = runtime
            .reconcile_confirmed_deletion(
                "workspace-1",
                "root",
                ["child".to_string()],
                "7fa286f5-d496-4345-9280-0daf06cf6e85",
                1_000,
            )
            .await
            .expect("accepted deletion");
        assert_eq!(report.registry_retirement_count, 2);
        runtime.shutdown().await;
    }

    #[test]
    fn canonical_snapshot_cache_is_revisioned_immutable_and_suppresses_unchanged_payloads() {
        use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};
        use crate::shared::global_sources_core::source_envelope::{
            FreshnessEvidence, FreshnessState, SourceKind, SourceTemporalClass,
        };
        use crate::shared::global_sources_core::source_registry::{
            ExternalLifecycle, SourceAuthorityRegistry, SourceLaneUpdate,
        };

        let runtime = GlobalRolloutRuntime::default();
        runtime.configure_live_sources(
            "monitor-process-1",
            [(
                "workspace-1".to_string(),
                CodexHomeIdentity {
                    normalized_path: "C:\\fixture\\codex-home".to_string(),
                    identity: "codex-home:fixture".to_string(),
                },
            )],
        );
        let mut registry = SourceAuthorityRegistry::default();
        let thread_key = CodexThreadKey::new("codex-home:fixture", "thread-1");
        registry
            .ingest(SourceLaneUpdate {
                observation_id: "rollout-start".to_string(),
                thread_key: thread_key.clone(),
                turn_key: Some(CodexTurnKey::new(thread_key, "turn-1")),
                source_kind: SourceKind::CodexCliRollout,
                temporal_class: SourceTemporalClass::NearLive,
                source_instance_id: "rollout-tail:fixture".to_string(),
                source_generation: "generation-1".to_string(),
                source_timestamp_ms: Some(1_000),
                observed_timestamp_ms: 1_025,
                freshness: FreshnessEvidence {
                    state: FreshnessState::Fresh,
                    last_complete_record_observed_at_ms: Some(1_025),
                    reason: "fixture".to_string(),
                },
                lifecycle: Some(ExternalLifecycle::Running),
                observed_model: None,
                token_snapshot: None,
            })
            .expect("registry ingest");

        let first = runtime
            .publish_canonical_snapshot(registry.snapshot(), 1_100)
            .expect("first publication");
        assert_eq!(first.revision, 1);
        assert_eq!(first.generated_at_ms, 1_100);
        assert_eq!(
            first.workspace_codex_home_identities.get("workspace-1"),
            Some(&"codex-home:fixture".to_string())
        );
        assert_eq!(first.threads.len(), 1);
        assert!(runtime
            .publish_canonical_snapshot(registry.snapshot(), 1_200)
            .is_none());

        let read = runtime.snapshot();
        assert_eq!(read, first);
        let mut detached = read;
        detached.threads.clear();
        assert_eq!(runtime.snapshot().threads.len(), 1);
    }
}

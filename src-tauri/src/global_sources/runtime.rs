use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, RwLock};

use crate::global_sources::app_server_live::normalize_app_server_live;
use crate::global_sources::snapshot::GlobalSourceSnapshot;
use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchCommand;
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

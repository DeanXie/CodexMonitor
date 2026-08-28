use super::deletion_tombstone::{
    DeletionReconciliationState, DeletionTombstone, DeletionTombstoneDocument,
    DeletionTombstoneStore, DesktopReconciliationState,
};
use super::rollout_checkpoint::{
    RolloutAdapterCheckpoint, RolloutCheckpointStore, RolloutSourceCheckpoint,
    RolloutWatcherCheckpoint,
};
use super::rollout_discovery::{discover_rollout_sources, CodexHomeSource};
use super::rollout_identity::{identity_from_session_meta, CodexTurnKey};
use super::rollout_record::{ParsedRolloutRecord, RolloutRecordParser};
use super::rollout_tail::{read_rollout_delta, RolloutDelta, RolloutTailState};
use super::source_envelope::{
    ConfidenceEvidence, EvidenceConfidence, FreshnessEvidence, FreshnessState, ProvenanceEvidence,
    SchemaEvidence, SourceCursor, SourceEnvelope, SourceFileIdentity, SourceKind,
    SourceTemporalClass, SourceTimestampKind, SourceTimestamps,
};
use super::source_registry::{
    ExternalLifecycle, SourceAuthorityRegistry, SourceLaneUpdate, SourceRegistryBatchItem,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WatcherRetryPolicy {
    pub max_attempts: usize,
    pub initial_backoff_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RolloutWatcherConfig {
    pub homes: Vec<CodexHomeSource>,
    pub checkpoint_path: PathBuf,
    pub deletion_tombstones_path: PathBuf,
    pub retry: WatcherRetryPolicy,
    pub fresh_window_ms: i64,
    pub settled_after_ms: i64,
    pub reconciliation_interval_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceHealth {
    pub source_path: PathBuf,
    pub last_complete_record_observed_at_ms: Option<i64>,
    pub last_successful_read_at_ms: Option<i64>,
    pub last_filesystem_signal_at_ms: Option<i64>,
    pub latest_source_timestamp_ms: Option<i64>,
    pub lag_ms: Option<i64>,
    pub consecutive_read_failures: u32,
    pub last_error: Option<String>,
    pub freshness: FreshnessEvidence,
}

impl SourceHealth {
    fn unknown(path: PathBuf) -> Self {
        Self {
            source_path: path,
            last_complete_record_observed_at_ms: None,
            last_successful_read_at_ms: None,
            last_filesystem_signal_at_ms: None,
            latest_source_timestamp_ms: None,
            lag_ms: None,
            consecutive_read_failures: 0,
            last_error: None,
            freshness: FreshnessEvidence {
                state: FreshnessState::Unknown,
                last_complete_record_observed_at_ms: None,
                reason: "no complete rollout record observed".to_string(),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct WatchedSource {
    codex_home: super::source_envelope::CodexHomeIdentity,
    path: PathBuf,
    source_file: SourceFileIdentity,
    tail: RolloutTailState,
    parser: RolloutRecordParser,
    adapter: RolloutAdapterCheckpoint,
    health: SourceHealth,
    published_freshness: Option<FreshnessState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceReadFailure {
    pub source_path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReconcileReport {
    pub envelopes: Vec<SourceEnvelope<Value>>,
    pub discovered_sources: usize,
    pub processed_sources: usize,
    pub read_failures: Vec<SourceReadFailure>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DeletionReconciliationReport {
    pub monitor_delete_operation_id: String,
    pub root_thread_id: String,
    pub descendant_thread_ids: Vec<String>,
    pub tombstone_persisted: bool,
    pub registry_retirement_count: usize,
    pub watcher_source_retirement_count: usize,
    pub checkpoint_rewritten: bool,
    pub reconciliation_state: Option<DeletionReconciliationState>,
    pub desktop_reconciliation: Option<DesktopReconciliationState>,
    pub snapshot_publication_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeletionReconciliationFailure {
    pub monitor_delete_operation_id: String,
    pub root_thread_id: String,
    pub descendant_thread_ids: Vec<String>,
    pub tombstone_persisted: bool,
    pub message: String,
}

impl std::fmt::Display for DeletionReconciliationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DeletionReconciliationFailure {}

pub(crate) trait RolloutDeltaReader: Clone {
    fn read_delta(
        &self,
        path: &Path,
        source_file: &mut SourceFileIdentity,
        state: &mut RolloutTailState,
        observed_timestamp_ms: i64,
    ) -> io::Result<RolloutDelta>;
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FsRolloutDeltaReader;

impl RolloutDeltaReader for FsRolloutDeltaReader {
    fn read_delta(
        &self,
        path: &Path,
        source_file: &mut SourceFileIdentity,
        state: &mut RolloutTailState,
        observed_timestamp_ms: i64,
    ) -> io::Result<RolloutDelta> {
        read_rollout_delta(path, source_file, state, observed_timestamp_ms)
    }
}

pub(crate) struct RolloutTailWatcher<R = FsRolloutDeltaReader> {
    config: RolloutWatcherConfig,
    checkpoint_store: RolloutCheckpointStore,
    tombstone_store: DeletionTombstoneStore,
    tombstones: DeletionTombstoneDocument,
    retired_source_files: HashSet<SourceFileIdentity>,
    restored: HashMap<String, RolloutSourceCheckpoint>,
    sources: HashMap<String, WatchedSource>,
    pending_signal_times: HashMap<String, i64>,
    pending_thread_deleted_confirmations: HashSet<super::rollout_identity::CodexThreadKey>,
    registry: SourceAuthorityRegistry,
    reader: R,
    startup_error: Option<(io::ErrorKind, String)>,
    startup_checkpoint_dirty: bool,
    startup_tombstones_dirty: bool,
    pending_checkpoint_rewrite: bool,
}

impl RolloutTailWatcher<FsRolloutDeltaReader> {
    pub(crate) fn new(config: RolloutWatcherConfig) -> Self {
        Self::with_reader(config, FsRolloutDeltaReader)
    }
}

impl<R: RolloutDeltaReader> RolloutTailWatcher<R> {
    pub(crate) fn with_reader(config: RolloutWatcherConfig, reader: R) -> Self {
        let tombstone_store = DeletionTombstoneStore::new(config.deletion_tombstones_path.clone());
        let (mut tombstones, tombstone_error) = match tombstone_store.load() {
            Ok(document) => (document, None),
            Err(error) => (
                DeletionTombstoneDocument::default(),
                Some((error.kind(), error.to_string())),
            ),
        };
        let tombstoned_keys = tombstones
            .operations
            .iter()
            .flat_map(|operation| operation.thread_keys().cloned())
            .collect::<HashSet<_>>();
        let mut retired_source_files = tombstones
            .operations
            .iter()
            .flat_map(|operation| operation.retired_source_files.iter().cloned())
            .collect::<HashSet<_>>();
        let checkpoint_store = RolloutCheckpointStore::new(config.checkpoint_path.clone());
        let (checkpoint, checkpoint_error) = match checkpoint_store.load() {
            Ok(checkpoint) => (checkpoint, None),
            Err(error) => (
                RolloutWatcherCheckpoint::default(),
                Some((error.kind(), error.to_string())),
            ),
        };
        let mut startup_checkpoint_dirty = false;
        let mut startup_tombstones_dirty = false;
        let restored = checkpoint
            .sources
            .into_iter()
            .filter_map(|checkpoint| {
                let retired_by_owner = checkpoint
                    .adapter
                    .thread_key
                    .as_ref()
                    .is_some_and(|key| tombstoned_keys.contains(key));
                let retired_by_file = retired_source_files.contains(&checkpoint.source_file);
                if retired_by_owner || retired_by_file {
                    startup_checkpoint_dirty = true;
                    retired_source_files.insert(checkpoint.source_file.clone());
                    if let Some(owner) = checkpoint.adapter.thread_key.as_ref() {
                        if let Some(operation) = tombstones
                            .operations
                            .iter_mut()
                            .find(|operation| operation.contains_thread_key(owner))
                        {
                            startup_tombstones_dirty |= operation
                                .record_retired_source_file(checkpoint.source_file.clone());
                        }
                    }
                    return None;
                }
                Some((
                    source_key(
                        &checkpoint.codex_home_identity,
                        &checkpoint.source_file.normalized_path,
                    ),
                    checkpoint,
                ))
            })
            .collect();
        let mut registry = SourceAuthorityRegistry::default();
        registry.retire_threads(tombstoned_keys);
        let pending_checkpoint_rewrite = startup_checkpoint_dirty
            || tombstones.operations.iter().any(|operation| {
                operation.reconciliation_state == DeletionReconciliationState::Pending
            });
        Self {
            config,
            checkpoint_store,
            tombstone_store,
            tombstones,
            retired_source_files,
            restored,
            sources: HashMap::new(),
            pending_signal_times: HashMap::new(),
            pending_thread_deleted_confirmations: HashSet::new(),
            registry,
            reader,
            startup_error: tombstone_error.or(checkpoint_error),
            startup_checkpoint_dirty,
            startup_tombstones_dirty,
            pending_checkpoint_rewrite,
        }
    }

    pub(crate) fn reconcile(&mut self, observed_timestamp_ms: i64) -> io::Result<ReconcileReport> {
        self.reconcile_internal(observed_timestamp_ms, false)
    }

    pub(crate) fn reconcile_now(&mut self) -> io::Result<ReconcileReport> {
        self.reconcile_internal(chrono::Utc::now().timestamp_millis(), true)
    }

    fn reconcile_internal(
        &mut self,
        observed_timestamp_ms: i64,
        observe_after_read: bool,
    ) -> io::Result<ReconcileReport> {
        if let Some((kind, message)) = &self.startup_error {
            return Err(io::Error::new(*kind, message.clone()));
        }
        let discovered = discover_rollout_sources(&self.config.homes)?;
        let mut report = ReconcileReport::default();
        for source in discovered {
            if self.retired_source_files.contains(&source.file_identity) {
                continue;
            }
            let key = source_key(
                &source.codex_home.identity,
                &source.file_identity.normalized_path,
            );
            if self.sources.contains_key(&key) {
                continue;
            }
            let restored = self.restored.remove(&key).filter(|checkpoint| {
                checkpoint.source_file.filesystem_id == source.file_identity.filesystem_id
            });
            let watched = match restored {
                Some(checkpoint) => WatchedSource {
                    codex_home: source.codex_home,
                    path: source.path.clone(),
                    source_file: checkpoint.source_file,
                    tail: RolloutTailState::from_checkpoint(checkpoint.tail),
                    parser: RolloutRecordParser::default(),
                    adapter: checkpoint.adapter.clone(),
                    health: SourceHealth {
                        source_path: source.path,
                        last_complete_record_observed_at_ms: checkpoint
                            .last_complete_record_observed_at_ms,
                        last_successful_read_at_ms: checkpoint.last_successful_read_at_ms,
                        last_filesystem_signal_at_ms: checkpoint.last_filesystem_signal_at_ms,
                        latest_source_timestamp_ms: checkpoint.adapter.source_timestamp_ms,
                        lag_ms: None,
                        consecutive_read_failures: 0,
                        last_error: None,
                        freshness: FreshnessEvidence {
                            state: FreshnessState::Unknown,
                            last_complete_record_observed_at_ms: checkpoint
                                .last_complete_record_observed_at_ms,
                            reason: "restored checkpoint awaiting reconciliation".to_string(),
                        },
                    },
                    published_freshness: None,
                },
                None => WatchedSource {
                    codex_home: source.codex_home,
                    path: source.path.clone(),
                    tail: RolloutTailState::new(source.file_identity.generation.clone()),
                    source_file: source.file_identity,
                    parser: RolloutRecordParser::default(),
                    adapter: RolloutAdapterCheckpoint::default(),
                    health: SourceHealth::unknown(source.path),
                    published_freshness: None,
                },
            };
            let mut watched = watched;
            if let Some(signal) = self.pending_signal_times.remove(&path_key(&watched.path)) {
                watched.health.last_filesystem_signal_at_ms = Some(signal);
            }
            self.sources.insert(key, watched);
            report.discovered_sources += 1;
        }

        let keys = self.sources.keys().cloned().collect::<Vec<_>>();
        let mut checkpoint_dirty = report.discovered_sources > 0
            || self.startup_checkpoint_dirty
            || self.pending_checkpoint_rewrite;
        for key in keys {
            let Some(mut source) = self.sources.remove(&key) else {
                continue;
            };
            let before_cursor = (
                source.source_file.generation.clone(),
                source.tail.checkpoint().committed_byte_offset,
                source.tail.checkpoint().record_ordinal,
            );
            match self.process_source(&mut source, observed_timestamp_ms, observe_after_read) {
                Ok(envelopes) => {
                    if source
                        .adapter
                        .thread_key
                        .as_ref()
                        .is_some_and(|key| self.registry.is_tombstoned(key))
                    {
                        self.record_retired_source(&source);
                        self.tombstone_store.save(&self.tombstones)?;
                        checkpoint_dirty = true;
                        continue;
                    }
                    let after_cursor = (
                        source.source_file.generation.clone(),
                        source.tail.checkpoint().committed_byte_offset,
                        source.tail.checkpoint().record_ordinal,
                    );
                    if after_cursor != before_cursor {
                        report.processed_sources += 1;
                        checkpoint_dirty = true;
                    }
                    report.envelopes.extend(envelopes);
                }
                Err(error) => {
                    source.health.consecutive_read_failures =
                        source.health.consecutive_read_failures.saturating_add(1);
                    source.health.last_error = Some(error.to_string());
                    report.read_failures.push(SourceReadFailure {
                        source_path: source.path.clone(),
                        message: error.to_string(),
                    });
                }
            }
            refresh_source_health(
                &mut source,
                observed_timestamp_ms,
                self.config.fresh_window_ms,
                self.config.settled_after_ms,
            );
            self.publish_health_if_changed(&mut source, observed_timestamp_ms)?;
            self.sources.insert(key, source);
        }

        if checkpoint_dirty {
            self.save_checkpoint()?;
            self.startup_checkpoint_dirty = false;
            self.pending_checkpoint_rewrite = false;
        }
        if self.startup_tombstones_dirty {
            self.tombstone_store.save(&self.tombstones)?;
            self.startup_tombstones_dirty = false;
        }
        if !self.pending_checkpoint_rewrite
            && self.tombstones.operations.iter().any(|operation| {
                operation.reconciliation_state == DeletionReconciliationState::Pending
            })
        {
            for operation in &mut self.tombstones.operations {
                if operation.reconciliation_state == DeletionReconciliationState::Pending {
                    operation.mark_local_reconciliation_completed();
                }
            }
            if let Err(error) = self.tombstone_store.save(&self.tombstones) {
                for operation in &mut self.tombstones.operations {
                    if operation.reconciliation_state == DeletionReconciliationState::Completed {
                        operation.reconciliation_state = DeletionReconciliationState::Pending;
                    }
                }
                return Err(error);
            }
        }
        Ok(report)
    }

    pub(crate) fn reconcile_deletion(
        &mut self,
        tombstone: DeletionTombstone,
    ) -> Result<DeletionReconciliationReport, DeletionReconciliationFailure> {
        let operation_id = tombstone.monitor_delete_operation_id.clone();
        let root_thread_id = tombstone.root_thread_key.thread_id.clone();
        let descendant_thread_ids = tombstone
            .descendant_thread_keys
            .iter()
            .map(|key| key.thread_id.clone())
            .collect::<Vec<_>>();
        let failure = |tombstone_persisted: bool, error: &dyn std::fmt::Display| {
            DeletionReconciliationFailure {
                monitor_delete_operation_id: operation_id.clone(),
                root_thread_id: root_thread_id.clone(),
                descendant_thread_ids: descendant_thread_ids.clone(),
                tombstone_persisted,
                message: error.to_string(),
            }
        };
        if let Some((kind, message)) = &self.startup_error {
            return Err(failure(false, &io::Error::new(*kind, message.clone())));
        }
        let original_tombstones = self.tombstones.clone();
        self.tombstones.upsert(tombstone);
        if let Some(operation) = self
            .tombstones
            .operations
            .iter_mut()
            .find(|operation| operation.monitor_delete_operation_id == operation_id)
        {
            let confirmations = self
                .pending_thread_deleted_confirmations
                .iter()
                .filter(|key| operation.contains_thread_key(key))
                .cloned()
                .collect::<Vec<_>>();
            for key in confirmations {
                operation.record_thread_deleted_confirmation(&key.thread_id);
                self.pending_thread_deleted_confirmations.remove(&key);
            }
        }
        // The pending tombstone must be durable before Registry or Watcher mutation.
        if let Err(error) = self.tombstone_store.save(&self.tombstones) {
            self.tombstones = original_tombstones;
            return Err(failure(false, &error));
        }

        let keys = self
            .tombstones
            .operations
            .iter()
            .find(|operation| operation.monitor_delete_operation_id == operation_id)
            .map(|operation| operation.thread_keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let registry_retirement_count = self.registry.retire_threads(keys.clone());
        self.pending_checkpoint_rewrite = true;
        let key_set = keys.into_iter().collect::<HashSet<_>>();
        let mut retired_sources = Vec::new();
        self.sources.retain(|_, source| {
            let should_retire = source
                .adapter
                .thread_key
                .as_ref()
                .is_some_and(|key| key_set.contains(key));
            if should_retire {
                retired_sources.push((source.path.clone(), source.source_file.clone()));
            }
            !should_retire
        });
        self.restored.retain(|_, source| {
            let should_retire = source
                .adapter
                .thread_key
                .as_ref()
                .is_some_and(|key| key_set.contains(key));
            if should_retire {
                retired_sources.push((
                    PathBuf::from(&source.source_file.normalized_path),
                    source.source_file.clone(),
                ));
            }
            !should_retire
        });
        let watcher_source_retirement_count = retired_sources.len();
        if let Some(operation) = self
            .tombstones
            .operations
            .iter_mut()
            .find(|operation| operation.monitor_delete_operation_id == operation_id)
        {
            for (path, source_file) in retired_sources {
                self.pending_signal_times.remove(&path_key(&path));
                self.retired_source_files.insert(source_file.clone());
                operation.record_retired_source_file(source_file);
            }
        }
        self.tombstone_store
            .save(&self.tombstones)
            .map_err(|error| failure(true, &error))?;
        self.save_checkpoint()
            .map_err(|error| failure(true, &error))?;
        self.pending_checkpoint_rewrite = false;
        let (reconciliation_state, desktop_reconciliation) = if let Some(operation) = self
            .tombstones
            .operations
            .iter_mut()
            .find(|operation| operation.monitor_delete_operation_id == operation_id)
        {
            operation.mark_local_reconciliation_completed();
            (
                Some(operation.reconciliation_state),
                Some(operation.desktop_reconciliation),
            )
        } else {
            (None, None)
        };
        if let Err(error) = self.tombstone_store.save(&self.tombstones) {
            if let Some(operation) = self
                .tombstones
                .operations
                .iter_mut()
                .find(|operation| operation.monitor_delete_operation_id == operation_id)
            {
                operation.reconciliation_state = DeletionReconciliationState::Pending;
            }
            return Err(failure(true, &error));
        }
        Ok(DeletionReconciliationReport {
            monitor_delete_operation_id: operation_id,
            root_thread_id,
            descendant_thread_ids,
            tombstone_persisted: true,
            registry_retirement_count,
            watcher_source_retirement_count,
            checkpoint_rewritten: true,
            reconciliation_state,
            desktop_reconciliation,
            snapshot_publication_revision: None,
        })
    }

    pub(crate) fn record_thread_deleted_confirmation(
        &mut self,
        key: &super::rollout_identity::CodexThreadKey,
    ) -> io::Result<bool> {
        let mut changed = false;
        for operation in &mut self.tombstones.operations {
            if operation.contains_thread_key(key) {
                changed |= operation.record_thread_deleted_confirmation(&key.thread_id);
            }
        }
        if changed {
            self.tombstone_store.save(&self.tombstones)?;
        } else if !self
            .tombstones
            .operations
            .iter()
            .any(|operation| operation.contains_thread_key(key))
        {
            self.pending_thread_deleted_confirmations
                .insert(key.clone());
        }
        Ok(changed)
    }

    fn record_retired_source(&mut self, source: &WatchedSource) {
        self.retired_source_files.insert(source.source_file.clone());
        if let Some(owner) = source.adapter.thread_key.as_ref() {
            if let Some(operation) = self
                .tombstones
                .operations
                .iter_mut()
                .find(|operation| operation.contains_thread_key(owner))
            {
                operation.record_retired_source_file(source.source_file.clone());
            }
        }
    }

    fn process_source(
        &mut self,
        source: &mut WatchedSource,
        observed_timestamp_ms: i64,
        observe_after_read: bool,
    ) -> io::Result<Vec<SourceEnvelope<Value>>> {
        let original_file = source.source_file.clone();
        let original_tail = source.tail.clone();
        let original_parser = source.parser.clone();
        let original_adapter = source.adapter.clone();
        let original_health = source.health.clone();

        let delta = match self.read_with_retry(source, observed_timestamp_ms) {
            Ok(delta) => delta,
            Err(error) => {
                source.source_file = original_file;
                source.tail = original_tail;
                return Err(error);
            }
        };
        let observed_timestamp_ms = if observe_after_read {
            chrono::Utc::now().timestamp_millis()
        } else {
            observed_timestamp_ms
        };
        source.health.last_successful_read_at_ms = Some(observed_timestamp_ms);
        source.health.consecutive_read_failures = 0;
        source.health.last_error = None;
        if delta.did_reset {
            source.parser = RolloutRecordParser::default();
            source.adapter = RolloutAdapterCheckpoint::default();
        }

        let mut envelopes = Vec::new();
        let mut registry_batch = Vec::new();
        for record in delta.records {
            let value: Value = match serde_json::from_str(&record.text) {
                Ok(value) => value,
                Err(error) => {
                    source.source_file = original_file;
                    source.tail = original_tail;
                    source.parser = original_parser;
                    source.adapter = original_adapter;
                    source.health = original_health;
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid complete rollout JSON: {error}"),
                    ));
                }
            };
            source.health.last_complete_record_observed_at_ms = Some(observed_timestamp_ms);
            let reliable_source_timestamp_ms = value
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|timestamp| chrono::DateTime::parse_from_rfc3339(timestamp).ok())
                .map(|timestamp| timestamp.timestamp_millis());
            if let Some(source_timestamp_ms) = reliable_source_timestamp_ms {
                source.health.lag_ms = Some(observed_timestamp_ms - source_timestamp_ms);
                source.health.latest_source_timestamp_ms = Some(source_timestamp_ms);
                source.adapter.source_timestamp_ms = Some(source_timestamp_ms);
            }
            let parsed = match source.parser.parse_value(&value) {
                Ok(parsed) => parsed,
                Err(error) => {
                    source.source_file = original_file;
                    source.tail = original_tail;
                    source.parser = original_parser;
                    source.adapter = original_adapter;
                    source.health = original_health;
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        error.to_string(),
                    ));
                }
            };
            let Some(parsed) = parsed else {
                continue;
            };
            let source_timestamp_ms = parsed.record_timestamp_ms();
            let observation_id = observation_id(
                &source.codex_home.identity,
                &source.source_file,
                record.byte_start,
                record.byte_end,
                &record.line_hash,
            );
            let field_update = apply_record(source, &parsed);
            let freshness = source_freshness(
                Some(source_timestamp_ms),
                observed_timestamp_ms,
                source.adapter.completed,
                self.config.fresh_window_ms,
                self.config.settled_after_ms,
                Some(observed_timestamp_ms),
            );
            if let Some(update) = field_update {
                let lane_update = SourceLaneUpdate {
                    observation_id: observation_id.clone(),
                    thread_key: update.thread_key,
                    turn_key: update.turn_key,
                    source_kind: SourceKind::CodexCliRollout,
                    temporal_class: SourceTemporalClass::NearLive,
                    source_instance_id: source_instance_id(&source.codex_home.identity),
                    source_generation: source.source_file.generation.clone(),
                    source_timestamp_ms: Some(source_timestamp_ms),
                    observed_timestamp_ms,
                    freshness: freshness.clone(),
                    lifecycle: update.lifecycle,
                    observed_model: update.observed_model,
                    token_snapshot: update.token_snapshot,
                };
                registry_batch.push(SourceRegistryBatchItem {
                    update: lane_update,
                    parent_thread_key: update.parent_thread_key,
                    agent_path: update.agent_path,
                });
            }
            source.health.lag_ms = Some(observed_timestamp_ms - source_timestamp_ms);
            source.adapter.source_timestamp_ms = Some(source_timestamp_ms);
            envelopes.push(SourceEnvelope {
                envelope_version: 1,
                observation_id,
                source_kind: SourceKind::CodexCliRollout,
                temporal_class: SourceTemporalClass::NearLive,
                source_instance_id: source_instance_id(&source.codex_home.identity),
                codex_home: Some(source.codex_home.clone()),
                source_file: Some(source.source_file.clone()),
                cursor: Some(SourceCursor {
                    byte_start: record.byte_start,
                    byte_end: record.byte_end,
                    record_ordinal: record.record_ordinal,
                    line_hash: record.line_hash,
                }),
                timestamps: SourceTimestamps::new(
                    Some(source_timestamp_ms),
                    SourceTimestampKind::Record,
                    observed_timestamp_ms,
                ),
                freshness,
                schema: SchemaEvidence {
                    producer: "codex-rollout".to_string(),
                    producer_version: source.adapter.producer_version.clone(),
                    record_schema: "rollout-jsonl-confirmed".to_string(),
                    schema_version: Some("1".to_string()),
                    schema_fingerprint: Some("phase-2-confirmed-record-set-v1".to_string()),
                },
                confidence: ConfidenceEvidence {
                    level: EvidenceConfidence::Confirmed,
                    basis: vec!["real-cli-rollout-fixtures".to_string()],
                },
                provenance: ProvenanceEvidence {
                    evidence_kind: "rollout-tail-complete-line".to_string(),
                    evidence_refs: vec![format!(
                        "{}:{}-{}",
                        source.source_file.normalized_path, record.byte_start, record.byte_end
                    )],
                },
                record: value,
            });
        }
        if let Err(error) = self.registry.ingest_batch(registry_batch) {
            source.source_file = original_file;
            source.tail = original_tail;
            source.parser = original_parser;
            source.adapter = original_adapter;
            source.health = original_health;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                error.to_string(),
            ));
        }
        Ok(envelopes)
    }

    fn read_with_retry(
        &self,
        source: &mut WatchedSource,
        observed_timestamp_ms: i64,
    ) -> io::Result<RolloutDelta> {
        let attempts = self.config.retry.max_attempts.max(1);
        let mut last_error = None;
        for attempt in 0..attempts {
            let mut working_file = source.source_file.clone();
            let mut working_tail = source.tail.clone();
            match self.reader.read_delta(
                &source.path,
                &mut working_file,
                &mut working_tail,
                observed_timestamp_ms,
            ) {
                Ok(delta) => {
                    source.source_file = working_file;
                    source.tail = working_tail;
                    return Ok(delta);
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < attempts && self.config.retry.initial_backoff_ms > 0 {
                        let multiplier = 1u64 << attempt.min(10);
                        thread::sleep(Duration::from_millis(
                            self.config
                                .retry
                                .initial_backoff_ms
                                .saturating_mul(multiplier),
                        ));
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "read failed")))
    }

    fn save_checkpoint(&self) -> io::Result<()> {
        let mut sources = self
            .sources
            .values()
            .map(|source| RolloutSourceCheckpoint {
                codex_home_identity: source.codex_home.identity.clone(),
                source_file: source.source_file.clone(),
                tail: source.tail.checkpoint().clone(),
                adapter: source.adapter.clone(),
                last_complete_record_observed_at_ms: source
                    .health
                    .last_complete_record_observed_at_ms,
                last_successful_read_at_ms: source.health.last_successful_read_at_ms,
                last_filesystem_signal_at_ms: source.health.last_filesystem_signal_at_ms,
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| {
            left.codex_home_identity
                .cmp(&right.codex_home_identity)
                .then_with(|| {
                    left.source_file
                        .normalized_path
                        .cmp(&right.source_file.normalized_path)
                })
        });
        self.checkpoint_store.save(&RolloutWatcherCheckpoint {
            version: 1,
            sources,
        })
    }

    fn publish_health_if_changed(
        &mut self,
        source: &mut WatchedSource,
        observed_timestamp_ms: i64,
    ) -> io::Result<()> {
        let state = source.health.freshness.state;
        if source.published_freshness == Some(state) {
            return Ok(());
        }
        source.published_freshness = Some(state);
        let Some(thread_key) = source.adapter.thread_key.clone() else {
            return Ok(());
        };
        let turn_key = source
            .adapter
            .active_turn_id
            .as_ref()
            .map(|turn_id| CodexTurnKey::new(thread_key.clone(), turn_id.clone()));
        let last_complete = source
            .health
            .last_complete_record_observed_at_ms
            .unwrap_or(0);
        self.registry
            .ingest(SourceLaneUpdate {
                observation_id: format!(
                    "health:{}:{state:?}:{last_complete}",
                    source.source_file.generation
                ),
                thread_key,
                turn_key,
                source_kind: SourceKind::CodexCliRollout,
                temporal_class: SourceTemporalClass::NearLive,
                source_instance_id: source_instance_id(&source.codex_home.identity),
                source_generation: source.source_file.generation.clone(),
                source_timestamp_ms: source.adapter.source_timestamp_ms,
                observed_timestamp_ms,
                freshness: source.health.freshness.clone(),
                lifecycle: source.adapter.lifecycle,
                observed_model: source.adapter.observed_model.clone(),
                token_snapshot: source.adapter.token_snapshot.clone(),
            })
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        Ok(())
    }

    pub(crate) fn record_filesystem_signal(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        observed_timestamp_ms: i64,
    ) {
        for path in paths {
            let key = path_key(&path);
            if self
                .retired_source_files
                .iter()
                .any(|source_file| path_key(Path::new(&source_file.normalized_path)) == key)
            {
                continue;
            }
            self.pending_signal_times
                .insert(key.clone(), observed_timestamp_ms);
            for source in self.sources.values_mut() {
                if path_key(&source.path) == key {
                    source.health.last_filesystem_signal_at_ms = Some(observed_timestamp_ms);
                }
            }
        }
    }

    pub(crate) fn refresh_health(&mut self, observed_timestamp_ms: i64) -> Vec<SourceHealth> {
        let keys = self.sources.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let Some(mut source) = self.sources.remove(&key) else {
                continue;
            };
            refresh_source_health(
                &mut source,
                observed_timestamp_ms,
                self.config.fresh_window_ms,
                self.config.settled_after_ms,
            );
            let _ = self.publish_health_if_changed(&mut source, observed_timestamp_ms);
            self.sources.insert(key, source);
        }
        let mut health = self
            .sources
            .values()
            .map(|source| source.health.clone())
            .collect::<Vec<_>>();
        health.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        health
    }

    pub(crate) fn health_for_path(&self, path: &Path) -> Option<&SourceHealth> {
        let key = path_key(path);
        self.sources
            .values()
            .find(|source| path_key(&source.path) == key)
            .map(|source| &source.health)
    }

    pub(crate) fn source_file_for_path(&self, path: &Path) -> Option<&SourceFileIdentity> {
        let key = path_key(path);
        self.sources
            .values()
            .find(|source| path_key(&source.path) == key)
            .map(|source| &source.source_file)
    }

    pub(crate) fn registry(&self) -> &SourceAuthorityRegistry {
        &self.registry
    }

    pub(crate) fn registry_mut(&mut self) -> &mut SourceAuthorityRegistry {
        &mut self.registry
    }

    pub(crate) fn watched_roots(&self) -> Vec<PathBuf> {
        self.config
            .homes
            .iter()
            .map(|home| home.root.join("sessions"))
            .collect()
    }

    pub(crate) fn reconciliation_interval(&self) -> Duration {
        Duration::from_millis(self.config.reconciliation_interval_ms.max(1))
    }

    pub(crate) fn fresh_window_ms(&self) -> i64 {
        self.config.fresh_window_ms
    }
}

#[derive(Clone, Debug)]
struct RecordFieldUpdate {
    thread_key: super::rollout_identity::CodexThreadKey,
    turn_key: Option<CodexTurnKey>,
    parent_thread_key: Option<super::rollout_identity::CodexThreadKey>,
    agent_path: Option<String>,
    lifecycle: Option<ExternalLifecycle>,
    observed_model: Option<String>,
    token_snapshot: Option<super::source_registry::TokenSnapshot>,
}

fn apply_record(
    source: &mut WatchedSource,
    record: &ParsedRolloutRecord,
) -> Option<RecordFieldUpdate> {
    let (lifecycle, observed_model, token_snapshot) = match record {
        ParsedRolloutRecord::SessionMeta(meta) => {
            let identity = identity_from_session_meta(&source.codex_home, meta);
            source.source_file.session_meta_id = Some(meta.id.clone());
            source.adapter.thread_key = Some(identity.thread_key);
            source.adapter.root_session_id = identity.root_session_id;
            source.adapter.parent_thread_key = identity.parent_thread_key;
            source.adapter.agent_path = identity.agent_path;
            source.adapter.producer_version = meta.cli_version.clone();
            source.adapter.completed = false;
            (None, None, None)
        }
        ParsedRolloutRecord::TaskStarted(value) => {
            source.adapter.active_turn_id = Some(value.turn_id.clone());
            source.adapter.lifecycle = Some(ExternalLifecycle::Running);
            source.adapter.completed = false;
            (Some(ExternalLifecycle::Running), None, None)
        }
        ParsedRolloutRecord::TurnContext(value) => {
            source.adapter.active_turn_id = Some(value.turn_id.clone());
            if let Some(model) = value.model.clone() {
                source.adapter.observed_model = Some(model.clone());
                (None, Some(model), None)
            } else {
                (None, None, None)
            }
        }
        ParsedRolloutRecord::TokenCount(value) => {
            if let Some(total) = value.total.clone() {
                source.adapter.token_snapshot = Some(total.clone());
                (None, None, Some(total))
            } else {
                (None, None, None)
            }
        }
        ParsedRolloutRecord::WaitStarted(_) => {
            source.adapter.lifecycle = Some(ExternalLifecycle::Waiting);
            (Some(ExternalLifecycle::Waiting), None, None)
        }
        ParsedRolloutRecord::WaitResumed(_) => {
            source.adapter.lifecycle = Some(ExternalLifecycle::Running);
            (Some(ExternalLifecycle::Running), None, None)
        }
        ParsedRolloutRecord::TaskComplete(value) => {
            source.adapter.active_turn_id = Some(value.turn_id.clone());
            source.adapter.lifecycle = Some(ExternalLifecycle::Completed);
            source.adapter.completed = true;
            (Some(ExternalLifecycle::Completed), None, None)
        }
    };
    let thread_key = source.adapter.thread_key.clone()?;
    let turn_key = source
        .adapter
        .active_turn_id
        .as_ref()
        .map(|turn_id| CodexTurnKey::new(thread_key.clone(), turn_id.clone()));
    Some(RecordFieldUpdate {
        thread_key,
        turn_key,
        parent_thread_key: source.adapter.parent_thread_key.clone(),
        agent_path: source.adapter.agent_path.clone(),
        lifecycle,
        observed_model,
        token_snapshot,
    })
}

fn refresh_source_health(
    source: &mut WatchedSource,
    observed_timestamp_ms: i64,
    fresh_window_ms: i64,
    settled_after_ms: i64,
) {
    source.health.freshness = source_freshness(
        source
            .health
            .latest_source_timestamp_ms
            .or(source.adapter.source_timestamp_ms),
        observed_timestamp_ms,
        source.adapter.completed,
        fresh_window_ms,
        settled_after_ms,
        source.health.last_complete_record_observed_at_ms,
    );
}

fn source_freshness(
    source_timestamp_ms: Option<i64>,
    observed_timestamp_ms: i64,
    completed: bool,
    fresh_window_ms: i64,
    settled_after_ms: i64,
    last_complete_record_observed_at_ms: Option<i64>,
) -> FreshnessEvidence {
    let (state, reason) = match source_timestamp_ms {
        None => (
            FreshnessState::Unknown,
            "no reliable rollout source timestamp",
        ),
        Some(source_time)
            if observed_timestamp_ms.saturating_sub(source_time) <= fresh_window_ms.max(0) =>
        {
            (FreshnessState::Fresh, "rollout source timestamp is recent")
        }
        Some(source_time)
            if completed
                && observed_timestamp_ms.saturating_sub(source_time) >= settled_after_ms.max(0) =>
        {
            (
                FreshnessState::Settled,
                "completed rollout source has settled",
            )
        }
        Some(_) => (
            FreshnessState::Stale,
            "rollout source timestamp is not recent",
        ),
    };
    FreshnessEvidence {
        state,
        last_complete_record_observed_at_ms,
        reason: reason.to_string(),
    }
}

fn source_key(home_identity: &str, normalized_path: &str) -> String {
    format!("{home_identity}\u{1f}{normalized_path}")
}

fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn source_instance_id(home_identity: &str) -> String {
    format!("rollout-tail:{home_identity}")
}

fn observation_id(
    home_identity: &str,
    source_file: &SourceFileIdentity,
    byte_start: u64,
    byte_end: u64,
    line_hash: &str,
) -> String {
    let mut digest = Sha256::new();
    for part in [
        home_identity,
        source_file.normalized_path.as_str(),
        source_file.generation.as_str(),
        line_hash,
    ] {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    digest.update(byte_start.to_le_bytes());
    digest.update(byte_end.to_le_bytes());
    format!("rollout:{:x}", digest.finalize())
}

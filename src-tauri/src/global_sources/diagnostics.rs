use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchEvent;
use crate::shared::global_sources_core::source_envelope::SourceEnvelope;
use crate::shared::global_sources_core::source_registry::{
    SourceAuthorityRegistry, SourceLaneUpdate,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct DiagnosticJournal {
    path: PathBuf,
    max_bytes: u64,
}

impl DiagnosticJournal {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            max_bytes: 16 * 1024 * 1024,
        }
    }

    #[cfg(test)]
    fn with_max_bytes(path: PathBuf, max_bytes: u64) -> Self {
        Self { path, max_bytes }
    }

    pub(crate) fn record_watch_event(
        &self,
        event: &RolloutWatchEvent,
        registry: &SourceAuthorityRegistry,
    ) -> io::Result<()> {
        match event {
            RolloutWatchEvent::Reconciled(report) => {
                for envelope in &report.envelopes {
                    self.append(&diagnostic_from_envelope(envelope, registry))?;
                }
                for failure in &report.read_failures {
                    self.append(&json!({
                        "eventKind": "readFailure",
                        "sourcePath": failure.source_path,
                        "message": failure.message,
                        "observedTimestampMs": chrono::Utc::now().timestamp_millis(),
                    }))?;
                }
            }
            RolloutWatchEvent::LiveIngested { update, accepted } => {
                self.append(&diagnostic_from_live(update, *accepted, registry))?;
            }
            RolloutWatchEvent::DeletionReconciled(report) => {
                self.append(&json!({
                    "eventKind": "deletionReconciled",
                    "monitorDeleteOperationId": report.monitor_delete_operation_id,
                    "rootThreadId": report.root_thread_id,
                    "descendantThreadIds": report.descendant_thread_ids,
                    "tombstonePersistenceOutcome": report.tombstone_persisted,
                    "registryRetirementCount": report.registry_retirement_count,
                    "watcherSourceRetirementCount": report.watcher_source_retirement_count,
                    "checkpointRewriteOutcome": report.checkpoint_rewritten,
                    "reconciliationState": report.reconciliation_state,
                    "desktopReconciliation": report.desktop_reconciliation,
                    "snapshotPublicationRevision": report.snapshot_publication_revision,
                    "observedTimestampMs": chrono::Utc::now().timestamp_millis(),
                }))?;
            }
            RolloutWatchEvent::DeletionReconciliationFailed(failure) => {
                self.append(&json!({
                    "eventKind": "deletionReconciliationFailed",
                    "monitorDeleteOperationId": failure.monitor_delete_operation_id,
                    "rootThreadId": failure.root_thread_id,
                    "descendantThreadIds": failure.descendant_thread_ids,
                    "tombstonePersistenceOutcome": failure.tombstone_persisted,
                    "message": failure.message,
                    "observedTimestampMs": chrono::Utc::now().timestamp_millis(),
                }))?;
            }
        }
        Ok(())
    }

    pub(crate) fn record_service_state(&self, state: &str, details: Value) -> io::Result<()> {
        self.append(&json!({
            "eventKind": "serviceState",
            "state": state,
            "observedTimestampMs": chrono::Utc::now().timestamp_millis(),
            "details": details,
        }))
    }

    fn append(&self, value: &Value) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut record = serde_json::to_vec(value).map_err(io::Error::other)?;
        record.push(b'\n');
        let should_truncate = fs::metadata(&self.path)
            .map(|metadata| metadata.len().saturating_add(record.len() as u64) > self.max_bytes)
            .unwrap_or(false);
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if should_truncate {
            options.truncate(true);
        } else {
            options.append(true);
        }
        let mut file = options.open(&self.path)?;
        file.write_all(&record)?;
        file.flush()
    }
}

fn diagnostic_from_envelope(
    envelope: &SourceEnvelope<Value>,
    registry: &SourceAuthorityRegistry,
) -> Value {
    let payload = envelope.record.get("payload").unwrap_or(&Value::Null);
    let record_kind = rollout_record_kind(&envelope.record);
    let thread_id = envelope
        .source_file
        .as_ref()
        .and_then(|source| source.session_meta_id.as_deref())
        .or_else(|| payload.get("id").and_then(Value::as_str));
    let parent_thread_id = payload
        .get("source")
        .and_then(|source| source.get("subagent"))
        .and_then(|subagent| subagent.get("thread_spawn"))
        .and_then(|spawn| spawn.get("parent_thread_id"))
        .and_then(Value::as_str);
    let turn_id = payload.get("turn_id").and_then(Value::as_str);
    let model = if record_kind == "turn_context" {
        payload.get("model").and_then(Value::as_str)
    } else {
        None
    };
    let token_snapshot = if record_kind == "token_count" {
        payload
            .get("info")
            .and_then(|info| info.get("total_token_usage"))
            .map(rollout_token_json)
    } else {
        None
    };
    let authority = authority_json(
        envelope
            .codex_home
            .as_ref()
            .map(|home| home.identity.as_str()),
        thread_id,
        registry,
    );
    json!({
        "eventKind": "rolloutObservation",
        "recordKind": record_kind,
        "observationId": envelope.observation_id,
        "sourceKind": envelope.source_kind,
        "temporalClass": envelope.temporal_class,
        "sourceInstanceId": envelope.source_instance_id,
        "codexHomeIdentity": envelope.codex_home.as_ref().map(|home| &home.identity),
        "sourcePath": envelope.source_file.as_ref().map(|source| &source.normalized_path),
        "generation": envelope.source_file.as_ref().map(|source| &source.generation),
        "byteStart": envelope.cursor.as_ref().map(|cursor| cursor.byte_start),
        "byteEnd": envelope.cursor.as_ref().map(|cursor| cursor.byte_end),
        "recordOrdinal": envelope.cursor.as_ref().map(|cursor| cursor.record_ordinal),
        "lineHash": envelope.cursor.as_ref().map(|cursor| &cursor.line_hash),
        "threadId": thread_id,
        "parentThreadId": parent_thread_id,
        "turnId": turn_id,
        "model": model,
        "tokenSnapshot": token_snapshot,
        "lifecycle": rollout_lifecycle(record_kind),
        "sourceTimestampMs": envelope.timestamps.source_timestamp_ms,
        "observedTimestampMs": envelope.timestamps.observed_timestamp_ms,
        "lagMs": envelope.timestamps.lag_ms,
        "freshness": envelope.freshness,
        "authority": authority,
    })
}

fn rollout_token_json(value: &Value) -> Value {
    json!({
        "inputTokens": value.get("input_tokens").and_then(Value::as_u64),
        "cachedInputTokens": value.get("cached_input_tokens").and_then(Value::as_u64),
        "cacheWriteInputTokens": value.get("cache_write_input_tokens").and_then(Value::as_u64),
        "outputTokens": value.get("output_tokens").and_then(Value::as_u64),
        "reasoningOutputTokens": value.get("reasoning_output_tokens").and_then(Value::as_u64),
        "totalTokens": value.get("total_tokens").and_then(Value::as_u64),
    })
}

fn diagnostic_from_live(
    update: &SourceLaneUpdate,
    accepted: bool,
    registry: &SourceAuthorityRegistry,
) -> Value {
    json!({
        "eventKind": "liveObservation",
        "accepted": accepted,
        "observationId": update.observation_id,
        "sourceKind": update.source_kind,
        "temporalClass": update.temporal_class,
        "sourceInstanceId": update.source_instance_id,
        "generation": update.source_generation,
        "threadId": update.thread_key.thread_id,
        "turnId": update.turn_key.as_ref().map(|turn| &turn.turn_id),
        "model": update.observed_model,
        "tokenSnapshot": update.token_snapshot,
        "lifecycle": update.lifecycle,
        "sourceTimestampMs": update.source_timestamp_ms,
        "observedTimestampMs": update.observed_timestamp_ms,
        "lagMs": update.source_timestamp_ms.map(|source| update.observed_timestamp_ms - source),
        "freshness": update.freshness,
        "authority": authority_json(
            Some(&update.thread_key.codex_home_identity),
            Some(&update.thread_key.thread_id),
            registry,
        ),
    })
}

fn authority_json(
    home_identity: Option<&str>,
    thread_id: Option<&str>,
    registry: &SourceAuthorityRegistry,
) -> Value {
    let (Some(home_identity), Some(thread_id)) = (home_identity, thread_id) else {
        return Value::Null;
    };
    let key = CodexThreadKey::new(home_identity, thread_id);
    let Some(lanes) = registry.lanes(&key) else {
        return Value::Null;
    };
    let resolved = registry.resolve(&key).unwrap_or_default();
    json!({
        "canonicalThreadId": thread_id,
        "liveLaneCount": lanes.live_count(),
        "nearLiveLaneCount": lanes.near_live_count(),
        "lifecycle": resolved_value_json(resolved.lifecycle.as_ref()),
        "observedModel": resolved_value_json(resolved.observed_model.as_ref()),
        "tokenSnapshot": resolved_value_json(resolved.token_snapshot.as_ref()),
    })
}

fn resolved_value_json<T: Serialize>(
    resolved: Option<&crate::shared::global_sources_core::source_registry::ResolvedValue<T>>,
) -> Value {
    let Some(resolved) = resolved else {
        return Value::Null;
    };
    json!({
        "value": resolved.value,
        "provenance": {
            "sourceKind": resolved.provenance.source_kind,
            "temporalClass": resolved.provenance.temporal_class,
            "sourceInstanceId": resolved.provenance.source_instance_id,
            "sourceGeneration": resolved.provenance.source_generation,
            "sourceTimestampMs": resolved.provenance.source_timestamp_ms,
            "observedTimestampMs": resolved.provenance.observed_timestamp_ms,
            "freshness": resolved.provenance.freshness,
        }
    })
}

fn rollout_record_kind(record: &Value) -> &str {
    let outer = record
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if outer == "event_msg" {
        record
            .get("payload")
            .and_then(|payload| payload.get("type"))
            .and_then(Value::as_str)
            .unwrap_or(outer)
    } else {
        outer
    }
}

fn rollout_lifecycle(record_kind: &str) -> Option<&'static str> {
    match record_kind {
        "task_started" | "resume" => Some("running"),
        "wait_agent" => Some("waiting"),
        "task_complete" => Some("completed"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
    use crate::shared::global_sources_core::rollout_watcher::ReconcileReport;
    use crate::shared::global_sources_core::source_envelope::{
        CodexHomeIdentity, ConfidenceEvidence, EvidenceConfidence, FreshnessEvidence,
        FreshnessState, ProvenanceEvidence, SchemaEvidence, SourceCursor, SourceEnvelope,
        SourceFileIdentity, SourceKind, SourceTemporalClass, SourceTimestampKind, SourceTimestamps,
    };
    use crate::shared::global_sources_core::source_registry::{
        SourceAuthorityRegistry, SourceLaneUpdate, TokenSnapshot,
    };
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn journal_records_identity_timing_tokens_and_authority_without_conversation_content() {
        let path = std::env::temp_dir().join(format!(
            "codex-monitor-source-diagnostics-{}.jsonl",
            Uuid::new_v4()
        ));
        let mut registry = SourceAuthorityRegistry::default();
        let key = CodexThreadKey::new("codex-home:fixture", "thread-1");
        registry
            .ingest(SourceLaneUpdate {
                observation_id: "near-live-token".to_string(),
                thread_key: key,
                turn_key: None,
                source_kind: SourceKind::CodexCliRollout,
                temporal_class: SourceTemporalClass::NearLive,
                source_instance_id: "rollout-tail:fixture".to_string(),
                source_generation: "generation-1".to_string(),
                source_timestamp_ms: Some(1_000),
                observed_timestamp_ms: 1_125,
                freshness: FreshnessEvidence {
                    state: FreshnessState::Fresh,
                    last_complete_record_observed_at_ms: Some(1_125),
                    reason: "fixture".to_string(),
                },
                lifecycle: None,
                observed_model: None,
                token_snapshot: Some(TokenSnapshot {
                    total_tokens: 42,
                    input_tokens: 40,
                    output_tokens: 2,
                    ..TokenSnapshot::default()
                }),
            })
            .expect("registry ingest");
        let envelope = SourceEnvelope {
            envelope_version: 1,
            observation_id: "rollout-observation".to_string(),
            source_kind: SourceKind::CodexCliRollout,
            temporal_class: SourceTemporalClass::NearLive,
            source_instance_id: "rollout-tail:fixture".to_string(),
            codex_home: Some(CodexHomeIdentity {
                normalized_path: r"C:\fixture\codex-home".to_string(),
                identity: "codex-home:fixture".to_string(),
            }),
            source_file: Some(SourceFileIdentity {
                normalized_path: r"C:\fixture\rollout.jsonl".to_string(),
                filesystem_id: Some("fixture-file".to_string()),
                generation: "generation-1".to_string(),
                session_meta_id: Some("thread-1".to_string()),
            }),
            cursor: Some(SourceCursor {
                byte_start: 10,
                byte_end: 120,
                record_ordinal: 2,
                line_hash: "line-hash".to_string(),
            }),
            timestamps: SourceTimestamps::new(Some(1_000), SourceTimestampKind::Record, 1_125),
            freshness: FreshnessEvidence {
                state: FreshnessState::Fresh,
                last_complete_record_observed_at_ms: Some(1_125),
                reason: "fixture".to_string(),
            },
            schema: SchemaEvidence {
                producer: "codex-rollout".to_string(),
                producer_version: Some("0.147.0".to_string()),
                record_schema: "rollout-jsonl-confirmed".to_string(),
                schema_version: Some("1".to_string()),
                schema_fingerprint: Some("fixture".to_string()),
            },
            confidence: ConfidenceEvidence {
                level: EvidenceConfidence::Confirmed,
                basis: vec!["fixture".to_string()],
            },
            provenance: ProvenanceEvidence {
                evidence_kind: "rollout-tail-complete-line".to_string(),
                evidence_refs: vec!["fixture".to_string()],
            },
            record: json!({
                "timestamp": "1970-01-01T00:00:01Z",
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "text": "SECRET_PROMPT_MUST_NOT_BE_WRITTEN",
                    "info": { "total_token_usage": {
                        "input_tokens": 40,
                        "cached_input_tokens": 0,
                        "cache_write_input_tokens": 0,
                        "output_tokens": 2,
                        "reasoning_output_tokens": 0,
                        "total_tokens": 42
                    }}
                }
            }),
        };
        let journal = DiagnosticJournal::new(path.clone());

        journal
            .record_watch_event(
                &RolloutWatchEvent::Reconciled(ReconcileReport {
                    envelopes: vec![envelope],
                    discovered_sources: 1,
                    processed_sources: 1,
                    read_failures: vec![],
                }),
                &registry,
            )
            .expect("write diagnostic");

        let written = fs::read_to_string(&path).expect("journal");
        let row: serde_json::Value = serde_json::from_str(written.trim()).expect("json row");
        assert_eq!(row["recordKind"], "token_count");
        assert_eq!(row["threadId"], "thread-1");
        assert_eq!(row["lagMs"], 125);
        assert_eq!(row["tokenSnapshot"]["totalTokens"], 42);
        assert_eq!(row["authority"]["nearLiveLaneCount"], 1);
        assert!(!written.contains("SECRET_PROMPT_MUST_NOT_BE_WRITTEN"));
        fs::remove_file(path).ok();
    }

    #[test]
    fn diagnostic_journal_truncates_non_authoritative_history_at_the_size_cap() {
        let path = std::env::temp_dir().join(format!(
            "codex-monitor-diagnostic-cap-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let journal = DiagnosticJournal::with_max_bytes(path.clone(), 160);
        journal
            .record_service_state("first", json!({ "payload": "x".repeat(80) }))
            .expect("first diagnostic");
        journal
            .record_service_state("second", json!({ "payload": "y".repeat(80) }))
            .expect("second diagnostic");

        let written = fs::read_to_string(&path).expect("capped journal");
        assert!(!written.contains("\"state\":\"first\""));
        assert!(written.contains("\"state\":\"second\""));
        assert!(fs::metadata(&path).expect("metadata").len() < 256);
        fs::remove_file(path).ok();
    }

    #[tokio::test]
    #[ignore = "manual Phase 2.3 real CLI probe"]
    async fn real_cli_rollout_watch_service_probe() {
        use crate::shared::global_sources_core::rollout_watch_service::RolloutWatchService;
        use crate::shared::global_sources_core::rollout_watcher::{
            RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy,
        };
        use crate::shared::global_sources_core::runtime_config::discover_runtime_codex_homes;

        let codex_home =
            PathBuf::from(std::env::var("CODEX_MONITOR_PHASE23_PROBE_HOME").expect("probe home"));
        let checkpoint = PathBuf::from(
            std::env::var("CODEX_MONITOR_PHASE23_PROBE_CHECKPOINT").expect("probe checkpoint"),
        );
        let diagnostics = PathBuf::from(
            std::env::var("CODEX_MONITOR_PHASE23_PROBE_DIAGNOSTICS").expect("probe diagnostics"),
        );
        let seconds = std::env::var("CODEX_MONITOR_PHASE23_PROBE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(120);
        let homes = discover_runtime_codex_homes(Some(codex_home), []);
        let watcher = RolloutTailWatcher::new(RolloutWatcherConfig {
            homes,
            deletion_tombstones_path: checkpoint.with_file_name("deletion-tombstones.json"),
            checkpoint_path: checkpoint,
            retry: WatcherRetryPolicy {
                max_attempts: 5,
                initial_backoff_ms: 50,
            },
            fresh_window_ms: 5_000,
            settled_after_ms: 2_000,
            reconciliation_interval_ms: 500,
        });
        let service = RolloutWatchService::new(watcher).expect("probe watch service");
        let journal = DiagnosticJournal::new(diagnostics);
        let (shutdown, receiver) = tokio::sync::watch::channel(false);
        let (_commands, command_receiver) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            let _ = shutdown.send(true);
        });

        service
            .run_until(receiver, command_receiver, |event, registry| {
                journal
                    .record_watch_event(&event, registry)
                    .expect("probe diagnostic write");
            })
            .await
            .expect("probe service run");
    }
}

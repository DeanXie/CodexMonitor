use super::rollout_identity::{identity_from_session_meta, CodexThreadKey, CodexTurnKey};
use super::rollout_record::{ParsedRolloutRecord, RolloutRecordParser};
use super::rollout_tail::{read_rollout_delta, RolloutTailState};
use super::source_envelope::{
    CodexHomeIdentity, ConfidenceEvidence, EvidenceConfidence, FreshnessEvidence, FreshnessState,
    ProvenanceEvidence, SchemaEvidence, SourceCursor, SourceEnvelope, SourceFileIdentity,
    SourceKind, SourceTemporalClass, SourceTimestampKind, SourceTimestamps,
};
use super::source_registry::{
    ExternalLifecycle, SourceAuthorityRegistry, SourceLaneUpdate, TokenSnapshot,
};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("docs")
        .join("fixtures")
        .join("cli-rollout")
        .join(name)
}

fn parse_jsonl_fixture(name: &str) -> Vec<ParsedRolloutRecord> {
    let content = fs::read_to_string(fixture_path(name)).expect("fixture");
    let mut parser = RolloutRecordParser::default();
    content
        .lines()
        .filter_map(|line| parser.parse_line(line).expect("valid rollout record"))
        .collect()
}

fn temp_file() -> PathBuf {
    std::env::temp_dir().join(format!("codex-monitor-rollout-{}.jsonl", Uuid::new_v4()))
}

fn file_identity(path: &Path, generation: &str) -> SourceFileIdentity {
    SourceFileIdentity {
        normalized_path: path.to_string_lossy().into_owned(),
        filesystem_id: None,
        generation: generation.to_string(),
        session_meta_id: None,
    }
}

fn home() -> CodexHomeIdentity {
    CodexHomeIdentity {
        normalized_path: "C:\\fixture\\codex-home".to_string(),
        identity: "codex-home-fixture".to_string(),
    }
}

fn freshness(state: FreshnessState) -> FreshnessEvidence {
    FreshnessEvidence {
        state,
        last_complete_record_observed_at_ms: Some(2_000),
        reason: "fixture".to_string(),
    }
}

fn token(total: u64) -> TokenSnapshot {
    TokenSnapshot {
        input_tokens: total - 10,
        cached_input_tokens: 0,
        cache_write_input_tokens: Some(0),
        output_tokens: 10,
        reasoning_output_tokens: 0,
        total_tokens: total,
    }
}

fn lane_update(
    observation_id: &str,
    thread_key: CodexThreadKey,
    source_kind: SourceKind,
    temporal_class: SourceTemporalClass,
    timestamp_ms: i64,
) -> SourceLaneUpdate {
    SourceLaneUpdate {
        observation_id: observation_id.to_string(),
        thread_key,
        turn_key: None,
        source_kind,
        temporal_class,
        source_instance_id: format!("source-{observation_id}"),
        source_generation: "generation-1".to_string(),
        source_timestamp_ms: Some(timestamp_ms),
        observed_timestamp_ms: timestamp_ms + 5,
        freshness: freshness(FreshnessState::Fresh),
        lifecycle: None,
        observed_model: None,
        token_snapshot: None,
    }
}

#[test]
fn source_envelope_keeps_source_and_observed_time_separate() {
    let envelope = SourceEnvelope {
        envelope_version: 1,
        observation_id: "observation-1".to_string(),
        source_kind: SourceKind::CodexCliRollout,
        temporal_class: SourceTemporalClass::NearLive,
        source_instance_id: "rollout-tail-default".to_string(),
        codex_home: Some(home()),
        source_file: Some(SourceFileIdentity {
            normalized_path: "C:\\fixture\\rollout.jsonl".to_string(),
            filesystem_id: Some("volume:file".to_string()),
            generation: "generation-1".to_string(),
            session_meta_id: Some("thread-1".to_string()),
        }),
        cursor: Some(SourceCursor {
            byte_start: 10,
            byte_end: 25,
            record_ordinal: 2,
            line_hash: "abc".to_string(),
        }),
        timestamps: SourceTimestamps::new(Some(1_000), SourceTimestampKind::Record, 1_025),
        freshness: freshness(FreshnessState::Fresh),
        schema: SchemaEvidence {
            producer: "codex-rollout".to_string(),
            producer_version: Some("0.147.0".to_string()),
            record_schema: "rollout-jsonl-confirmed-v1".to_string(),
            schema_version: Some("1".to_string()),
            schema_fingerprint: Some("fixture-schema-fingerprint".to_string()),
        },
        confidence: ConfidenceEvidence {
            level: EvidenceConfidence::Confirmed,
            basis: vec!["fixture".to_string()],
        },
        provenance: ProvenanceEvidence {
            evidence_kind: "captured-fixture".to_string(),
            evidence_refs: vec!["single-agent.events.jsonl".to_string()],
        },
        record: serde_json::json!({"type": "session_meta"}),
    };

    assert_eq!(envelope.timestamps.source_timestamp_ms, Some(1_000));
    assert_eq!(envelope.timestamps.observed_timestamp_ms, 1_025);
    assert_eq!(envelope.timestamps.lag_ms, Some(25));
    let json = serde_json::to_value(envelope).expect("serialize envelope");
    assert_eq!(json["sourceKind"], "codex-cli-rollout");
    assert_eq!(json["temporalClass"], "NEAR_LIVE");
    assert_eq!(json["schema"]["schemaVersion"], "1");
    assert_eq!(json["provenance"]["evidenceKind"], "captured-fixture");
}

#[test]
fn parser_reads_single_agent_model_token_and_lifecycle() {
    let records = parse_jsonl_fixture("single-agent.events.jsonl");
    assert_eq!(records.len(), 5);

    let ParsedRolloutRecord::SessionMeta(meta) = &records[0] else {
        panic!("session_meta expected");
    };
    assert_eq!(meta.id, "thread-single");
    assert_eq!(meta.session_id.as_deref(), Some("thread-single"));
    assert!(meta.subagent_spawn.is_none());

    let ParsedRolloutRecord::TaskStarted(started) = &records[1] else {
        panic!("task_started expected");
    };
    assert_eq!(started.turn_id, "turn-single-1");
    assert_eq!(started.started_at_seconds, 1_787_666_229);

    let ParsedRolloutRecord::TurnContext(context) = &records[2] else {
        panic!("turn_context expected");
    };
    assert_eq!(context.model.as_deref(), Some("gpt-5.6-terra"));

    let ParsedRolloutRecord::TokenCount(tokens) = &records[3] else {
        panic!("token_count expected");
    };
    assert_eq!(tokens.total.as_ref().unwrap().total_tokens, 18_320);
    assert_eq!(tokens.last.as_ref().unwrap().total_tokens, 18_320);

    let ParsedRolloutRecord::TaskComplete(completed) = &records[4] else {
        panic!("task_complete expected");
    };
    assert_eq!(completed.turn_id, "turn-single-1");
    assert_eq!(completed.duration_ms, 10_513);
}

#[test]
fn parser_reads_resumed_turn_as_new_turn_with_cumulative_and_last_token_usage() {
    let records = parse_jsonl_fixture("multi-turn.events.jsonl");
    let starts = records
        .iter()
        .filter_map(|record| match record {
            ParsedRolloutRecord::TaskStarted(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let tokens = records
        .iter()
        .filter_map(|record| match record {
            ParsedRolloutRecord::TokenCount(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0].turn_id, "turn-1");
    assert_eq!(starts[1].turn_id, "turn-2");
    assert_eq!(tokens[1].total.as_ref().unwrap().total_tokens, 38_243);
    assert_eq!(tokens[1].last.as_ref().unwrap().total_tokens, 19_923);
}

#[test]
fn parser_uses_only_child_session_meta_for_parent_relation_and_pairs_wait_resume() {
    let value: Value = serde_json::from_str(
        &fs::read_to_string(fixture_path("multi-agent.files.json")).expect("fixture"),
    )
    .expect("json");
    let files = value["files"].as_array().expect("files");
    let mut parser = RolloutRecordParser::default();

    let main_records = files[0]["records"].as_array().expect("main records");
    let parsed_main = main_records
        .iter()
        .filter_map(|record| parser.parse_value(record).expect("main record"))
        .collect::<Vec<_>>();
    assert!(parsed_main
        .iter()
        .any(|record| matches!(record, ParsedRolloutRecord::WaitStarted(value) if value.call_id == "wait-call-1")));
    assert!(parsed_main
        .iter()
        .any(|record| matches!(record, ParsedRolloutRecord::WaitResumed(value) if value.call_id == "wait-call-1")));

    for child in files.iter().skip(1) {
        let meta_value = &child["records"][0];
        let ParsedRolloutRecord::SessionMeta(meta) = parser
            .parse_value(meta_value)
            .expect("child meta")
            .expect("handled child meta")
        else {
            panic!("session_meta expected");
        };
        let spawn = meta.subagent_spawn.expect("confirmed parent relation");
        assert_eq!(meta.session_id.as_deref(), Some("thread-main"));
        assert_ne!(Some(meta.id.as_str()), meta.session_id.as_deref());
        assert_eq!(spawn.parent_thread_id, "thread-main");
        assert!(spawn.agent_path.as_deref().unwrap().starts_with("/root/"));
    }
}

#[test]
fn identity_uses_codex_home_and_full_ids_without_inferring_parent() {
    let records = parse_jsonl_fixture("single-agent.events.jsonl");
    let ParsedRolloutRecord::SessionMeta(meta) = &records[0] else {
        panic!("session_meta expected");
    };
    let identity = identity_from_session_meta(&home(), meta);
    assert_eq!(
        identity.thread_key,
        CodexThreadKey::new("codex-home-fixture", "thread-single")
    );
    assert_eq!(identity.root_session_id.as_deref(), Some("thread-single"));
    assert!(identity.parent_thread_key.is_none());
    assert_eq!(
        CodexTurnKey::new(identity.thread_key.clone(), "turn-single-1").turn_id,
        "turn-single-1"
    );
}

#[test]
fn byte_cursor_handles_utf8_partial_lines_and_does_not_replay_committed_records() {
    let path = temp_file();
    let first = "{\"text\":\"你好\"}\n";
    let partial = "{\"text\":\"半";
    fs::write(&path, format!("{first}{partial}").as_bytes()).expect("write fixture");
    let mut identity = file_identity(&path, "generation-1");
    let mut state = RolloutTailState::new("generation-1");

    let first_delta =
        read_rollout_delta(&path, &mut identity, &mut state, 1_000).expect("first delta");
    assert_eq!(first_delta.records.len(), 1);
    assert_eq!(first_delta.records[0].text, "{\"text\":\"你好\"}");
    assert_eq!(
        first_delta.records[0].byte_end,
        first.as_bytes().len() as u64
    );
    assert_eq!(
        state.checkpoint().committed_byte_offset,
        first.as_bytes().len() as u64
    );
    assert_eq!(state.pending_tail(), partial.as_bytes());

    let mut append = OpenOptions::new().append(true).open(&path).expect("append");
    append.write_all("条\"}\n".as_bytes()).expect("finish line");
    drop(append);

    let second_delta =
        read_rollout_delta(&path, &mut identity, &mut state, 1_010).expect("second delta");
    assert_eq!(second_delta.records.len(), 1);
    assert_eq!(second_delta.records[0].text, "{\"text\":\"半条\"}");
    assert!(state.pending_tail().is_empty());

    let checkpoint = serde_json::from_str(
        &serde_json::to_string(state.checkpoint()).expect("serialize checkpoint"),
    )
    .expect("restore checkpoint");
    let mut restarted = RolloutTailState::from_checkpoint(checkpoint);
    let replay =
        read_rollout_delta(&path, &mut identity, &mut restarted, 1_020).expect("restart delta");
    assert!(replay.records.is_empty());
    fs::remove_file(path).ok();
}

#[test]
fn line_hash_is_stable_and_based_on_complete_line_bytes() {
    let path = temp_file();
    fs::write(&path, b"hello\n").expect("write fixture");
    let mut identity = file_identity(&path, "generation-1");
    let mut state = RolloutTailState::new("generation-1");
    let delta = read_rollout_delta(&path, &mut identity, &mut state, 1_000).expect("delta");
    assert_eq!(
        delta.records[0].line_hash,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    fs::remove_file(path).ok();
}

#[test]
fn truncate_rotates_generation_and_rereads_session_meta_without_negative_cursor() {
    let path = temp_file();
    fs::write(
        &path,
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"old\"}}\n{\"type\":\"event_msg\"}\n",
    )
    .expect("old generation");
    let mut identity = file_identity(&path, "generation-1");
    let mut state = RolloutTailState::new("generation-1");
    let first = read_rollout_delta(&path, &mut identity, &mut state, 1_000).expect("first");
    assert_eq!(first.records.len(), 2);
    let previous_offset = state.checkpoint().committed_byte_offset;

    fs::write(
        &path,
        b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"new\"}}\n",
    )
    .expect("truncate");
    assert!(fs::metadata(&path).expect("metadata").len() < previous_offset);

    let reset = read_rollout_delta(&path, &mut identity, &mut state, 2_000).expect("reset");
    assert!(reset.did_reset);
    assert_ne!(identity.generation, "generation-1");
    assert_eq!(reset.records.len(), 1);
    assert!(reset.records[0].text.contains("session_meta"));
    assert_eq!(
        state.checkpoint().committed_byte_offset,
        fs::metadata(&path).unwrap().len()
    );
    fs::remove_file(path).ok();
}

#[test]
fn authority_deduplicates_same_thread_across_live_and_rollout_lanes() {
    let pair: Value = serde_json::from_str(
        &fs::read_to_string(fixture_path("app-server-rollout-pair.json")).expect("fixture"),
    )
    .expect("json");
    let thread_id = pair["appServer"]["threadStart"]["thread"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(thread_id, pair["rollout"]["sessionMeta"]["payload"]["id"]);
    let key = CodexThreadKey::new("codex-home-fixture", thread_id);
    let mut registry = SourceAuthorityRegistry::default();

    let mut live = lane_update(
        "live-1",
        key.clone(),
        SourceKind::MonitorAppServer,
        SourceTemporalClass::Live,
        2_000,
    );
    live.lifecycle = Some(ExternalLifecycle::Running);
    live.observed_model = Some("gpt-5.6-terra".to_string());
    live.token_snapshot = Some(token(20_635));
    registry.ingest(live).expect("live ingest");

    let mut rollout = lane_update(
        "rollout-1",
        key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        2_100,
    );
    rollout.lifecycle = Some(ExternalLifecycle::Completed);
    rollout.observed_model = Some("gpt-5.6-terra".to_string());
    rollout.token_snapshot = Some(token(20_635));
    registry.ingest(rollout.clone()).expect("rollout ingest");
    assert!(!registry.ingest(rollout).expect("duplicate ingest"));

    assert_eq!(registry.thread_count(), 1);
    let lanes = registry.lanes(&key).expect("lanes");
    assert_eq!(lanes.live_count(), 1);
    assert_eq!(lanes.near_live_count(), 1);
    let view = registry.resolve(&key).expect("view");
    assert_eq!(view.lifecycle.unwrap().value, ExternalLifecycle::Running);
    assert_eq!(view.token_snapshot.unwrap().value.total_tokens, 20_635);
}

#[test]
fn older_rollout_cannot_override_fresh_live_and_tokens_are_never_added() {
    let key = CodexThreadKey::new("codex-home-fixture", "thread-1");
    let mut registry = SourceAuthorityRegistry::default();
    let mut live = lane_update(
        "live",
        key.clone(),
        SourceKind::MonitorAppServer,
        SourceTemporalClass::Live,
        3_000,
    );
    live.lifecycle = Some(ExternalLifecycle::Waiting);
    live.token_snapshot = Some(token(300));
    registry.ingest(live).unwrap();

    let mut rollout = lane_update(
        "rollout",
        key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        2_000,
    );
    rollout.lifecycle = Some(ExternalLifecycle::Completed);
    rollout.token_snapshot = Some(token(250));
    registry.ingest(rollout).unwrap();

    let view = registry.resolve(&key).unwrap();
    assert_eq!(view.lifecycle.unwrap().value, ExternalLifecycle::Waiting);
    assert_eq!(view.token_snapshot.unwrap().value.total_tokens, 300);
}

#[test]
fn stale_live_falls_back_to_cumulative_rollout_snapshot_without_delta_math() {
    let key = CodexThreadKey::new("codex-home-fixture", "thread-1");
    let mut registry = SourceAuthorityRegistry::default();
    let mut live = lane_update(
        "live",
        key.clone(),
        SourceKind::MonitorAppServer,
        SourceTemporalClass::Live,
        2_000,
    );
    live.freshness = freshness(FreshnessState::Stale);
    live.token_snapshot = Some(token(300));
    registry.ingest(live).unwrap();

    let mut behind = lane_update(
        "rollout-behind",
        key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        2_100,
    );
    behind.token_snapshot = Some(token(250));
    registry.ingest(behind).unwrap();
    assert_eq!(
        registry
            .resolve(&key)
            .unwrap()
            .token_snapshot
            .unwrap()
            .value
            .total_tokens,
        300
    );

    let mut caught_up = lane_update(
        "rollout-caught-up",
        key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        2_200,
    );
    caught_up.source_instance_id = "source-rollout-behind".to_string();
    caught_up.token_snapshot = Some(token(340));
    registry.ingest(caught_up).unwrap();
    let resolved = registry.resolve(&key).unwrap().token_snapshot.unwrap();
    assert_eq!(resolved.value.total_tokens, 340);
    assert_eq!(
        resolved.provenance.temporal_class,
        SourceTemporalClass::NearLive
    );
}

#[test]
fn live_lane_expires_from_elapsed_observation_time_and_falls_back_without_token_regression() {
    let key = CodexThreadKey::new("codex-home-fixture", "thread-expiry");
    let mut registry = SourceAuthorityRegistry::default();
    let mut live = lane_update(
        "live-expiry",
        key.clone(),
        SourceKind::MonitorAppServer,
        SourceTemporalClass::Live,
        1_000,
    );
    live.lifecycle = Some(ExternalLifecycle::Running);
    live.token_snapshot = Some(token(300));
    registry.ingest(live).unwrap();
    let mut rollout = lane_update(
        "rollout-expiry",
        key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        5_500,
    );
    rollout.lifecycle = Some(ExternalLifecycle::Completed);
    rollout.token_snapshot = Some(token(250));
    registry.ingest(rollout).unwrap();

    assert_eq!(registry.expire_live_lanes(6_006, 5_000), 1);

    let resolved = registry.resolve(&key).unwrap();
    assert_eq!(
        resolved.lifecycle.unwrap().value,
        ExternalLifecycle::Completed
    );
    assert_eq!(resolved.token_snapshot.unwrap().value.total_tokens, 300);
}

#[test]
fn historical_lane_never_drives_active_lifecycle() {
    let key = CodexThreadKey::new("codex-home-fixture", "thread-1");
    let mut registry = SourceAuthorityRegistry::default();
    let mut historical = lane_update(
        "history",
        key.clone(),
        SourceKind::HistoricalRolloutScan,
        SourceTemporalClass::Historical,
        1_000,
    );
    historical.lifecycle = Some(ExternalLifecycle::Running);
    historical.observed_model = Some("gpt-history".to_string());
    registry.ingest(historical).unwrap();

    let view = registry.resolve(&key).unwrap();
    assert!(view.lifecycle.is_none());
    assert_eq!(view.observed_model.unwrap().value, "gpt-history");
}

#[test]
fn canonical_snapshot_exposes_confirmed_identity_turn_authority_and_provenance() {
    let parent_key = CodexThreadKey::new("codex-home-fixture", "thread-main");
    let child_key = CodexThreadKey::new("codex-home-fixture", "thread-child");
    let turn_key = CodexTurnKey::new(child_key.clone(), "turn-child-1");
    let mut registry = SourceAuthorityRegistry::default();

    let mut started = lane_update(
        "rollout-started",
        child_key.clone(),
        SourceKind::CodexCliRollout,
        SourceTemporalClass::NearLive,
        1_000,
    );
    started.turn_key = Some(turn_key.clone());
    started.lifecycle = Some(ExternalLifecycle::Running);
    started.observed_model = Some("gpt-5.6-terra".to_string());
    started.token_snapshot = Some(token(300));
    registry.observe_identity(
        &started,
        Some(parent_key.clone()),
        Some("/root/reader".to_string()),
    );
    registry.ingest(started).expect("started ingest");

    let mut live_tokens = lane_update(
        "live-token",
        child_key.clone(),
        SourceKind::MonitorAppServer,
        SourceTemporalClass::Live,
        1_100,
    );
    live_tokens.turn_key = Some(turn_key.clone());
    live_tokens.token_snapshot = Some(token(320));
    registry.ingest(live_tokens).expect("live ingest");

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let thread = &snapshot.threads[0];
    assert_eq!(thread.key, child_key);
    assert_eq!(
        thread.parent_thread_key.as_ref().map(|value| &value.value),
        Some(&parent_key)
    );
    assert_eq!(
        thread.agent_path.as_ref().map(|value| value.value.as_str()),
        Some("/root/reader")
    );
    assert_eq!(thread.live_lane_count, 1);
    assert_eq!(thread.near_live_lane_count, 1);
    assert_eq!(thread.historical_lane_count, 0);
    assert_eq!(
        thread.current_turn.as_ref().map(|turn| &turn.key),
        Some(&turn_key)
    );
    assert_eq!(
        thread.lifecycle.as_ref().map(|value| value.value),
        Some(ExternalLifecycle::Running)
    );
    assert_eq!(
        thread
            .observed_model
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("gpt-5.6-terra")
    );
    let tokens = thread.token_snapshot.as_ref().expect("tokens");
    assert_eq!(tokens.value.total_tokens, 320);
    assert_eq!(tokens.provenance.temporal_class, SourceTemporalClass::Live);
    assert_eq!(tokens.provenance.source_timestamp_ms, Some(1_100));
    assert_eq!(tokens.provenance.observed_timestamp_ms, 1_105);
    assert_eq!(
        thread
            .authority_provenance
            .as_ref()
            .map(|value| value.temporal_class),
        Some(SourceTemporalClass::Live)
    );
}

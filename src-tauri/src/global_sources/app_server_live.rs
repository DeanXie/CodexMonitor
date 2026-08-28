use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};
use crate::shared::global_sources_core::source_envelope::{
    CodexHomeIdentity, FreshnessEvidence, FreshnessState, SourceKind, SourceTemporalClass,
};
use crate::shared::global_sources_core::source_registry::{
    ExternalLifecycle, SourceLaneUpdate, TokenSnapshot,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn normalize_app_server_live(
    source_instance_id: &str,
    workspace_id: &str,
    codex_home: &CodexHomeIdentity,
    message: &Value,
    observed_timestamp_ms: i64,
) -> Option<SourceLaneUpdate> {
    let method = message.get("method")?.as_str()?;
    let params = message.get("params")?;
    let thread_id = params.get("threadId")?.as_str()?;
    let thread_key = CodexThreadKey::new(codex_home.identity.clone(), thread_id);
    let turn_id = params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| params.get("turn")?.get("id")?.as_str());
    let turn_key = turn_id.map(|turn_id| CodexTurnKey::new(thread_key.clone(), turn_id));
    let (lifecycle, observed_model, token_snapshot) = match method {
        "thread/settings/updated" => (
            None,
            params
                .get("threadSettings")?
                .get("model")?
                .as_str()
                .map(str::to_string),
            None,
        ),
        "turn/started" => (Some(ExternalLifecycle::Running), None, None),
        "turn/completed" => (Some(ExternalLifecycle::Completed), None, None),
        "thread/tokenUsage/updated" => (
            None,
            None,
            Some(parse_token_snapshot(
                params.get("tokenUsage")?.get("total")?,
            )?),
        ),
        _ => return None,
    };
    let source_timestamp_ms = message.get("emittedAtMs").and_then(Value::as_i64);
    let mut digest = Sha256::new();
    digest.update(source_instance_id.as_bytes());
    digest.update([0]);
    digest.update(workspace_id.as_bytes());
    digest.update([0]);
    digest.update(method.as_bytes());
    digest.update([0]);
    digest.update(thread_id.as_bytes());
    digest.update([0]);
    digest.update(serde_json::to_vec(params).ok()?);
    Some(SourceLaneUpdate {
        observation_id: format!("app-server:{:x}", digest.finalize()),
        thread_key,
        turn_key,
        source_kind: SourceKind::MonitorAppServer,
        temporal_class: SourceTemporalClass::Live,
        source_instance_id: source_instance_id.to_string(),
        source_generation: format!("process:{source_instance_id}"),
        source_timestamp_ms,
        observed_timestamp_ms,
        freshness: FreshnessEvidence {
            state: FreshnessState::Fresh,
            last_complete_record_observed_at_ms: Some(observed_timestamp_ms),
            reason: "confirmed app-server notification observed".to_string(),
        },
        lifecycle,
        observed_model,
        token_snapshot,
    })
}

fn parse_token_snapshot(value: &Value) -> Option<TokenSnapshot> {
    Some(TokenSnapshot {
        input_tokens: value.get("inputTokens")?.as_u64()?,
        cached_input_tokens: value.get("cachedInputTokens")?.as_u64()?,
        cache_write_input_tokens: value.get("cacheWriteInputTokens").and_then(Value::as_u64),
        output_tokens: value.get("outputTokens")?.as_u64()?,
        reasoning_output_tokens: value
            .get("reasoningOutputTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: value.get("totalTokens")?.as_u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::global_sources_core::source_envelope::{
        CodexHomeIdentity, SourceKind, SourceTemporalClass,
    };
    use crate::shared::global_sources_core::source_registry::ExternalLifecycle;
    use serde_json::json;

    fn home() -> CodexHomeIdentity {
        CodexHomeIdentity {
            normalized_path: r"C:\fixture\codex-home".to_string(),
            identity: "codex-home:fixture".to_string(),
        }
    }

    #[test]
    fn confirmed_settings_event_produces_live_observed_model_with_server_time() {
        let update = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &home(),
            &json!({
                "emittedAtMs": 1787440105934_i64,
                "method": "thread/settings/updated",
                "params": {
                    "threadId": "thread-paired",
                    "threadSettings": { "model": "gpt-5.6-terra" }
                }
            }),
            1787440106000,
        )
        .expect("confirmed model event");

        assert_eq!(update.thread_key.thread_id, "thread-paired");
        assert_eq!(update.thread_key.codex_home_identity, "codex-home:fixture");
        assert_eq!(update.observed_model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(update.source_kind, SourceKind::MonitorAppServer);
        assert_eq!(update.temporal_class, SourceTemporalClass::Live);
        assert_eq!(update.source_timestamp_ms, Some(1787440105934));
        assert_eq!(update.observed_timestamp_ms, 1787440106000);
    }

    #[test]
    fn confirmed_token_event_uses_total_snapshot_without_adding_cached_or_reasoning() {
        let update = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &home(),
            &json!({
                "emittedAtMs": 1787440111248_i64,
                "method": "thread/tokenUsage/updated",
                "params": {
                    "threadId": "thread-paired",
                    "turnId": "turn-paired",
                    "tokenUsage": { "total": {
                        "inputTokens": 23760,
                        "cachedInputTokens": 6912,
                        "cacheWriteInputTokens": 3,
                        "outputTokens": 18,
                        "reasoningOutputTokens": 5,
                        "totalTokens": 23778
                    }}
                }
            }),
            1787440111300,
        )
        .expect("confirmed token event");

        let tokens = update.token_snapshot.expect("total snapshot");
        assert_eq!(tokens.input_tokens, 23760);
        assert_eq!(tokens.cached_input_tokens, 6912);
        assert_eq!(tokens.reasoning_output_tokens, 5);
        assert_eq!(tokens.total_tokens, 23778);
        assert_eq!(update.turn_key.expect("turn key").turn_id, "turn-paired");
    }

    #[test]
    fn confirmed_turn_events_drive_only_their_observed_lifecycle() {
        let started = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &home(),
            &json!({
                "emittedAtMs": 1787440106008_i64,
                "method": "turn/started",
                "params": { "threadId": "thread-paired", "turn": { "id": "turn-paired" } }
            }),
            1787440106010,
        )
        .expect("turn started");
        let completed = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &home(),
            &json!({
                "emittedAtMs": 1787440111255_i64,
                "method": "turn/completed",
                "params": { "threadId": "thread-paired", "turn": { "id": "turn-paired" } }
            }),
            1787440111260,
        )
        .expect("turn completed");

        assert_eq!(started.lifecycle, Some(ExternalLifecycle::Running));
        assert_eq!(completed.lifecycle, Some(ExternalLifecycle::Completed));
    }

    #[test]
    fn unknown_or_unconfirmed_event_does_not_create_live_evidence() {
        let update = normalize_app_server_live(
            "monitor-process-1",
            "workspace-1",
            &home(),
            &json!({
                "method": "model/rerouted",
                "params": { "threadId": "thread-paired", "model": "gpt-guessed" }
            }),
            1787440106000,
        );

        assert!(update.is_none());
    }
}

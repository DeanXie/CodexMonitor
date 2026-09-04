//! Phase 3.3.3b ingestion for authoritative execution-settings evidence.
//!
//! Request evidence stays pending until a real Thread or Turn identity is
//! returned by app-server. Thread-only settings notifications remain
//! Thread-default snapshots. Rollout observations require the full Turn ID
//! carried by `turn_context`; no time/value/nearest-Turn correlation is used.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

#[cfg(test)]
use super::execution_settings_evidence::SettingEvidence;
use super::execution_settings_evidence::{
    ExecutionSettingField, ExecutionSettingValue, ExecutionSettingsEvidenceLayer,
    ExecutionSettingsEvidenceRecord, ExecutionSettingsEvidenceStore,
    ExecutionSettingsObservationKey, ExecutionSettingsProvenance,
};
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::global_sources_core::rollout_watcher::RolloutTurnContextSettingsObservation;

const MONITOR_REQUEST: &str = "monitor-request";
const APP_SERVER_RESPONSE: &str = "app-server-response";
const APP_SERVER_SETTINGS_NOTIFICATION: &str = "app-server-settings-notification";
const ROLLOUT_TURN_CONTEXT: &str = "rollout-turn-context";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PendingRequestKey {
    codex_home_identity: String,
    request_id: u64,
}

#[derive(Clone)]
struct PendingSettingsRequest {
    method: String,
    params: Value,
    observed_at: u64,
}

#[derive(Default)]
struct ExecutionSettingsEvidenceRuntimeState {
    store: ExecutionSettingsEvidenceStore,
    pending: HashMap<PendingRequestKey, PendingSettingsRequest>,
}

#[derive(Clone, Default)]
pub(crate) struct ExecutionSettingsEvidenceRuntime {
    state: Arc<Mutex<ExecutionSettingsEvidenceRuntimeState>>,
}

impl ExecutionSettingsEvidenceRuntime {
    pub(crate) fn observe_rollout_observations(
        &self,
        observations: impl IntoIterator<Item = RolloutTurnContextSettingsObservation>,
    ) -> usize {
        observations
            .into_iter()
            .filter(|observation| {
                self.observe_rollout_turn_context(
                    observation.thread_key.clone(),
                    &observation.turn_context,
                    &observation.observation_id,
                    observation.observed_timestamp_ms.max(0) as u64,
                )
            })
            .count()
    }

    pub(crate) fn observe_outgoing_request(
        &self,
        codex_home_identity: &str,
        request_id: u64,
        method: &str,
        params: &Value,
        observed_at: u64,
    ) -> bool {
        if !matches!(method, "thread/start" | "turn/start") {
            return false;
        }
        let key = PendingRequestKey {
            codex_home_identity: codex_home_identity.to_string(),
            request_id,
        };
        self.lock().pending.insert(
            key,
            PendingSettingsRequest {
                method: method.to_string(),
                params: params.clone(),
                observed_at,
            },
        );
        true
    }

    pub(crate) fn observe_app_server_response(
        &self,
        codex_home_identity: &str,
        request_id: u64,
        method: &str,
        response: &Value,
        observed_at: u64,
    ) -> bool {
        let pending_key = PendingRequestKey {
            codex_home_identity: codex_home_identity.to_string(),
            request_id,
        };
        let pending = self.lock().pending.remove(&pending_key);
        let Some(pending) = pending.filter(|pending| pending.method == method) else {
            return false;
        };

        match method {
            "turn/start" => {
                let Some(thread_id) = pending.params.get("threadId").and_then(Value::as_str) else {
                    return false;
                };
                let Some(turn_id) = response.pointer("/result/turn/id").and_then(Value::as_str)
                else {
                    return false;
                };
                let key = ExecutionSettingsObservationKey::turn(
                    CodexThreadKey::new(codex_home_identity, thread_id),
                    turn_id,
                );
                let comparison_id = format!("turn:{turn_id}");
                self.observe_correlated_fields(
                    key.clone(),
                    ExecutionSettingsEvidenceLayer::Requested,
                    MONITOR_REQUEST,
                    &comparison_id,
                    &pending.params,
                    pending.observed_at,
                );
                let effective = response.get("result").unwrap_or(response);
                self.observe_correlated_fields(
                    key,
                    ExecutionSettingsEvidenceLayer::ServerEffective,
                    APP_SERVER_RESPONSE,
                    &comparison_id,
                    effective,
                    observed_at,
                );
                true
            }
            "thread/start" => {
                let Some(thread_id) = response
                    .pointer("/result/thread/id")
                    .and_then(Value::as_str)
                else {
                    return false;
                };
                let key = ExecutionSettingsObservationKey::thread_default(CodexThreadKey::new(
                    codex_home_identity,
                    thread_id,
                ));
                let comparison_id = format!("thread-start:{request_id}");
                self.observe_correlated_fields(
                    key.clone(),
                    ExecutionSettingsEvidenceLayer::Requested,
                    MONITOR_REQUEST,
                    &comparison_id,
                    &pending.params,
                    pending.observed_at,
                );
                let effective = response.get("result").unwrap_or(response);
                self.observe_correlated_fields(
                    key,
                    ExecutionSettingsEvidenceLayer::ServerEffective,
                    APP_SERVER_RESPONSE,
                    &comparison_id,
                    effective,
                    observed_at,
                );
                true
            }
            _ => false,
        }
    }

    pub(crate) fn forget_outgoing_request(
        &self,
        codex_home_identity: &str,
        request_id: u64,
    ) -> bool {
        self.lock()
            .pending
            .remove(&PendingRequestKey {
                codex_home_identity: codex_home_identity.to_string(),
                request_id,
            })
            .is_some()
    }

    pub(crate) fn observe_app_server_notification(
        &self,
        codex_home_identity: &str,
        message: &Value,
        observation_id: &str,
        observed_at: u64,
    ) -> bool {
        if message.get("method").and_then(Value::as_str) != Some("thread/settings/updated") {
            return false;
        }
        let Some(params) = message.get("params") else {
            return false;
        };
        let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
            return false;
        };
        let thread_key = CodexThreadKey::new(codex_home_identity, thread_id);
        let (key, comparison_id) = if let Some(turn_id) = params
            .get("turnId")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            (
                ExecutionSettingsObservationKey::turn(thread_key, turn_id),
                format!("turn:{turn_id}"),
            )
        } else {
            (
                ExecutionSettingsObservationKey::thread_default(thread_key),
                format!("thread-settings:{observation_id}"),
            )
        };
        let Some(settings) = params.get("threadSettings") else {
            return false;
        };
        self.observe_correlated_fields(
            key,
            ExecutionSettingsEvidenceLayer::ServerEffective,
            APP_SERVER_SETTINGS_NOTIFICATION,
            &comparison_id,
            settings,
            observed_at,
        ) > 0
    }

    pub(crate) fn observe_rollout_turn_context(
        &self,
        thread_key: CodexThreadKey,
        turn_context: &Value,
        observation_id: &str,
        observed_at: u64,
    ) -> bool {
        let Some(turn_id) = turn_context
            .get("turn_id")
            .or_else(|| turn_context.get("turnId"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return false;
        };
        let key = ExecutionSettingsObservationKey::turn(thread_key, turn_id);
        let comparison_id = format!("turn:{turn_id}");
        let source = format!("{ROLLOUT_TURN_CONTEXT}:{observation_id}");
        self.observe_correlated_fields(
            key,
            ExecutionSettingsEvidenceLayer::PersistedObserved,
            &source,
            &comparison_id,
            turn_context,
            observed_at,
        ) > 0
    }

    pub(crate) fn observe_correlated_fields(
        &self,
        key: ExecutionSettingsObservationKey,
        layer: ExecutionSettingsEvidenceLayer,
        source: &str,
        comparison_id: &str,
        fields: &Value,
        observed_at: u64,
    ) -> usize {
        let records = extract_setting_fields(fields);
        let mut state = self.lock();
        records
            .into_iter()
            .filter(|(field, value)| {
                state.store.observe(
                    key.clone(),
                    *field,
                    ExecutionSettingsEvidenceRecord::new(
                        layer,
                        value.clone(),
                        ExecutionSettingsProvenance::confirmed(source, comparison_id, observed_at),
                    ),
                )
            })
            .count()
    }

    #[cfg(test)]
    pub(crate) fn history(
        &self,
        key: &ExecutionSettingsObservationKey,
        field: ExecutionSettingField,
    ) -> Vec<ExecutionSettingsEvidenceRecord<ExecutionSettingValue>> {
        self.lock().store.history(key, field).to_vec()
    }

    #[cfg(test)]
    pub(crate) fn select(
        &self,
        key: &ExecutionSettingsObservationKey,
        field: ExecutionSettingField,
    ) -> SettingEvidence<ExecutionSettingValue> {
        self.lock().store.select(key, field)
    }

    #[cfg(test)]
    pub(crate) fn pending_request_count(&self) -> usize {
        self.lock().pending.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ExecutionSettingsEvidenceRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn extract_setting_fields(value: &Value) -> Vec<(ExecutionSettingField, ExecutionSettingValue)> {
    let mut fields = Vec::new();
    push_value(
        &mut fields,
        ExecutionSettingField::Model,
        member(value, &["model"]),
    );
    push_value(
        &mut fields,
        ExecutionSettingField::Effort,
        member(value, &["effort", "reasoningEffort", "reasoning_effort"]),
    );
    push_value(
        &mut fields,
        ExecutionSettingField::ApprovalPolicy,
        member(value, &["approvalPolicy", "approval_policy"]),
    );
    push_value(
        &mut fields,
        ExecutionSettingField::Cwd,
        member(value, &["cwd"]),
    );
    push_collaboration_mode(
        &mut fields,
        member(value, &["collaborationMode", "collaboration_mode"]),
    );

    let sandbox = member(value, &["sandboxPolicy", "sandbox_policy", "sandbox"]);
    if let Some(sandbox) = sandbox {
        if sandbox.is_null() {
            fields.push((
                ExecutionSettingField::SandboxPolicy,
                ExecutionSettingValue::Null,
            ));
        } else if let Some(kind) = member(sandbox, &["type"]) {
            push_value(
                &mut fields,
                ExecutionSettingField::SandboxPolicy,
                Some(kind),
            );
        } else if sandbox.is_string() {
            push_value(
                &mut fields,
                ExecutionSettingField::SandboxPolicy,
                Some(sandbox),
            );
        }
    }
    push_value(
        &mut fields,
        ExecutionSettingField::NetworkAccess,
        member(value, &["networkAccess", "network_access"]).or_else(|| {
            sandbox.and_then(|sandbox| member(sandbox, &["networkAccess", "network_access"]))
        }),
    );
    push_string_list(
        &mut fields,
        ExecutionSettingField::WritableRoots,
        member(value, &["writableRoots", "writable_roots"]).or_else(|| {
            sandbox.and_then(|sandbox| member(sandbox, &["writableRoots", "writable_roots"]))
        }),
    );
    fields
}

fn member<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| value.get(*name))
}

fn push_value(
    fields: &mut Vec<(ExecutionSettingField, ExecutionSettingValue)>,
    field: ExecutionSettingField,
    value: Option<&Value>,
) {
    let Some(value) = value else { return };
    let parsed = if value.is_null() {
        Some(ExecutionSettingValue::Null)
    } else if let Some(value) = value.as_str() {
        Some(ExecutionSettingValue::Text(value.to_string()))
    } else {
        value.as_bool().map(ExecutionSettingValue::Bool)
    };
    if let Some(value) = parsed {
        fields.push((field, value));
    }
}

fn push_string_list(
    fields: &mut Vec<(ExecutionSettingField, ExecutionSettingValue)>,
    field: ExecutionSettingField,
    value: Option<&Value>,
) {
    let Some(value) = value else { return };
    if value.is_null() {
        fields.push((field, ExecutionSettingValue::Null));
        return;
    }
    let Some(values) = value.as_array() else {
        return;
    };
    let Some(values) = values.iter().map(Value::as_str).collect::<Option<Vec<_>>>() else {
        return;
    };
    fields.push((
        field,
        ExecutionSettingValue::StringList(values.into_iter().map(str::to_string).collect()),
    ));
}

fn push_collaboration_mode(
    fields: &mut Vec<(ExecutionSettingField, ExecutionSettingValue)>,
    value: Option<&Value>,
) {
    let Some(value) = value else { return };
    let value = if value.is_object() {
        member(value, &["mode"])
    } else {
        Some(value)
    };
    push_value(fields, ExecutionSettingField::CollaborationMode, value);
}

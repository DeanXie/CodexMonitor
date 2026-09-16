//! Provenance for explicit Remote approval decision attempts.
//!
//! This records local admission and dispatch evidence only. It never identifies
//! an approver, client owner, approval owner, or lease, and it never upgrades an
//! app-server response write into evidence that the upstream decision won.

use super::approval_observation::{
    ApprovalObservationSnapshot, ApprovalRequestId, ApprovalRequestIdentity, ApprovalRequestKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ApprovalDecisionAttemptId(String);

impl ApprovalDecisionAttemptId {
    pub(crate) fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalDecisionRemoteProvenance {
    pub remote_transport_generation: String,
    pub transport_request_id: u64,
}

impl ApprovalDecisionRemoteProvenance {
    pub(crate) fn new(
        remote_transport_generation: impl Into<String>,
        transport_request_id: u64,
    ) -> Result<Self, String> {
        let value = Self {
            remote_transport_generation: remote_transport_generation.into(),
            transport_request_id,
        };
        if value.remote_transport_generation.trim().is_empty() {
            return Err("RemoteTransportGeneration is required".to_string());
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalDecisionState {
    NotObserved,
    DecisionPending,
    DecisionDispatched,
    DecisionNotDispatched,
    DecisionOutcomeUnknown,
    DecisionStaleRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalDecisionFailureKind {
    ApprovalNotPending,
    DuplicateDecisionAttempt,
    InvalidResponseSchema,
    TransportLostBeforeDispatch,
    DispatchWriteFailed,
    CancellationBeforeDispatch,
    CancellationAfterDispatch,
    RemoteResponseUnobserved,
    SessionEndedAfterDispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalDecisionResponseKind {
    CommandAccept,
    CommandAcceptForSession,
    CommandDecline,
    CommandCancel,
    CommandExecPolicyAmendment,
    CommandNetworkPolicyAmendment,
    FileAccept,
    FileAcceptForSession,
    FileDecline,
    FileCancel,
    PermissionsGrant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalDecisionAttemptSnapshot {
    pub attempt_id: ApprovalDecisionAttemptId,
    pub workspace_session_generation: String,
    pub app_server_connection_generation: String,
    pub requested_request_id: ApprovalRequestId,
    pub identity: Option<ApprovalRequestIdentity>,
    pub remote_transport_generation: String,
    pub transport_request_id: u64,
    pub state: ApprovalDecisionState,
    pub response_kind: Option<ApprovalDecisionResponseKind>,
    pub observed_at: i64,
    pub dispatched_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub failure_kind: Option<ApprovalDecisionFailureKind>,
    pub resolution_method: Option<String>,
    pub resolution_observed_at: Option<i64>,
    pub dispatch_count: u32,
    pub retry_count: u32,
}

#[derive(Default)]
pub(crate) struct ApprovalDecisionStore {
    attempts: Vec<ApprovalDecisionAttemptSnapshot>,
    admitted: HashMap<ApprovalRequestIdentity, ApprovalDecisionAttemptId>,
}

impl ApprovalDecisionStore {
    pub(crate) fn begin(
        &mut self,
        requested_request_id: ApprovalRequestId,
        pending: Option<&ApprovalObservationSnapshot>,
        result: &Value,
        remote: ApprovalDecisionRemoteProvenance,
        workspace_session_generation: &str,
        app_server_connection_generation: &str,
        observed_at: i64,
    ) -> Result<ApprovalDecisionAttemptSnapshot, String> {
        let attempt_id = ApprovalDecisionAttemptId::generate();
        let identity = pending.map(|snapshot| snapshot.identity.clone());
        let mut snapshot = ApprovalDecisionAttemptSnapshot {
            attempt_id: attempt_id.clone(),
            workspace_session_generation: workspace_session_generation.to_string(),
            app_server_connection_generation: app_server_connection_generation.to_string(),
            requested_request_id,
            identity: identity.clone(),
            remote_transport_generation: remote.remote_transport_generation,
            transport_request_id: remote.transport_request_id,
            state: ApprovalDecisionState::DecisionPending,
            response_kind: None,
            observed_at,
            dispatched_at: None,
            completed_at: None,
            failure_kind: None,
            resolution_method: None,
            resolution_observed_at: None,
            dispatch_count: 0,
            retry_count: 0,
        };
        let Some(pending) = pending else {
            snapshot.state = ApprovalDecisionState::DecisionStaleRejected;
            snapshot.failure_kind = Some(ApprovalDecisionFailureKind::ApprovalNotPending);
            snapshot.completed_at = Some(observed_at);
            self.attempts.push(snapshot);
            return Err(
                "approval decision rejected: exact current approval is not pending".to_string(),
            );
        };
        let response_kind = match validate_approval_decision_response(pending.kind, result) {
            Ok(kind) => kind,
            Err(error) => {
                snapshot.state = ApprovalDecisionState::DecisionNotDispatched;
                snapshot.failure_kind = Some(ApprovalDecisionFailureKind::InvalidResponseSchema);
                snapshot.completed_at = Some(observed_at);
                self.attempts.push(snapshot);
                return Err(error);
            }
        };
        if self.admitted.contains_key(&pending.identity) {
            snapshot.state = ApprovalDecisionState::DecisionStaleRejected;
            snapshot.failure_kind = Some(ApprovalDecisionFailureKind::DuplicateDecisionAttempt);
            snapshot.completed_at = Some(observed_at);
            self.attempts.push(snapshot);
            return Err("approval decision rejected: an attempt is already admitted".to_string());
        }
        snapshot.response_kind = Some(response_kind);
        self.admitted.insert(pending.identity.clone(), attempt_id);
        self.attempts.push(snapshot.clone());
        Ok(snapshot)
    }

    pub(crate) fn attempts(&self) -> Vec<ApprovalDecisionAttemptSnapshot> {
        self.attempts.clone()
    }

    pub(crate) fn record_dispatched(
        &mut self,
        attempt_id: &ApprovalDecisionAttemptId,
        observed_at: i64,
    ) -> Result<(), String> {
        let attempt = self.find_mut(attempt_id)?;
        attempt.dispatch_count = 1;
        attempt.dispatched_at = Some(observed_at);
        if attempt.state == ApprovalDecisionState::DecisionNotDispatched {
            attempt.state = ApprovalDecisionState::DecisionOutcomeUnknown;
            attempt.failure_kind = Some(ApprovalDecisionFailureKind::RemoteResponseUnobserved);
        } else if attempt.state != ApprovalDecisionState::DecisionOutcomeUnknown {
            attempt.state = ApprovalDecisionState::DecisionDispatched;
            attempt.failure_kind = None;
        }
        Ok(())
    }

    pub(crate) fn record_not_dispatched(
        &mut self,
        attempt_id: &ApprovalDecisionAttemptId,
        failure_kind: ApprovalDecisionFailureKind,
        observed_at: i64,
    ) -> Result<(), String> {
        let attempt = self.find_mut(attempt_id)?;
        if attempt.dispatch_count > 0 || attempt.dispatched_at.is_some() {
            attempt.state = ApprovalDecisionState::DecisionOutcomeUnknown;
        } else {
            attempt.state = ApprovalDecisionState::DecisionNotDispatched;
        }
        attempt.failure_kind = Some(failure_kind);
        attempt.completed_at = Some(observed_at);
        Ok(())
    }

    pub(crate) fn record_outcome_unknown(
        &mut self,
        attempt_id: &ApprovalDecisionAttemptId,
        failure_kind: ApprovalDecisionFailureKind,
        observed_at: i64,
    ) -> Result<(), String> {
        let attempt = self.find_mut(attempt_id)?;
        attempt.state = ApprovalDecisionState::DecisionOutcomeUnknown;
        attempt.failure_kind = Some(failure_kind);
        attempt.completed_at = Some(observed_at);
        Ok(())
    }

    pub(crate) fn record_resolution(
        &mut self,
        identity: &ApprovalRequestIdentity,
        method: &str,
        observed_at: i64,
    ) {
        for attempt in &mut self.attempts {
            if attempt.identity.as_ref() == Some(identity) {
                attempt.resolution_method = Some(method.to_string());
                attempt.resolution_observed_at = Some(observed_at);
            }
        }
    }

    pub(crate) fn record_session_ended(&mut self, observed_at: i64) {
        for attempt in &mut self.attempts {
            if attempt.state == ApprovalDecisionState::DecisionDispatched
                && attempt.resolution_observed_at.is_none()
            {
                attempt.state = ApprovalDecisionState::DecisionOutcomeUnknown;
                attempt.failure_kind = Some(ApprovalDecisionFailureKind::SessionEndedAfterDispatch);
                attempt.completed_at = Some(observed_at);
            }
        }
    }

    fn find_mut(
        &mut self,
        attempt_id: &ApprovalDecisionAttemptId,
    ) -> Result<&mut ApprovalDecisionAttemptSnapshot, String> {
        self.attempts
            .iter_mut()
            .find(|attempt| &attempt.attempt_id == attempt_id)
            .ok_or_else(|| "approval decision attempt not found".to_string())
    }
}

pub(crate) fn validate_approval_decision_response(
    kind: ApprovalRequestKind,
    result: &Value,
) -> Result<ApprovalDecisionResponseKind, String> {
    match kind {
        ApprovalRequestKind::CommandExecution => validate_command_response(result),
        ApprovalRequestKind::FileChange => validate_file_response(result),
        ApprovalRequestKind::Permissions => validate_permissions_response(result),
    }
}

pub(crate) fn looks_like_approval_decision_response(result: &Value) -> bool {
    result.as_object().is_some_and(|object| {
        object.contains_key("decision")
            || object.contains_key("permissions")
            || object.contains_key("scope")
            || object.contains_key("strictAutoReview")
    })
}

fn exact_object<'a>(
    value: &'a Value,
    allowed: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("invalid {label}: expected object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("invalid {label}: unknown field"));
    }
    Ok(object)
}

fn validate_command_response(result: &Value) -> Result<ApprovalDecisionResponseKind, String> {
    let object = exact_object(result, &["decision"], "command approval response")?;
    if object.len() != 1 {
        return Err("invalid command approval response: decision is required".to_string());
    }
    let decision = object.get("decision").expect("checked decision field");
    if let Some(decision) = decision.as_str() {
        return match decision {
            "accept" => Ok(ApprovalDecisionResponseKind::CommandAccept),
            "acceptForSession" => Ok(ApprovalDecisionResponseKind::CommandAcceptForSession),
            "decline" => Ok(ApprovalDecisionResponseKind::CommandDecline),
            "cancel" => Ok(ApprovalDecisionResponseKind::CommandCancel),
            _ => Err("invalid command approval decision".to_string()),
        };
    }
    let tagged = exact_object(
        decision,
        &[
            "acceptWithExecpolicyAmendment",
            "applyNetworkPolicyAmendment",
        ],
        "command approval decision",
    )?;
    if tagged.len() != 1 {
        return Err("invalid command approval decision: expected one tagged variant".to_string());
    }
    if let Some(payload) = tagged.get("acceptWithExecpolicyAmendment") {
        let payload = exact_object(payload, &["execpolicy_amendment"], "execpolicy amendment")?;
        let command = payload
            .get("execpolicy_amendment")
            .and_then(Value::as_array)
            .ok_or_else(|| "invalid execpolicy amendment".to_string())?;
        if command.iter().any(|value| value.as_str().is_none()) {
            return Err("invalid execpolicy amendment command".to_string());
        }
        return Ok(ApprovalDecisionResponseKind::CommandExecPolicyAmendment);
    }
    let payload = tagged
        .get("applyNetworkPolicyAmendment")
        .expect("known network amendment variant");
    let payload = exact_object(
        payload,
        &["network_policy_amendment"],
        "network policy amendment",
    )?;
    let amendment = payload
        .get("network_policy_amendment")
        .ok_or_else(|| "invalid network policy amendment".to_string())?;
    let amendment = exact_object(amendment, &["host", "action"], "network policy amendment")?;
    let host = amendment
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let action = amendment
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if host.trim().is_empty() || !matches!(action, "allow" | "deny") {
        return Err("invalid network policy amendment".to_string());
    }
    Ok(ApprovalDecisionResponseKind::CommandNetworkPolicyAmendment)
}

fn validate_file_response(result: &Value) -> Result<ApprovalDecisionResponseKind, String> {
    let object = exact_object(result, &["decision"], "file approval response")?;
    if object.len() != 1 {
        return Err("invalid file approval response: decision is required".to_string());
    }
    match object.get("decision").and_then(Value::as_str) {
        Some("accept") => Ok(ApprovalDecisionResponseKind::FileAccept),
        Some("acceptForSession") => Ok(ApprovalDecisionResponseKind::FileAcceptForSession),
        Some("decline") => Ok(ApprovalDecisionResponseKind::FileDecline),
        Some("cancel") => Ok(ApprovalDecisionResponseKind::FileCancel),
        _ => Err("invalid file approval decision".to_string()),
    }
}

fn validate_permissions_response(result: &Value) -> Result<ApprovalDecisionResponseKind, String> {
    let object = exact_object(
        result,
        &["permissions", "scope", "strictAutoReview"],
        "permissions approval response",
    )?;
    let permissions = object.get("permissions").ok_or_else(|| {
        "invalid permissions approval response: permissions is required".to_string()
    })?;
    let permissions = exact_object(
        permissions,
        &["network", "fileSystem"],
        "granted permissions",
    )?;
    if let Some(network) = permissions.get("network") {
        let network = exact_object(network, &["enabled"], "network permissions")?;
        if network
            .get("enabled")
            .is_some_and(|value| !value.is_boolean() && !value.is_null())
        {
            return Err("invalid network permissions".to_string());
        }
    }
    if let Some(file_system) = permissions.get("fileSystem") {
        validate_file_system_permissions(file_system)?;
    }
    let scope = object.get("scope").and_then(Value::as_str);
    if !matches!(scope, Some("turn" | "session")) {
        return Err("invalid permissions approval scope".to_string());
    }
    if object
        .get("strictAutoReview")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err("invalid strictAutoReview value".to_string());
    }
    if scope == Some("session") && object.get("strictAutoReview") == Some(&Value::Bool(true)) {
        return Err("strictAutoReview cannot be granted for session scope".to_string());
    }
    Ok(ApprovalDecisionResponseKind::PermissionsGrant)
}

fn validate_file_system_permissions(value: &Value) -> Result<(), String> {
    let object = exact_object(
        value,
        &["read", "write", "globScanMaxDepth", "entries"],
        "file-system permissions",
    )?;
    for key in ["read", "write"] {
        if let Some(value) = object.get(key) {
            if !value.is_null()
                && value
                    .as_array()
                    .is_none_or(|items| items.iter().any(|item| item.as_str().is_none()))
            {
                return Err(format!("invalid file-system {key} permissions"));
            }
        }
    }
    if object
        .get("globScanMaxDepth")
        .is_some_and(|value| !value.is_null() && value.as_u64().is_none_or(|depth| depth == 0))
    {
        return Err("invalid file-system globScanMaxDepth".to_string());
    }
    if object
        .get("entries")
        .is_some_and(|value| !value.is_null() && value.as_array().is_none())
    {
        return Err("invalid file-system entries".to_string());
    }
    Ok(())
}

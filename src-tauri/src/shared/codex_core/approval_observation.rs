//! WorkspaceSession/app-server-generation-scoped approval request evidence.
//!
//! This module observes upstream request and lifecycle facts. It neither makes
//! approval decisions nor identifies an approver, owner, lease, or Remote client.

#![cfg_attr(not(test), allow(dead_code))]

use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

const COMMAND_APPROVAL_METHOD: &str = "item/commandExecution/requestApproval";
const FILE_CHANGE_APPROVAL_METHOD: &str = "item/fileChange/requestApproval";
const PERMISSIONS_APPROVAL_METHOD: &str = "item/permissions/requestApproval";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ApprovalRequestId {
    Number(i64),
    String(String),
}

impl ApprovalRequestId {
    fn from_value(value: &Value) -> Option<Self> {
        value
            .as_i64()
            .map(Self::Number)
            .or_else(|| value.as_str().map(|value| Self::String(value.to_string())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalObservationState {
    NotObserved,
    Pending,
    ResolvedOrCleared,
    SessionEndedUnresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalRequestKind {
    CommandExecution,
    FileChange,
    Permissions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalSessionEndEvidenceKind {
    AppServerProcessExited,
    AppServerProcessTerminated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalRequestIdentity {
    pub workspace_session_generation: String,
    pub app_server_connection_generation: String,
    pub request_id: ApprovalRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalAutoReviewEvidence {
    pub phase: String,
    pub status: String,
    pub target_item_id: String,
    pub observed_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalObservationSnapshot {
    pub identity: ApprovalRequestIdentity,
    pub kind: ApprovalRequestKind,
    pub state: ApprovalObservationState,
    pub method: String,
    pub approval_id: Option<String>,
    pub decision_options: Vec<String>,
    pub observed_at: i64,
    pub resolution_method: Option<String>,
    pub completed_item_status: Option<String>,
    pub session_end_kind: Option<ApprovalSessionEndEvidenceKind>,
    pub auto_review: Option<ApprovalAutoReviewEvidence>,
}

#[derive(Default)]
struct ApprovalObservationStore {
    pending: HashMap<(ApprovalRequestId, String), ApprovalObservationSnapshot>,
    history: Vec<ApprovalObservationSnapshot>,
}

pub(crate) struct ApprovalObservationRuntime {
    workspace_session_generation: WorkspaceSessionGeneration,
    app_server_connection_generation: AppServerConnectionGeneration,
    store: Mutex<ApprovalObservationStore>,
}

impl Default for ApprovalObservationRuntime {
    fn default() -> Self {
        Self::new(
            WorkspaceSessionGeneration::new(uuid::Uuid::new_v4().to_string())
                .expect("UUID workspace session generation"),
            AppServerConnectionGeneration::new(uuid::Uuid::new_v4().to_string())
                .expect("UUID app-server connection generation"),
        )
    }
}

impl ApprovalObservationRuntime {
    pub(crate) fn new(
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self {
            workspace_session_generation,
            app_server_connection_generation,
            store: Mutex::new(ApprovalObservationStore::default()),
        }
    }

    pub(crate) fn workspace_session_generation(&self) -> &WorkspaceSessionGeneration {
        &self.workspace_session_generation
    }

    pub(crate) fn app_server_connection_generation(&self) -> &AppServerConnectionGeneration {
        &self.app_server_connection_generation
    }

    pub(crate) fn observe_request(
        &self,
        message: &Value,
        observed_at: i64,
    ) -> Result<ApprovalObservationSnapshot, String> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| "approval request method is required".to_string())?;
        let kind = request_kind(method)
            .ok_or_else(|| format!("unsupported approval request method: {method}"))?;
        let request_id = message
            .get("id")
            .and_then(ApprovalRequestId::from_value)
            .ok_or_else(|| "approval request id is required".to_string())?;
        let params = message
            .get("params")
            .and_then(Value::as_object)
            .ok_or_else(|| "approval request params are required".to_string())?;
        let thread_id = required_string(params.get("threadId"), "threadId")?;
        let turn_id = required_string(params.get("turnId"), "turnId")?;
        let item_id = params
            .get("itemId")
            .or_else(|| params.get("item").and_then(|item| item.get("id")))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| "approval request itemId is required".to_string())?;
        let approval_id = params
            .get("approvalId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let decision_options = params
            .get("availableDecisions")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let snapshot = ApprovalObservationSnapshot {
            identity: ApprovalRequestIdentity {
                workspace_session_generation: self
                    .workspace_session_generation
                    .as_str()
                    .to_string(),
                app_server_connection_generation: self
                    .app_server_connection_generation
                    .as_str()
                    .to_string(),
                request_id: request_id.clone(),
                thread_id: thread_id.clone(),
                turn_id,
                item_id,
            },
            kind,
            state: ApprovalObservationState::Pending,
            method: method.to_string(),
            approval_id,
            decision_options,
            observed_at,
            resolution_method: None,
            completed_item_status: None,
            session_end_kind: None,
            auto_review: None,
        };
        self.store
            .lock()
            .expect("approval observation store")
            .pending
            .insert((request_id, thread_id), snapshot.clone());
        Ok(snapshot)
    }

    pub(crate) fn record_server_request_resolved(
        &self,
        workspace_generation: &WorkspaceSessionGeneration,
        connection_generation: &AppServerConnectionGeneration,
        message: &Value,
        observed_at: i64,
    ) -> bool {
        if !self.is_current_generation(workspace_generation, connection_generation) {
            return false;
        }
        let params = match message.get("params") {
            Some(params) => params,
            None => return false,
        };
        let request_id = match params
            .get("requestId")
            .and_then(ApprovalRequestId::from_value)
        {
            Some(value) => value,
            None => return false,
        };
        let thread_id = match params.get("threadId").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => value,
            _ => return false,
        };
        self.resolve_exact(
            &(request_id, thread_id.to_string()),
            "serverRequest/resolved",
            None,
            observed_at,
        )
    }

    pub(crate) fn record_item_completed(
        &self,
        workspace_generation: &WorkspaceSessionGeneration,
        connection_generation: &AppServerConnectionGeneration,
        message: &Value,
        observed_at: i64,
    ) -> bool {
        if !self.is_current_generation(workspace_generation, connection_generation) {
            return false;
        }
        let params = match message.get("params") {
            Some(params) => params,
            None => return false,
        };
        let request_id = match params
            .get("requestId")
            .and_then(ApprovalRequestId::from_value)
        {
            Some(value) => value,
            None => return false,
        };
        let thread_id = match params.get("threadId").and_then(Value::as_str) {
            Some(value) => value,
            None => return false,
        };
        let turn_id = match params.get("turnId").and_then(Value::as_str) {
            Some(value) => value,
            None => return false,
        };
        let item = match params.get("item") {
            Some(value) => value,
            None => return false,
        };
        let item_id = match item.get("id").and_then(Value::as_str) {
            Some(value) => value,
            None => return false,
        };
        let status = item
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string);
        let key = (request_id, thread_id.to_string());
        let mut store = self.store.lock().expect("approval observation store");
        let is_exact = store.pending.get(&key).is_some_and(|snapshot| {
            snapshot.identity.turn_id == turn_id && snapshot.identity.item_id == item_id
        });
        if !is_exact {
            return false;
        }
        let mut snapshot = store.pending.remove(&key).expect("exact pending request");
        snapshot.state = ApprovalObservationState::ResolvedOrCleared;
        snapshot.observed_at = observed_at;
        snapshot.resolution_method = Some("item/completed".to_string());
        snapshot.completed_item_status = status;
        store.history.push(snapshot);
        true
    }

    pub(crate) fn record_auto_review(
        &self,
        workspace_generation: &WorkspaceSessionGeneration,
        connection_generation: &AppServerConnectionGeneration,
        message: &Value,
        observed_at: i64,
    ) -> bool {
        if !self.is_current_generation(workspace_generation, connection_generation) {
            return false;
        }
        let params = match message.get("params") {
            Some(params) => params,
            None => return false,
        };
        let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
            return false;
        };
        let Some(turn_id) = params.get("turnId").and_then(Value::as_str) else {
            return false;
        };
        let Some(target_item_id) = params.get("targetItemId").and_then(Value::as_str) else {
            return false;
        };
        let phase = match message.get("method").and_then(Value::as_str) {
            Some("item/autoApprovalReview/started") => "started",
            Some("item/autoApprovalReview/completed") => "completed",
            _ => return false,
        };
        let status = params
            .pointer("/review/status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let evidence = ApprovalAutoReviewEvidence {
            phase: phase.to_string(),
            status: status.to_string(),
            target_item_id: target_item_id.to_string(),
            observed_at,
        };
        let mut store = self.store.lock().expect("approval observation store");
        let mut matched = false;
        for snapshot in store.pending.values_mut() {
            if snapshot.identity.thread_id == thread_id
                && snapshot.identity.turn_id == turn_id
                && snapshot.identity.item_id == target_item_id
            {
                snapshot.auto_review = Some(evidence.clone());
                matched = true;
            }
        }
        matched
    }

    pub(crate) fn record_session_ended(
        &self,
        kind: ApprovalSessionEndEvidenceKind,
        _diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> usize {
        let mut store = self.store.lock().expect("approval observation store");
        let pending = std::mem::take(&mut store.pending);
        let count = pending.len();
        for (_, mut snapshot) in pending {
            snapshot.state = ApprovalObservationState::SessionEndedUnresolved;
            snapshot.observed_at = observed_at;
            snapshot.session_end_kind = Some(kind);
            store.history.push(snapshot);
        }
        count
    }

    pub(crate) fn current_pending(&self) -> Vec<ApprovalObservationSnapshot> {
        self.store
            .lock()
            .expect("approval observation store")
            .pending
            .values()
            .cloned()
            .collect()
    }

    pub(crate) fn history(&self) -> Vec<ApprovalObservationSnapshot> {
        self.store
            .lock()
            .expect("approval observation store")
            .history
            .clone()
    }

    pub(crate) fn record_local_allowlist_non_transition(&self) {}

    pub(crate) fn record_remote_transport_non_transition(&self) {}

    fn is_current_generation(
        &self,
        workspace_generation: &WorkspaceSessionGeneration,
        connection_generation: &AppServerConnectionGeneration,
    ) -> bool {
        workspace_generation == &self.workspace_session_generation
            && connection_generation == &self.app_server_connection_generation
    }

    fn resolve_exact(
        &self,
        key: &(ApprovalRequestId, String),
        method: &str,
        status: Option<String>,
        observed_at: i64,
    ) -> bool {
        let mut store = self.store.lock().expect("approval observation store");
        let Some(mut snapshot) = store.pending.remove(key) else {
            return false;
        };
        snapshot.state = ApprovalObservationState::ResolvedOrCleared;
        snapshot.observed_at = observed_at;
        snapshot.resolution_method = Some(method.to_string());
        snapshot.completed_item_status = status;
        store.history.push(snapshot);
        true
    }
}

pub(crate) fn reconcile_approval_observation_message(
    runtime: &ApprovalObservationRuntime,
    message: &Value,
    observed_at: i64,
) {
    let method = message.get("method").and_then(Value::as_str);
    match method {
        Some(
            COMMAND_APPROVAL_METHOD | FILE_CHANGE_APPROVAL_METHOD | PERMISSIONS_APPROVAL_METHOD,
        ) => {
            let _ = runtime.observe_request(message, observed_at);
        }
        Some("serverRequest/resolved") => {
            runtime.record_server_request_resolved(
                runtime.workspace_session_generation(),
                runtime.app_server_connection_generation(),
                message,
                observed_at,
            );
        }
        Some("item/completed") => {
            runtime.record_item_completed(
                runtime.workspace_session_generation(),
                runtime.app_server_connection_generation(),
                message,
                observed_at,
            );
        }
        Some("item/autoApprovalReview/started" | "item/autoApprovalReview/completed") => {
            runtime.record_auto_review(
                runtime.workspace_session_generation(),
                runtime.app_server_connection_generation(),
                message,
                observed_at,
            );
        }
        _ => {}
    }
}

fn request_kind(method: &str) -> Option<ApprovalRequestKind> {
    match method {
        COMMAND_APPROVAL_METHOD => Some(ApprovalRequestKind::CommandExecution),
        FILE_CHANGE_APPROVAL_METHOD => Some(ApprovalRequestKind::FileChange),
        PERMISSIONS_APPROVAL_METHOD => Some(ApprovalRequestKind::Permissions),
        _ => None,
    }
}

fn required_string(value: Option<&Value>, name: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("approval request {name} is required"))
}

//! Exact-ID deletion evidence for one WorkspaceSession/app-server generation.
//!
//! `thread/delete` remains the only mutation authority. This module records
//! direct upstream evidence and gates local tombstone reconciliation; it never
//! infers deletion from projection absence, runtime unload, transport loss, or
//! a missing rollout.

use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::RemoteHostIdentity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const ACTIVE_WRITER_ERROR_CODE: i64 = -32600;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct DeleteAttemptId(String);

impl DeleteAttemptId {
    pub(crate) fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteMutationState {
    NotObserved,
    DeletePending,
    DeleteConfirmed,
    DeleteRejected,
    DeleteOutcomeUnknown,
    SessionEndedOutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteMutationFailureKind {
    BlockedByActiveWriter,
    UpstreamRejected,
    Timeout,
    ResponseLost,
    DispatchDisconnected,
    Cancellation,
    MalformedResponse,
    SessionEndedAfterDispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeleteNonTransitionEvent {
    RolloutMissing,
    ThreadClosed,
    ThreadNotLoaded,
    UiProjectionRemoved,
    SidebarRemoved,
    CatalogRemoved,
    RemoteTcpDisconnected,
    RemoteClientClosed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteDirectEvidenceKind {
    ThreadDeleteResponse,
    ThreadDeletedNotification,
    UpstreamError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteMutationObservation {
    pub attempt_id: DeleteAttemptId,
    pub remote_host_identity: RemoteHostIdentity,
    pub thread_key: CodexThreadKey,
    pub workspace_session_generation: String,
    pub app_server_connection_generation: String,
    pub requested_full_thread_id: String,
    pub state: DeleteMutationState,
    pub observed_at: i64,
    pub dispatched_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub dispatch_count: u32,
    pub retry_count: u32,
    pub direct_evidence: Option<DeleteDirectEvidenceKind>,
    pub upstream_error_code: Option<i64>,
    pub failure_kind: Option<DeleteMutationFailureKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfirmedDeleteTombstone {
    pub attempt_id: DeleteAttemptId,
    pub thread_key: CodexThreadKey,
    pub remote_host_identity: RemoteHostIdentity,
    pub confirmed_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeleteMutationTransitionError {
    ExactThreadKeyRequired,
    FullThreadIdRequired,
    ThreadIdentityMismatch,
    WorkspaceSessionGenerationMismatch,
    AppServerConnectionGenerationMismatch,
    AttemptNotFound,
    InvalidTransition,
    DirectEvidenceMismatch,
}

#[derive(Clone)]
pub(crate) struct DeleteMutationObservationRuntime {
    workspace_session_generation: WorkspaceSessionGeneration,
    app_server_connection_generation: AppServerConnectionGeneration,
    attempts: Arc<Mutex<HashMap<DeleteAttemptId, DeleteMutationObservation>>>,
}

impl Default for DeleteMutationObservationRuntime {
    fn default() -> Self {
        Self::new(
            WorkspaceSessionGeneration::new(uuid::Uuid::new_v4().to_string())
                .expect("UUID workspace session generation"),
            AppServerConnectionGeneration::new(uuid::Uuid::new_v4().to_string())
                .expect("UUID app-server connection generation"),
        )
    }
}

impl DeleteMutationObservationRuntime {
    pub(crate) fn new(
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self {
            workspace_session_generation,
            app_server_connection_generation,
            attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn workspace_session_generation(&self) -> &WorkspaceSessionGeneration {
        &self.workspace_session_generation
    }

    pub(crate) fn app_server_connection_generation(&self) -> &AppServerConnectionGeneration {
        &self.app_server_connection_generation
    }

    pub(crate) fn begin_delete(
        &self,
        remote_host_identity: RemoteHostIdentity,
        thread_key: CodexThreadKey,
        requested_full_thread_id: &str,
        expected_workspace_session_generation: &str,
        expected_app_server_connection_generation: &str,
        observed_at: i64,
    ) -> Result<DeleteAttemptId, DeleteMutationTransitionError> {
        validate_exact_thread_key(&thread_key, requested_full_thread_id)?;
        if expected_workspace_session_generation != self.workspace_session_generation.as_str() {
            return Err(DeleteMutationTransitionError::WorkspaceSessionGenerationMismatch);
        }
        if expected_app_server_connection_generation
            != self.app_server_connection_generation.as_str()
        {
            return Err(DeleteMutationTransitionError::AppServerConnectionGenerationMismatch);
        }
        let attempt_id = DeleteAttemptId::generate();
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                attempt_id.clone(),
                DeleteMutationObservation {
                    attempt_id: attempt_id.clone(),
                    remote_host_identity,
                    thread_key,
                    workspace_session_generation: self
                        .workspace_session_generation
                        .as_str()
                        .to_string(),
                    app_server_connection_generation: self
                        .app_server_connection_generation
                        .as_str()
                        .to_string(),
                    requested_full_thread_id: requested_full_thread_id.to_string(),
                    state: DeleteMutationState::DeletePending,
                    observed_at,
                    dispatched_at: None,
                    completed_at: None,
                    dispatch_count: 0,
                    retry_count: 0,
                    direct_evidence: None,
                    upstream_error_code: None,
                    failure_kind: None,
                },
            );
        Ok(attempt_id)
    }

    pub(crate) fn snapshot(
        &self,
        attempt_id: &DeleteAttemptId,
    ) -> Option<DeleteMutationObservation> {
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(attempt_id)
            .cloned()
    }

    pub(crate) fn latest_for_thread(
        &self,
        thread_key: &CodexThreadKey,
    ) -> Option<DeleteMutationObservation> {
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|observation| &observation.thread_key == thread_key)
            .max_by_key(|observation| observation.observed_at)
            .cloned()
    }

    pub(crate) fn record_dispatched(
        &self,
        attempt_id: &DeleteAttemptId,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.update(attempt_id, |observation| {
            if observation.dispatch_count == 1 {
                return Ok(());
            }
            if observation.state != DeleteMutationState::DeletePending {
                return Err(DeleteMutationTransitionError::InvalidTransition);
            }
            observation.dispatch_count = 1;
            observation.dispatched_at = Some(observed_at);
            observation.observed_at = observed_at;
            Ok(())
        })
    }

    pub(crate) fn record_response(
        &self,
        attempt_id: &DeleteAttemptId,
        response: &Value,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.update(attempt_id, |observation| {
            if observation.state == DeleteMutationState::DeleteConfirmed {
                return Ok(());
            }
            ensure_dispatched(observation)?;
            if response
                .get("result")
                .and_then(Value::as_object)
                .is_some_and(|result| result.is_empty())
                && response.get("error").is_none()
            {
                confirm(
                    observation,
                    DeleteDirectEvidenceKind::ThreadDeleteResponse,
                    observed_at,
                );
                return Ok(());
            }
            if let Some(error) = response.get("error").and_then(Value::as_object) {
                let code = error.get("code").and_then(Value::as_i64);
                let active_writer = code == Some(ACTIVE_WRITER_ERROR_CODE)
                    && error
                        .get("message")
                        .and_then(Value::as_str)
                        .is_some_and(|message| {
                            message.to_ascii_lowercase().contains("active writer")
                        });
                observation.state = DeleteMutationState::DeleteRejected;
                observation.direct_evidence = Some(DeleteDirectEvidenceKind::UpstreamError);
                observation.upstream_error_code = code;
                observation.failure_kind = Some(if active_writer {
                    DeleteMutationFailureKind::BlockedByActiveWriter
                } else {
                    DeleteMutationFailureKind::UpstreamRejected
                });
                observation.observed_at = observed_at;
                observation.completed_at = Some(observed_at);
                return Ok(());
            }
            observation.state = DeleteMutationState::DeleteOutcomeUnknown;
            observation.failure_kind = Some(DeleteMutationFailureKind::MalformedResponse);
            observation.observed_at = observed_at;
            observation.completed_at = Some(observed_at);
            Ok(())
        })
    }

    pub(crate) fn record_thread_deleted(
        &self,
        remote_host_identity: &RemoteHostIdentity,
        workspace_session_generation: &str,
        app_server_connection_generation: &str,
        full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        if workspace_session_generation != self.workspace_session_generation.as_str() {
            return Err(DeleteMutationTransitionError::WorkspaceSessionGenerationMismatch);
        }
        if app_server_connection_generation != self.app_server_connection_generation.as_str() {
            return Err(DeleteMutationTransitionError::AppServerConnectionGenerationMismatch);
        }
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(observation) = attempts
            .values_mut()
            .filter(|observation| {
                &observation.remote_host_identity == remote_host_identity
                    && observation.requested_full_thread_id == full_thread_id
            })
            .max_by_key(|observation| observation.observed_at)
        else {
            return Err(DeleteMutationTransitionError::DirectEvidenceMismatch);
        };
        if observation.dispatch_count != 1 {
            return Err(DeleteMutationTransitionError::InvalidTransition);
        }
        confirm(
            observation,
            DeleteDirectEvidenceKind::ThreadDeletedNotification,
            observed_at,
        );
        Ok(())
    }

    pub(crate) fn record_outcome_unknown(
        &self,
        attempt_id: &DeleteAttemptId,
        failure_kind: DeleteMutationFailureKind,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.update(attempt_id, |observation| {
            ensure_dispatched(observation)?;
            if observation.state == DeleteMutationState::DeleteConfirmed {
                return Ok(());
            }
            if observation.state == DeleteMutationState::DeleteOutcomeUnknown {
                return Ok(());
            }
            observation.state = DeleteMutationState::DeleteOutcomeUnknown;
            observation.failure_kind = Some(failure_kind);
            observation.observed_at = observed_at;
            observation.completed_at = Some(observed_at);
            Ok(())
        })
    }

    pub(crate) fn record_not_dispatched(
        &self,
        attempt_id: &DeleteAttemptId,
        failure_kind: DeleteMutationFailureKind,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.update(attempt_id, |observation| {
            if observation.dispatch_count != 0
                || observation.state != DeleteMutationState::DeletePending
            {
                return Err(DeleteMutationTransitionError::InvalidTransition);
            }
            observation.state = DeleteMutationState::DeleteRejected;
            observation.failure_kind = Some(failure_kind);
            observation.observed_at = observed_at;
            observation.completed_at = Some(observed_at);
            Ok(())
        })
    }

    pub(crate) fn record_current_thread_deleted(
        &self,
        full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(observation) = attempts
            .values_mut()
            .filter(|observation| observation.requested_full_thread_id == full_thread_id)
            .max_by_key(|observation| observation.observed_at)
        else {
            return Err(DeleteMutationTransitionError::DirectEvidenceMismatch);
        };
        if observation.dispatch_count != 1 {
            return Err(DeleteMutationTransitionError::InvalidTransition);
        }
        confirm(
            observation,
            DeleteDirectEvidenceKind::ThreadDeletedNotification,
            observed_at,
        );
        Ok(())
    }

    pub(crate) fn record_session_ended(&self, observed_at: i64) -> usize {
        let mut changed = 0;
        for observation in self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values_mut()
        {
            if observation.dispatch_count == 1
                && matches!(
                    observation.state,
                    DeleteMutationState::DeletePending | DeleteMutationState::DeleteOutcomeUnknown
                )
            {
                observation.state = DeleteMutationState::SessionEndedOutcomeUnknown;
                observation.failure_kind =
                    Some(DeleteMutationFailureKind::SessionEndedAfterDispatch);
                observation.observed_at = observed_at;
                observation.completed_at = Some(observed_at);
                changed += 1;
            }
        }
        changed
    }

    pub(crate) fn observe_non_transition(
        &self,
        attempt_id: &DeleteAttemptId,
        _event: DeleteNonTransitionEvent,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.snapshot(attempt_id)
            .map(|_| ())
            .ok_or(DeleteMutationTransitionError::AttemptNotFound)
    }

    pub(crate) fn confirmed_tombstone(
        &self,
        attempt_id: &DeleteAttemptId,
    ) -> Option<ConfirmedDeleteTombstone> {
        let observation = self.snapshot(attempt_id)?;
        (observation.state == DeleteMutationState::DeleteConfirmed).then(|| {
            ConfirmedDeleteTombstone {
                attempt_id: observation.attempt_id,
                thread_key: observation.thread_key,
                remote_host_identity: observation.remote_host_identity,
                confirmed_at: observation.completed_at.unwrap_or(observation.observed_at),
            }
        })
    }

    pub(crate) fn dispatch_count(&self) -> u32 {
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .map(|observation| observation.dispatch_count)
            .sum()
    }

    pub(crate) fn tombstone_count(&self) -> usize {
        self.attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|observation| observation.state == DeleteMutationState::DeleteConfirmed)
            .map(|observation| &observation.thread_key)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    fn update(
        &self,
        attempt_id: &DeleteAttemptId,
        update: impl FnOnce(&mut DeleteMutationObservation) -> Result<(), DeleteMutationTransitionError>,
    ) -> Result<(), DeleteMutationTransitionError> {
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        update(
            attempts
                .get_mut(attempt_id)
                .ok_or(DeleteMutationTransitionError::AttemptNotFound)?,
        )
    }
}

fn validate_exact_thread_key(
    thread_key: &CodexThreadKey,
    requested_full_thread_id: &str,
) -> Result<(), DeleteMutationTransitionError> {
    if thread_key.codex_home_identity.trim().is_empty() || thread_key.thread_id.trim().is_empty() {
        return Err(DeleteMutationTransitionError::ExactThreadKeyRequired);
    }
    if requested_full_thread_id != thread_key.thread_id {
        return Err(DeleteMutationTransitionError::ThreadIdentityMismatch);
    }
    let parsed = uuid::Uuid::parse_str(requested_full_thread_id)
        .map_err(|_| DeleteMutationTransitionError::FullThreadIdRequired)?;
    if parsed.hyphenated().to_string() != requested_full_thread_id.to_ascii_lowercase() {
        return Err(DeleteMutationTransitionError::FullThreadIdRequired);
    }
    Ok(())
}

fn ensure_dispatched(
    observation: &DeleteMutationObservation,
) -> Result<(), DeleteMutationTransitionError> {
    if observation.dispatch_count == 1 {
        Ok(())
    } else {
        Err(DeleteMutationTransitionError::InvalidTransition)
    }
}

fn confirm(
    observation: &mut DeleteMutationObservation,
    evidence: DeleteDirectEvidenceKind,
    observed_at: i64,
) {
    observation.state = DeleteMutationState::DeleteConfirmed;
    observation.direct_evidence = Some(evidence);
    observation.failure_kind = None;
    observation.observed_at = observed_at;
    observation.completed_at = Some(observed_at);
}

pub(crate) fn classify_delete_dispatch_error(error: &str) -> DeleteMutationFailureKind {
    if error.contains("timed out") {
        DeleteMutationFailureKind::Timeout
    } else if error.contains("canceled") {
        DeleteMutationFailureKind::Cancellation
    } else {
        DeleteMutationFailureKind::DispatchDisconnected
    }
}

pub(crate) fn is_exact_delete_success(response: &Value) -> bool {
    response.get("error").is_none()
        && response
            .get("result")
            .and_then(Value::as_object)
            .is_some_and(|result| result.is_empty())
}

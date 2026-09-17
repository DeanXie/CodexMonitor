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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteMutationRejectionSource {
    LocalPreDispatchRejection,
    UpstreamRejection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteMutationRejectionReason {
    DuplicateActiveAttempt,
    StaleTransportGeneration,
    StaleWorkspaceSessionGeneration,
    StaleAppServerGeneration,
    IdentityMismatch,
    InvalidExactThreadKey,
    TransportDisconnectedBeforeDispatch,
    CancellationBeforeDispatch,
    UpstreamActiveWriter,
    UpstreamOther,
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
    pub rejection_source: Option<DeleteMutationRejectionSource>,
    pub rejection_reason: Option<DeleteMutationRejectionReason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeleteAttemptAdmission {
    attempt_id: DeleteAttemptId,
    admitted: bool,
}

impl DeleteAttemptAdmission {
    pub(crate) fn attempt_id(&self) -> &DeleteAttemptId {
        &self.attempt_id
    }

    pub(crate) fn is_admitted(&self) -> bool {
        self.admitted
    }

    pub(crate) fn into_attempt_id(self) -> DeleteAttemptId {
        self.attempt_id
    }
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
    ActiveDeleteAttempt,
    InvalidTransition,
    DirectEvidenceMismatch,
}

#[derive(Default)]
struct DeleteMutationRuntimeState {
    attempts: HashMap<DeleteAttemptId, DeleteMutationObservation>,
    active_by_thread: HashMap<CodexThreadKey, DeleteAttemptId>,
    latest_by_thread: HashMap<CodexThreadKey, DeleteAttemptId>,
    attempt_order: Vec<DeleteAttemptId>,
}

#[derive(Clone)]
pub(crate) struct DeleteMutationObservationRuntime {
    workspace_session_generation: WorkspaceSessionGeneration,
    app_server_connection_generation: AppServerConnectionGeneration,
    state: Arc<Mutex<DeleteMutationRuntimeState>>,
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
            state: Arc::new(Mutex::new(DeleteMutationRuntimeState::default())),
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
        let admission = self.begin_delete_attempt(
            remote_host_identity,
            thread_key,
            requested_full_thread_id,
            expected_workspace_session_generation,
            expected_app_server_connection_generation,
            observed_at,
        );
        if admission.is_admitted() {
            return Ok(admission.into_attempt_id());
        }
        let observation = self
            .snapshot(admission.attempt_id())
            .expect("new delete attempt observation");
        Err(match observation.rejection_reason {
            Some(DeleteMutationRejectionReason::StaleWorkspaceSessionGeneration) => {
                DeleteMutationTransitionError::WorkspaceSessionGenerationMismatch
            }
            Some(DeleteMutationRejectionReason::StaleAppServerGeneration) => {
                DeleteMutationTransitionError::AppServerConnectionGenerationMismatch
            }
            Some(DeleteMutationRejectionReason::DuplicateActiveAttempt) => {
                DeleteMutationTransitionError::ActiveDeleteAttempt
            }
            Some(DeleteMutationRejectionReason::IdentityMismatch) => {
                DeleteMutationTransitionError::ThreadIdentityMismatch
            }
            _ => DeleteMutationTransitionError::ExactThreadKeyRequired,
        })
    }

    pub(crate) fn begin_delete_attempt(
        &self,
        remote_host_identity: RemoteHostIdentity,
        thread_key: CodexThreadKey,
        requested_full_thread_id: &str,
        expected_workspace_session_generation: &str,
        expected_app_server_connection_generation: &str,
        observed_at: i64,
    ) -> DeleteAttemptAdmission {
        let attempt_id = DeleteAttemptId::generate();
        let validation_error =
            validate_exact_thread_key(&thread_key, requested_full_thread_id).err();
        let generation_rejection = if expected_workspace_session_generation
            != self.workspace_session_generation.as_str()
        {
            Some(DeleteMutationRejectionReason::StaleWorkspaceSessionGeneration)
        } else if expected_app_server_connection_generation
            != self.app_server_connection_generation.as_str()
        {
            Some(DeleteMutationRejectionReason::StaleAppServerGeneration)
        } else {
            None
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active_rejection = state
            .active_by_thread
            .contains_key(&thread_key)
            .then_some(DeleteMutationRejectionReason::DuplicateActiveAttempt);
        let rejection_reason = validation_error
            .map(|error| match error {
                DeleteMutationTransitionError::ThreadIdentityMismatch => {
                    DeleteMutationRejectionReason::IdentityMismatch
                }
                _ => DeleteMutationRejectionReason::InvalidExactThreadKey,
            })
            .or(generation_rejection)
            .or(active_rejection);
        let admitted = rejection_reason.is_none();
        let observation = DeleteMutationObservation {
            attempt_id: attempt_id.clone(),
            remote_host_identity,
            thread_key: thread_key.clone(),
            workspace_session_generation: expected_workspace_session_generation.to_string(),
            app_server_connection_generation: expected_app_server_connection_generation.to_string(),
            requested_full_thread_id: requested_full_thread_id.to_string(),
            state: if admitted {
                DeleteMutationState::DeletePending
            } else {
                DeleteMutationState::DeleteRejected
            },
            observed_at,
            dispatched_at: None,
            completed_at: (!admitted).then_some(observed_at),
            dispatch_count: 0,
            retry_count: 0,
            direct_evidence: None,
            upstream_error_code: None,
            failure_kind: None,
            rejection_source: (!admitted)
                .then_some(DeleteMutationRejectionSource::LocalPreDispatchRejection),
            rejection_reason,
        };
        state.attempt_order.push(attempt_id.clone());
        state
            .latest_by_thread
            .insert(thread_key.clone(), attempt_id.clone());
        if admitted {
            state
                .active_by_thread
                .insert(thread_key, attempt_id.clone());
        }
        state.attempts.insert(attempt_id.clone(), observation);
        DeleteAttemptAdmission {
            attempt_id,
            admitted,
        }
    }

    pub(crate) fn snapshot(
        &self,
        attempt_id: &DeleteAttemptId,
    ) -> Option<DeleteMutationObservation> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .attempts
            .get(attempt_id)
            .cloned()
    }

    pub(crate) fn latest_for_thread(
        &self,
        thread_key: &CodexThreadKey,
    ) -> Option<DeleteMutationObservation> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .latest_by_thread
            .get(thread_key)
            .and_then(|attempt_id| state.attempts.get(attempt_id))
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
                observation.rejection_source =
                    Some(DeleteMutationRejectionSource::UpstreamRejection);
                observation.rejection_reason = Some(if active_writer {
                    DeleteMutationRejectionReason::UpstreamActiveWriter
                } else {
                    DeleteMutationRejectionReason::UpstreamOther
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
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let candidate = state.attempt_order.iter().rev().find(|attempt_id| {
            state.attempts.get(*attempt_id).is_some_and(|observation| {
                &observation.remote_host_identity == remote_host_identity
                    && observation.requested_full_thread_id == full_thread_id
                    && observation.dispatch_count == 1
            })
        });
        let Some(attempt_id) = candidate.cloned() else {
            return Err(DeleteMutationTransitionError::DirectEvidenceMismatch);
        };
        let observation = state
            .attempts
            .get_mut(&attempt_id)
            .expect("candidate delete attempt");
        if observation.dispatch_count != 1 {
            return Err(DeleteMutationTransitionError::InvalidTransition);
        }
        let thread_key = observation.thread_key.clone();
        confirm(
            observation,
            DeleteDirectEvidenceKind::ThreadDeletedNotification,
            observed_at,
        );
        state.active_by_thread.remove(&thread_key);
        state.latest_by_thread.insert(thread_key, attempt_id);
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
            if observation.state == DeleteMutationState::DeleteConfirmed
                || (observation.state == DeleteMutationState::DeleteRejected
                    && observation.direct_evidence == Some(DeleteDirectEvidenceKind::UpstreamError))
            {
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
        let reason = if failure_kind == DeleteMutationFailureKind::Cancellation {
            DeleteMutationRejectionReason::CancellationBeforeDispatch
        } else {
            DeleteMutationRejectionReason::TransportDisconnectedBeforeDispatch
        };
        self.record_local_pre_dispatch_rejection(attempt_id, failure_kind, reason, observed_at)
    }

    pub(crate) fn record_local_pre_dispatch_rejection(
        &self,
        attempt_id: &DeleteAttemptId,
        failure_kind: DeleteMutationFailureKind,
        rejection_reason: DeleteMutationRejectionReason,
        observed_at: i64,
    ) -> Result<(), DeleteMutationTransitionError> {
        self.update(attempt_id, |observation| {
            if observation.dispatch_count == 0
                && observation.state == DeleteMutationState::DeleteRejected
            {
                return Ok(());
            }
            if observation.dispatch_count != 0
                || observation.state != DeleteMutationState::DeletePending
            {
                return Err(DeleteMutationTransitionError::InvalidTransition);
            }
            observation.state = DeleteMutationState::DeleteRejected;
            observation.failure_kind = Some(failure_kind);
            observation.rejection_source =
                Some(DeleteMutationRejectionSource::LocalPreDispatchRejection);
            observation.rejection_reason = Some(rejection_reason);
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
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let candidate = state.attempt_order.iter().rev().find(|attempt_id| {
            state.attempts.get(*attempt_id).is_some_and(|observation| {
                observation.requested_full_thread_id == full_thread_id
                    && observation.dispatch_count == 1
            })
        });
        let Some(attempt_id) = candidate.cloned() else {
            return Err(DeleteMutationTransitionError::DirectEvidenceMismatch);
        };
        let observation = state
            .attempts
            .get_mut(&attempt_id)
            .expect("candidate delete attempt");
        if observation.dispatch_count != 1 {
            return Err(DeleteMutationTransitionError::InvalidTransition);
        }
        let thread_key = observation.thread_key.clone();
        confirm(
            observation,
            DeleteDirectEvidenceKind::ThreadDeletedNotification,
            observed_at,
        );
        state.active_by_thread.remove(&thread_key);
        state.latest_by_thread.insert(thread_key, attempt_id);
        Ok(())
    }

    pub(crate) fn record_session_ended(&self, observed_at: i64) -> usize {
        let mut changed = 0;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut released = Vec::new();
        for observation in state.attempts.values_mut() {
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
                released.push((
                    observation.thread_key.clone(),
                    observation.attempt_id.clone(),
                ));
                changed += 1;
            }
        }
        for (thread_key, attempt_id) in released {
            if state.active_by_thread.get(&thread_key) == Some(&attempt_id) {
                state.active_by_thread.remove(&thread_key);
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
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .attempts
            .values()
            .map(|observation| observation.dispatch_count)
            .sum()
    }

    pub(crate) fn attempt_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .attempts
            .len()
    }

    pub(crate) fn tombstone_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .attempts
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
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (thread_key, terminal, confirmed) = {
            let observation = state
                .attempts
                .get_mut(attempt_id)
                .ok_or(DeleteMutationTransitionError::AttemptNotFound)?;
            update(observation)?;
            (
                observation.thread_key.clone(),
                is_terminal(observation.state),
                observation.state == DeleteMutationState::DeleteConfirmed,
            )
        };
        if terminal && state.active_by_thread.get(&thread_key) == Some(attempt_id) {
            state.active_by_thread.remove(&thread_key);
        }
        if confirmed {
            state
                .latest_by_thread
                .insert(thread_key, attempt_id.clone());
        }
        Ok(())
    }
}

fn is_terminal(state: DeleteMutationState) -> bool {
    matches!(
        state,
        DeleteMutationState::DeleteConfirmed
            | DeleteMutationState::DeleteRejected
            | DeleteMutationState::DeleteOutcomeUnknown
            | DeleteMutationState::SessionEndedOutcomeUnknown
    )
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
    observation.rejection_source = None;
    observation.rejection_reason = None;
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

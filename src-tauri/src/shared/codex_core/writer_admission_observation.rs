//! WorkspaceSession-scoped writer-admission evidence.
//!
//! The upstream Codex app-server/core remains the writer authority. This
//! module records only direct admission outcomes for one WorkspaceSession
//! generation and one canonical Thread. It cannot establish global writer
//! availability, writer identity, lease identity, or release.

use crate::shared::codex_identity::CodexThreadKey;

const RESUME_METHOD: &str = "thread/resume";
const ACTIVE_WRITER_ERROR_CODE: i64 = -32600;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WorkspaceSessionGeneration(String);

impl WorkspaceSessionGeneration {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("workspace session generation is required".to_string());
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WriterAdmissionAttemptId(String);

impl WriterAdmissionAttemptId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("writer admission attempt id is required".to_string());
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriterAdmissionObservationState {
    NotObserved,
    AdmissionPending,
    AdmittedForSession,
    BlockedByActiveWriter,
    AdmissionOutcomeUnknown,
    SessionEndedReleaseUnobserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriterAdmissionErrorKind {
    BlockedByActiveWriter,
    Timeout,
    DispatchDisconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriterAdmissionNonTransitionEvent {
    ThreadRead,
    TurnCompleted,
    TurnIdle,
    OrdinaryRefresh,
    Polling,
    RemoteTcpDisconnected,
    RemoteClientClosed,
    MobilePageClosed,
    FocusLost,
    ThreadUnsubscribe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriterAdmissionEvidence {
    pub request_method: &'static str,
    pub returned_full_thread_id: Option<String>,
    pub upstream_error_code: Option<i64>,
    pub normalized_error_kind: Option<WriterAdmissionErrorKind>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriterAdmissionObservation {
    pub thread_key: CodexThreadKey,
    pub workspace_session_generation: WorkspaceSessionGeneration,
    pub observed_at: i64,
    pub attempt_id: WriterAdmissionAttemptId,
    pub requested_full_thread_id: String,
    pub state: WriterAdmissionObservationState,
    pub evidence: WriterAdmissionEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WriterAdmissionObservationTracker {
    thread_key: CodexThreadKey,
    workspace_session_generation: WorkspaceSessionGeneration,
    latest_observation: Option<WriterAdmissionObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriterAdmissionTransitionError {
    InvalidTransition,
    ThreadIdentityMismatch,
    SessionGenerationMismatch,
    ActiveWriterEvidenceMismatch,
}

impl WriterAdmissionObservationTracker {
    pub(crate) fn new(
        thread_key: CodexThreadKey,
        workspace_session_generation: WorkspaceSessionGeneration,
    ) -> Self {
        Self {
            thread_key,
            workspace_session_generation,
            latest_observation: None,
        }
    }

    pub(crate) fn thread_key(&self) -> &CodexThreadKey {
        &self.thread_key
    }

    pub(crate) fn workspace_session_generation(&self) -> &WorkspaceSessionGeneration {
        &self.workspace_session_generation
    }

    pub(crate) fn state(&self) -> WriterAdmissionObservationState {
        self.latest_observation.as_ref().map_or(
            WriterAdmissionObservationState::NotObserved,
            |observation| observation.state,
        )
    }

    pub(crate) fn latest_observation(&self) -> Option<&WriterAdmissionObservation> {
        self.latest_observation.as_ref()
    }

    pub(crate) fn begin_resume(
        &mut self,
        attempt_id: WriterAdmissionAttemptId,
        requested_full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        if self.state() != WriterAdmissionObservationState::NotObserved {
            return Err(WriterAdmissionTransitionError::InvalidTransition);
        }
        if requested_full_thread_id != self.thread_key.thread_id {
            return Err(WriterAdmissionTransitionError::ThreadIdentityMismatch);
        }
        self.latest_observation = Some(WriterAdmissionObservation {
            thread_key: self.thread_key.clone(),
            workspace_session_generation: self.workspace_session_generation.clone(),
            observed_at,
            attempt_id,
            requested_full_thread_id: requested_full_thread_id.to_string(),
            state: WriterAdmissionObservationState::AdmissionPending,
            evidence: WriterAdmissionEvidence {
                request_method: RESUME_METHOD,
                returned_full_thread_id: None,
                upstream_error_code: None,
                normalized_error_kind: None,
                diagnostic: None,
            },
        });
        Ok(())
    }

    pub(crate) fn record_exact_resume_success(
        &mut self,
        returned_full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let observation = self.pending_observation_mut()?;
        if returned_full_thread_id != observation.requested_full_thread_id {
            return Err(WriterAdmissionTransitionError::ThreadIdentityMismatch);
        }
        observation.state = WriterAdmissionObservationState::AdmittedForSession;
        observation.observed_at = observed_at;
        observation.evidence.returned_full_thread_id = Some(returned_full_thread_id.to_string());
        Ok(())
    }

    pub(crate) fn record_active_writer_blocked(
        &mut self,
        upstream_error_code: i64,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        if upstream_error_code != ACTIVE_WRITER_ERROR_CODE {
            return Err(WriterAdmissionTransitionError::ActiveWriterEvidenceMismatch);
        }
        let observation = self.pending_observation_mut()?;
        observation.state = WriterAdmissionObservationState::BlockedByActiveWriter;
        observation.observed_at = observed_at;
        observation.evidence.upstream_error_code = Some(upstream_error_code);
        observation.evidence.normalized_error_kind =
            Some(WriterAdmissionErrorKind::BlockedByActiveWriter);
        observation.evidence.diagnostic = Some(diagnostic.into());
        Ok(())
    }

    pub(crate) fn record_timeout(
        &mut self,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        self.record_outcome_unknown(
            WriterAdmissionErrorKind::Timeout,
            diagnostic.into(),
            observed_at,
        )
    }

    pub(crate) fn record_dispatch_disconnect(
        &mut self,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        self.record_outcome_unknown(
            WriterAdmissionErrorKind::DispatchDisconnected,
            diagnostic.into(),
            observed_at,
        )
    }

    pub(crate) fn record_session_ended(
        &mut self,
        workspace_session_generation: &WorkspaceSessionGeneration,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        if workspace_session_generation != &self.workspace_session_generation {
            return Err(WriterAdmissionTransitionError::SessionGenerationMismatch);
        }
        let observation = self
            .latest_observation
            .as_mut()
            .ok_or(WriterAdmissionTransitionError::InvalidTransition)?;
        if !matches!(
            observation.state,
            WriterAdmissionObservationState::AdmittedForSession
                | WriterAdmissionObservationState::BlockedByActiveWriter
                | WriterAdmissionObservationState::AdmissionOutcomeUnknown
        ) {
            return Err(WriterAdmissionTransitionError::InvalidTransition);
        }
        observation.state = WriterAdmissionObservationState::SessionEndedReleaseUnobserved;
        observation.observed_at = observed_at;
        Ok(())
    }

    pub(crate) fn for_new_generation(
        &self,
        workspace_session_generation: WorkspaceSessionGeneration,
    ) -> Self {
        Self::new(self.thread_key.clone(), workspace_session_generation)
    }

    pub(crate) fn observe_non_transition(&mut self, _event: WriterAdmissionNonTransitionEvent) {}

    pub(crate) fn writer_owner_identity(&self) -> Option<&str> {
        None
    }

    pub(crate) fn writer_lease_identity(&self) -> Option<&str> {
        None
    }

    pub(crate) fn global_writer_free_observed(&self) -> bool {
        false
    }

    fn pending_observation_mut(
        &mut self,
    ) -> Result<&mut WriterAdmissionObservation, WriterAdmissionTransitionError> {
        let observation = self
            .latest_observation
            .as_mut()
            .ok_or(WriterAdmissionTransitionError::InvalidTransition)?;
        if observation.state != WriterAdmissionObservationState::AdmissionPending {
            return Err(WriterAdmissionTransitionError::InvalidTransition);
        }
        Ok(observation)
    }

    fn record_outcome_unknown(
        &mut self,
        kind: WriterAdmissionErrorKind,
        diagnostic: String,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let observation = self.pending_observation_mut()?;
        observation.state = WriterAdmissionObservationState::AdmissionOutcomeUnknown;
        observation.observed_at = observed_at;
        observation.evidence.normalized_error_kind = Some(kind);
        observation.evidence.diagnostic = Some(diagnostic);
        Ok(())
    }
}

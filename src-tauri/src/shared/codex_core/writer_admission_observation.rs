//! WorkspaceSession-scoped writer-admission evidence.
//!
//! The upstream Codex app-server/core remains the writer authority. This
//! module records only direct admission outcomes for one WorkspaceSession
//! generation and one canonical Thread. It cannot establish global writer
//! availability, writer identity, lease identity, or release.

use crate::shared::codex_identity::CodexThreadKey;
use std::collections::HashMap;
use std::sync::Mutex;

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
    Cancellation,
    MalformedResponse,
    UnclassifiedUpstreamResponse,
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
    pub exact_id_match: Option<bool>,
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
    AttemptMismatch,
}

/// Process-local evidence for one concrete WorkspaceSession generation.
///
/// Remote clients are deliberately absent from the key. A new runtime is
/// created with every WorkspaceSession, so prior admission evidence cannot be
/// inherited by a replacement app-server process.
pub(crate) struct WriterAdmissionObservationRuntime {
    workspace_session_generation: WorkspaceSessionGeneration,
    observations: Mutex<HashMap<CodexThreadKey, WriterAdmissionThreadObservations>>,
}

struct WriterAdmissionThreadObservations {
    latest_attempt_id: WriterAdmissionAttemptId,
    attempts: HashMap<WriterAdmissionAttemptId, WriterAdmissionObservationTracker>,
}

impl Default for WriterAdmissionObservationRuntime {
    fn default() -> Self {
        Self::new(
            WorkspaceSessionGeneration::new(uuid::Uuid::new_v4().to_string())
                .expect("UUID workspace session generation"),
        )
    }
}

impl WriterAdmissionObservationRuntime {
    pub(crate) fn new(workspace_session_generation: WorkspaceSessionGeneration) -> Self {
        Self {
            workspace_session_generation,
            observations: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn workspace_session_generation(&self) -> &WorkspaceSessionGeneration {
        &self.workspace_session_generation
    }

    pub(crate) fn state(&self, thread_key: &CodexThreadKey) -> WriterAdmissionObservationState {
        self.snapshot(thread_key).map_or(
            WriterAdmissionObservationState::NotObserved,
            |observation| observation.state,
        )
    }

    pub(crate) fn snapshot(
        &self,
        thread_key: &CodexThreadKey,
    ) -> Option<WriterAdmissionObservation> {
        self.observations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(thread_key)
            .and_then(|observations| observations.attempts.get(&observations.latest_attempt_id))
            .and_then(|tracker| tracker.latest_observation().cloned())
    }

    pub(crate) fn begin_resume(
        &self,
        thread_key: CodexThreadKey,
        requested_full_thread_id: &str,
        observed_at: i64,
    ) -> Result<WriterAdmissionAttemptId, WriterAdmissionTransitionError> {
        let attempt_id = WriterAdmissionAttemptId(uuid::Uuid::new_v4().to_string());
        let mut tracker = WriterAdmissionObservationTracker::new(
            thread_key.clone(),
            self.workspace_session_generation.clone(),
        );
        tracker.begin_resume(attempt_id.clone(), requested_full_thread_id, observed_at)?;
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match observations.get_mut(&thread_key) {
            Some(existing) => {
                existing.attempts.retain(|_, tracker| {
                    tracker.state() == WriterAdmissionObservationState::AdmissionPending
                });
                existing.attempts.insert(attempt_id.clone(), tracker);
                existing.latest_attempt_id = attempt_id.clone();
            }
            None => {
                observations.insert(
                    thread_key,
                    WriterAdmissionThreadObservations {
                        latest_attempt_id: attempt_id.clone(),
                        attempts: HashMap::from([(attempt_id.clone(), tracker)]),
                    },
                );
            }
        }
        Ok(attempt_id)
    }

    pub(crate) fn record_exact_resume_success(
        &self,
        thread_key: &CodexThreadKey,
        attempt_id: &WriterAdmissionAttemptId,
        returned_full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        self.with_attempt(thread_key, attempt_id, |tracker| {
            tracker.record_exact_resume_success(returned_full_thread_id, observed_at)
        })
    }

    pub(crate) fn record_active_writer_blocked(
        &self,
        thread_key: &CodexThreadKey,
        attempt_id: &WriterAdmissionAttemptId,
        upstream_error_code: i64,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let diagnostic = diagnostic.into();
        self.with_attempt(thread_key, attempt_id, |tracker| {
            tracker.record_active_writer_blocked(upstream_error_code, diagnostic, observed_at)
        })
    }

    pub(crate) fn record_malformed_response(
        &self,
        thread_key: &CodexThreadKey,
        attempt_id: &WriterAdmissionAttemptId,
        returned_full_thread_id: Option<&str>,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let diagnostic = diagnostic.into();
        self.with_attempt(thread_key, attempt_id, |tracker| {
            tracker.record_outcome_unknown(
                WriterAdmissionErrorKind::MalformedResponse,
                diagnostic,
                observed_at,
            )?;
            let observation = tracker
                .latest_observation
                .as_mut()
                .ok_or(WriterAdmissionTransitionError::InvalidTransition)?;
            observation.evidence.returned_full_thread_id =
                returned_full_thread_id.map(str::to_string);
            observation.evidence.exact_id_match = Some(false);
            Ok(())
        })
    }

    pub(crate) fn record_outcome_unknown(
        &self,
        thread_key: &CodexThreadKey,
        attempt_id: &WriterAdmissionAttemptId,
        kind: WriterAdmissionErrorKind,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let diagnostic = diagnostic.into();
        self.with_attempt(thread_key, attempt_id, |tracker| {
            tracker.record_outcome_unknown(kind, diagnostic, observed_at)
        })
    }

    fn with_attempt(
        &self,
        thread_key: &CodexThreadKey,
        attempt_id: &WriterAdmissionAttemptId,
        update: impl FnOnce(
            &mut WriterAdmissionObservationTracker,
        ) -> Result<(), WriterAdmissionTransitionError>,
    ) -> Result<(), WriterAdmissionTransitionError> {
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let thread_observations = observations
            .get_mut(thread_key)
            .ok_or(WriterAdmissionTransitionError::InvalidTransition)?;
        let tracker = thread_observations
            .attempts
            .get_mut(attempt_id)
            .ok_or(WriterAdmissionTransitionError::AttemptMismatch)?;
        update(tracker)?;
        thread_observations.latest_attempt_id = attempt_id.clone();
        thread_observations
            .attempts
            .retain(|candidate_id, tracker| {
                candidate_id == attempt_id
                    || tracker.state() == WriterAdmissionObservationState::AdmissionPending
            });
        Ok(())
    }
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
                exact_id_match: None,
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
        observation.evidence.exact_id_match = Some(true);
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

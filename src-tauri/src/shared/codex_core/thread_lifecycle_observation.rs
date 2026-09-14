//! Connection-scoped Thread subscription and runtime-availability evidence.
//!
//! These observations are independent from writer admission. They record only
//! direct evidence for one WorkspaceSession generation, one app-server
//! connection generation, and one canonical Thread. They do not identify a
//! Remote client, writer owner, lease, writer availability, or writer release.

#![cfg_attr(not(test), allow(dead_code))]

use super::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use serde::{Deserialize, Serialize};

const UNSUBSCRIBE_METHOD: &str = "thread/unsubscribe";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AppServerConnectionGeneration(String);

impl AppServerConnectionGeneration {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("app-server connection generation is required".to_string());
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ThreadSubscriptionAttemptId(String);

impl ThreadSubscriptionAttemptId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("thread unsubscribe attempt id is required".to_string());
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadLifecycleObservationScope {
    workspace_session_generation: WorkspaceSessionGeneration,
    app_server_connection_generation: AppServerConnectionGeneration,
    thread_key: CodexThreadKey,
}

impl ThreadLifecycleObservationScope {
    pub(crate) fn new(
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
        thread_key: CodexThreadKey,
    ) -> Self {
        Self {
            workspace_session_generation,
            app_server_connection_generation,
            thread_key,
        }
    }

    pub(crate) fn workspace_session_generation(&self) -> &WorkspaceSessionGeneration {
        &self.workspace_session_generation
    }

    pub(crate) fn app_server_connection_generation(&self) -> &AppServerConnectionGeneration {
        &self.app_server_connection_generation
    }

    pub(crate) fn thread_key(&self) -> &CodexThreadKey {
        &self.thread_key
    }

    fn for_new_workspace_session_generation(
        &self,
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(
            workspace_session_generation,
            app_server_connection_generation,
            self.thread_key.clone(),
        )
    }

    fn for_new_app_server_connection_generation(
        &self,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(
            self.workspace_session_generation.clone(),
            app_server_connection_generation,
            self.thread_key.clone(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadSubscriptionObservationState {
    NotObserved,
    SubscribedForAppServerConnection,
    UnsubscribePending,
    UnsubscribedForAppServerConnection,
    NotSubscribedForAppServerConnection,
    UnsubscribeOutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadSubscriptionEvidenceSource {
    ThreadStartResponse,
    ThreadResumeResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadSubscriptionOutcomeErrorKind {
    Timeout,
    DispatchDisconnected,
    Cancellation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadSubscriptionEvidence {
    pub source: Option<ThreadSubscriptionEvidenceSource>,
    pub request_method: Option<&'static str>,
    pub response_status: Option<&'static str>,
    pub error_kind: Option<ThreadSubscriptionOutcomeErrorKind>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadSubscriptionObservation {
    pub scope: ThreadLifecycleObservationScope,
    pub state: ThreadSubscriptionObservationState,
    pub observed_at: i64,
    pub attempt_id: Option<ThreadSubscriptionAttemptId>,
    pub requested_full_thread_id: Option<String>,
    pub evidence: ThreadSubscriptionEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadSubscriptionEvidenceSnapshot {
    pub source: Option<ThreadSubscriptionEvidenceSource>,
    pub request_method: Option<String>,
    pub response_status: Option<String>,
    pub error_kind: Option<ThreadSubscriptionOutcomeErrorKind>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadSubscriptionObservationSnapshot {
    pub thread_key: CodexThreadKey,
    pub workspace_session_generation: String,
    pub app_server_connection_generation: String,
    pub state: ThreadSubscriptionObservationState,
    pub observed_at: Option<i64>,
    pub attempt_id: Option<String>,
    pub requested_full_thread_id: Option<String>,
    pub evidence: Option<ThreadSubscriptionEvidenceSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThreadSubscriptionTransitionError {
    InvalidTransition,
    ThreadIdentityMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadSubscriptionObservationTracker {
    scope: ThreadLifecycleObservationScope,
    latest_observation: Option<ThreadSubscriptionObservation>,
}

impl ThreadSubscriptionObservationTracker {
    pub(crate) fn new(scope: ThreadLifecycleObservationScope) -> Self {
        Self {
            scope,
            latest_observation: None,
        }
    }

    pub(crate) fn scope(&self) -> &ThreadLifecycleObservationScope {
        &self.scope
    }

    pub(crate) fn state(&self) -> ThreadSubscriptionObservationState {
        self.latest_observation.as_ref().map_or(
            ThreadSubscriptionObservationState::NotObserved,
            |observation| observation.state,
        )
    }

    pub(crate) fn latest_observation(&self) -> Option<&ThreadSubscriptionObservation> {
        self.latest_observation.as_ref()
    }

    pub(crate) fn snapshot(&self) -> ThreadSubscriptionObservationSnapshot {
        match &self.latest_observation {
            Some(observation) => ThreadSubscriptionObservationSnapshot {
                thread_key: observation.scope.thread_key.clone(),
                workspace_session_generation: observation
                    .scope
                    .workspace_session_generation
                    .as_str()
                    .to_string(),
                app_server_connection_generation: observation
                    .scope
                    .app_server_connection_generation
                    .as_str()
                    .to_string(),
                state: observation.state,
                observed_at: Some(observation.observed_at),
                attempt_id: observation
                    .attempt_id
                    .as_ref()
                    .map(|attempt| attempt.as_str().to_string()),
                requested_full_thread_id: observation.requested_full_thread_id.clone(),
                evidence: Some(ThreadSubscriptionEvidenceSnapshot {
                    source: observation.evidence.source,
                    request_method: observation.evidence.request_method.map(str::to_string),
                    response_status: observation.evidence.response_status.map(str::to_string),
                    error_kind: observation.evidence.error_kind,
                    diagnostic: observation.evidence.diagnostic.clone(),
                }),
            },
            None => ThreadSubscriptionObservationSnapshot {
                thread_key: self.scope.thread_key.clone(),
                workspace_session_generation: self
                    .scope
                    .workspace_session_generation
                    .as_str()
                    .to_string(),
                app_server_connection_generation: self
                    .scope
                    .app_server_connection_generation
                    .as_str()
                    .to_string(),
                state: ThreadSubscriptionObservationState::NotObserved,
                observed_at: None,
                attempt_id: None,
                requested_full_thread_id: None,
                evidence: None,
            },
        }
    }

    pub(crate) fn record_subscribed(
        &mut self,
        source: ThreadSubscriptionEvidenceSource,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        if self.state() != ThreadSubscriptionObservationState::NotObserved {
            return Err(ThreadSubscriptionTransitionError::InvalidTransition);
        }
        self.latest_observation = Some(ThreadSubscriptionObservation {
            scope: self.scope.clone(),
            state: ThreadSubscriptionObservationState::SubscribedForAppServerConnection,
            observed_at,
            attempt_id: None,
            requested_full_thread_id: None,
            evidence: ThreadSubscriptionEvidence {
                source: Some(source),
                request_method: None,
                response_status: None,
                error_kind: None,
                diagnostic: None,
            },
        });
        Ok(())
    }

    pub(crate) fn begin_unsubscribe(
        &mut self,
        attempt_id: ThreadSubscriptionAttemptId,
        requested_full_thread_id: &str,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        if self.state() != ThreadSubscriptionObservationState::SubscribedForAppServerConnection {
            return Err(ThreadSubscriptionTransitionError::InvalidTransition);
        }
        if requested_full_thread_id != self.scope.thread_key.thread_id {
            return Err(ThreadSubscriptionTransitionError::ThreadIdentityMismatch);
        }
        self.latest_observation = Some(ThreadSubscriptionObservation {
            scope: self.scope.clone(),
            state: ThreadSubscriptionObservationState::UnsubscribePending,
            observed_at,
            attempt_id: Some(attempt_id),
            requested_full_thread_id: Some(requested_full_thread_id.to_string()),
            evidence: ThreadSubscriptionEvidence {
                source: None,
                request_method: Some(UNSUBSCRIBE_METHOD),
                response_status: None,
                error_kind: None,
                diagnostic: None,
            },
        });
        Ok(())
    }

    pub(crate) fn record_unsubscribed(
        &mut self,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        self.record_response(
            ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection,
            "unsubscribed",
            observed_at,
        )
    }

    pub(crate) fn record_not_subscribed(
        &mut self,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        self.record_response(
            ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection,
            "notSubscribed",
            observed_at,
        )
    }

    pub(crate) fn record_timeout(
        &mut self,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        self.record_outcome_unknown(
            ThreadSubscriptionOutcomeErrorKind::Timeout,
            diagnostic.into(),
            observed_at,
        )
    }

    pub(crate) fn record_disconnect(
        &mut self,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        self.record_outcome_unknown(
            ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected,
            diagnostic.into(),
            observed_at,
        )
    }

    pub(crate) fn record_cancellation(
        &mut self,
        diagnostic: impl Into<String>,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        self.record_outcome_unknown(
            ThreadSubscriptionOutcomeErrorKind::Cancellation,
            diagnostic.into(),
            observed_at,
        )
    }

    pub(crate) fn for_new_workspace_session_generation(
        &self,
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(self.scope.for_new_workspace_session_generation(
            workspace_session_generation,
            app_server_connection_generation,
        ))
    }

    pub(crate) fn for_new_app_server_connection_generation(
        &self,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(
            self.scope
                .for_new_app_server_connection_generation(app_server_connection_generation),
        )
    }

    fn record_response(
        &mut self,
        state: ThreadSubscriptionObservationState,
        response_status: &'static str,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        let observation = self.pending_observation_mut()?;
        observation.state = state;
        observation.observed_at = observed_at;
        observation.evidence.response_status = Some(response_status);
        Ok(())
    }

    fn record_outcome_unknown(
        &mut self,
        error_kind: ThreadSubscriptionOutcomeErrorKind,
        diagnostic: String,
        observed_at: i64,
    ) -> Result<(), ThreadSubscriptionTransitionError> {
        let observation = self.pending_observation_mut()?;
        observation.state = ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown;
        observation.observed_at = observed_at;
        observation.evidence.error_kind = Some(error_kind);
        observation.evidence.diagnostic = Some(diagnostic);
        Ok(())
    }

    fn pending_observation_mut(
        &mut self,
    ) -> Result<&mut ThreadSubscriptionObservation, ThreadSubscriptionTransitionError> {
        let observation = self
            .latest_observation
            .as_mut()
            .ok_or(ThreadSubscriptionTransitionError::InvalidTransition)?;
        if observation.state != ThreadSubscriptionObservationState::UnsubscribePending {
            return Err(ThreadSubscriptionTransitionError::InvalidTransition);
        }
        Ok(observation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadRuntimeAvailabilityState {
    Unknown,
    LoadedObserved,
    NotLoadedObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThreadRuntimeAvailabilityEvidenceSource {
    ThreadStartResponse,
    ThreadReadResponse,
    ThreadResumeResponse,
    ThreadLoadedList,
    ThreadClosedNotification,
    ThreadStatusChangedNotification,
    ThreadUnsubscribeNotLoadedResponse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadRuntimeAvailabilityObservation {
    pub scope: ThreadLifecycleObservationScope,
    pub state: ThreadRuntimeAvailabilityState,
    pub observed_at: i64,
    pub evidence_source: ThreadRuntimeAvailabilityEvidenceSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadRuntimeAvailabilityTracker {
    scope: ThreadLifecycleObservationScope,
    latest_observation: Option<ThreadRuntimeAvailabilityObservation>,
}

impl ThreadRuntimeAvailabilityTracker {
    pub(crate) fn new(scope: ThreadLifecycleObservationScope) -> Self {
        Self {
            scope,
            latest_observation: None,
        }
    }

    pub(crate) fn state(&self) -> ThreadRuntimeAvailabilityState {
        self.latest_observation
            .as_ref()
            .map_or(ThreadRuntimeAvailabilityState::Unknown, |observation| {
                observation.state
            })
    }

    pub(crate) fn latest_observation(&self) -> Option<&ThreadRuntimeAvailabilityObservation> {
        self.latest_observation.as_ref()
    }

    pub(crate) fn record_loaded(
        &mut self,
        evidence_source: ThreadRuntimeAvailabilityEvidenceSource,
        observed_at: i64,
    ) {
        self.record(
            ThreadRuntimeAvailabilityState::LoadedObserved,
            evidence_source,
            observed_at,
        );
    }

    pub(crate) fn record_not_loaded(
        &mut self,
        evidence_source: ThreadRuntimeAvailabilityEvidenceSource,
        observed_at: i64,
    ) {
        self.record(
            ThreadRuntimeAvailabilityState::NotLoadedObserved,
            evidence_source,
            observed_at,
        );
    }

    pub(crate) fn for_new_workspace_session_generation(
        &self,
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(self.scope.for_new_workspace_session_generation(
            workspace_session_generation,
            app_server_connection_generation,
        ))
    }

    pub(crate) fn for_new_app_server_connection_generation(
        &self,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self::new(
            self.scope
                .for_new_app_server_connection_generation(app_server_connection_generation),
        )
    }

    fn record(
        &mut self,
        state: ThreadRuntimeAvailabilityState,
        evidence_source: ThreadRuntimeAvailabilityEvidenceSource,
        observed_at: i64,
    ) {
        self.latest_observation = Some(ThreadRuntimeAvailabilityObservation {
            scope: self.scope.clone(),
            state,
            observed_at,
            evidence_source,
        });
    }
}

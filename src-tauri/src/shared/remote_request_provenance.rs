//! Remote transport generation and request-dispatch provenance.
//!
//! This evidence is local to one process and one authenticated TCP transport.
//! It correlates transport request IDs across reconnects and records transport
//! dispatch boundaries. It is not a Remote-client identity, writer or
//! subscription ownership record, lease, or takeover authority.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RemoteTransportGeneration(String);

impl RemoteTransportGeneration {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("remote transport generation is required".to_string());
        }
        Ok(Self(value))
    }

    pub(crate) fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteRequestKey {
    pub transport_generation: RemoteTransportGeneration,
    pub transport_request_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteRequestDispatchState {
    Received,
    DispatchStarted,
    SessionAttemptBound,
    ResponseObserved,
    TransportLost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionAttemptProvenance {
    pub workspace_id: String,
    pub workspace_session_generation: String,
    pub attempt_id: String,
}

impl SessionAttemptProvenance {
    pub(crate) fn new(
        workspace_id: impl Into<String>,
        workspace_session_generation: impl Into<String>,
        attempt_id: impl Into<String>,
    ) -> Result<Self, String> {
        let value = Self {
            workspace_id: workspace_id.into(),
            workspace_session_generation: workspace_session_generation.into(),
            attempt_id: attempt_id.into(),
        };
        if value.workspace_id.trim().is_empty() {
            return Err("workspace id is required".to_string());
        }
        if value.workspace_session_generation.trim().is_empty() {
            return Err("workspace session generation is required".to_string());
        }
        if value.attempt_id.trim().is_empty() {
            return Err("session attempt id is required".to_string());
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteRequestProvenance {
    #[serde(flatten)]
    pub key: RemoteRequestKey,
    pub method: String,
    pub received_at: i64,
    pub dispatch_state: RemoteRequestDispatchState,
    pub dispatch_started_at: Option<i64>,
    pub session_attempt_bound_at: Option<i64>,
    pub session_attempt: Option<SessionAttemptProvenance>,
    pub response_observed_at: Option<i64>,
    pub transport_lost_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemoteRequestTransitionError {
    DuplicateRequest,
    RequestNotFound,
    TransportGenerationMismatch,
    InvalidTransition,
}

pub(crate) struct RemoteRequestProvenanceRuntime {
    transport_generation: RemoteTransportGeneration,
    observations: Mutex<HashMap<RemoteRequestKey, RemoteRequestProvenance>>,
}

impl RemoteRequestProvenanceRuntime {
    pub(crate) fn new(transport_generation: RemoteTransportGeneration) -> Self {
        Self {
            transport_generation,
            observations: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn new_authenticated_transport() -> Self {
        Self::new(RemoteTransportGeneration::generate())
    }

    pub(crate) fn transport_generation(&self) -> &RemoteTransportGeneration {
        &self.transport_generation
    }

    pub(crate) fn record_received(
        &self,
        transport_request_id: u64,
        method: impl Into<String>,
        received_at: i64,
    ) -> Result<RemoteRequestKey, RemoteRequestTransitionError> {
        let key = RemoteRequestKey {
            transport_generation: self.transport_generation.clone(),
            transport_request_id,
        };
        let mut observations = self.observations.lock().expect("provenance lock");
        if observations.contains_key(&key) {
            return Err(RemoteRequestTransitionError::DuplicateRequest);
        }
        observations.insert(
            key.clone(),
            RemoteRequestProvenance {
                key: key.clone(),
                method: method.into(),
                received_at,
                dispatch_state: RemoteRequestDispatchState::Received,
                dispatch_started_at: None,
                session_attempt_bound_at: None,
                session_attempt: None,
                response_observed_at: None,
                transport_lost_at: None,
            },
        );
        Ok(key)
    }

    pub(crate) fn record_dispatch_started(
        &self,
        key: &RemoteRequestKey,
        observed_at: i64,
    ) -> Result<(), RemoteRequestTransitionError> {
        self.validate_generation(key)?;
        let mut observations = self.observations.lock().expect("provenance lock");
        let observation = observations
            .get_mut(key)
            .ok_or(RemoteRequestTransitionError::RequestNotFound)?;
        if observation.dispatch_started_at.is_some() || observation.response_observed_at.is_some() {
            return Err(RemoteRequestTransitionError::InvalidTransition);
        }
        observation.dispatch_started_at = Some(observed_at);
        if observation.transport_lost_at.is_none() {
            observation.dispatch_state = RemoteRequestDispatchState::DispatchStarted;
        }
        Ok(())
    }

    pub(crate) fn record_session_attempt_bound(
        &self,
        key: &RemoteRequestKey,
        session_attempt: SessionAttemptProvenance,
        observed_at: i64,
    ) -> Result<(), RemoteRequestTransitionError> {
        self.validate_generation(key)?;
        let mut observations = self.observations.lock().expect("provenance lock");
        let observation = observations
            .get_mut(key)
            .ok_or(RemoteRequestTransitionError::RequestNotFound)?;
        if observation.dispatch_started_at.is_none()
            || observation.session_attempt.is_some()
            || observation.response_observed_at.is_some()
        {
            return Err(RemoteRequestTransitionError::InvalidTransition);
        }
        observation.session_attempt_bound_at = Some(observed_at);
        observation.session_attempt = Some(session_attempt);
        if observation.transport_lost_at.is_none() {
            observation.dispatch_state = RemoteRequestDispatchState::SessionAttemptBound;
        }
        Ok(())
    }

    pub(crate) fn record_response_observed(
        &self,
        key: &RemoteRequestKey,
        observed_at: i64,
    ) -> Result<(), RemoteRequestTransitionError> {
        self.validate_generation(key)?;
        let mut observations = self.observations.lock().expect("provenance lock");
        let observation = observations
            .get_mut(key)
            .ok_or(RemoteRequestTransitionError::RequestNotFound)?;
        if observation.dispatch_started_at.is_none() || observation.response_observed_at.is_some() {
            return Err(RemoteRequestTransitionError::InvalidTransition);
        }
        observation.response_observed_at = Some(observed_at);
        observation.dispatch_state = RemoteRequestDispatchState::ResponseObserved;
        Ok(())
    }

    pub(crate) fn record_transport_lost(&self, observed_at: i64) {
        let mut observations = self.observations.lock().expect("provenance lock");
        for observation in observations.values_mut() {
            if observation.transport_lost_at.is_none() {
                observation.transport_lost_at = Some(observed_at);
            }
            if observation.response_observed_at.is_none() {
                observation.dispatch_state = RemoteRequestDispatchState::TransportLost;
            }
        }
    }

    pub(crate) fn snapshot(&self, key: &RemoteRequestKey) -> Option<RemoteRequestProvenance> {
        self.observations
            .lock()
            .expect("provenance lock")
            .get(key)
            .cloned()
    }

    pub(crate) fn snapshots(&self) -> Vec<RemoteRequestProvenance> {
        self.observations
            .lock()
            .expect("provenance lock")
            .values()
            .cloned()
            .collect()
    }

    fn validate_generation(
        &self,
        key: &RemoteRequestKey,
    ) -> Result<(), RemoteRequestTransitionError> {
        if key.transport_generation != self.transport_generation {
            return Err(RemoteRequestTransitionError::TransportGenerationMismatch);
        }
        Ok(())
    }
}

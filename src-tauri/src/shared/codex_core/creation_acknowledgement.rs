//! Creation acknowledgement facts, not a creation-intent registry or recovery engine.
#![allow(dead_code)]

use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};
use crate::shared::global_sources_core::rollout_record::SessionMetaRecord;
use crate::shared::global_sources_core::source_envelope::CodexHomeIdentity;
use serde::Serialize;
use serde_json::{json, Value};
use std::future::Future;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CreationState {
    CreateInFlight,
    ThreadAcknowledged,
    CreationFailed,
    CreationOutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PersistenceState {
    NotYetConfirmed,
    PersistenceConfirmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum EphemeralState {
    #[serde(rename = "UNKNOWN")]
    Unknown,
    #[serde(rename = "TRUE")]
    Ephemeral,
    #[serde(rename = "FALSE")]
    NonEphemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FirstTurnAcceptance {
    NotYetAccepted,
    FirstTurnAccepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FirstTurnOutcome {
    Unknown,
    Completed,
    Failed,
    Interrupted,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreationFailure {
    InvalidResponse,
    ServerRejected,
    IdentityConflict,
    EvidenceConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreationAcknowledgement {
    state: CreationState,
    thread_key: CodexThreadKey,
    persistence: PersistenceState,
    ephemeral: EphemeralState,
    first_turn_acceptance: FirstTurnAcceptance,
    first_turn: Option<CodexTurnKey>,
    first_turn_outcome: FirstTurnOutcome,
}

impl CreationAcknowledgement {
    pub(crate) fn state(&self) -> CreationState {
        self.state
    }
    pub(crate) fn thread_key(&self) -> &CodexThreadKey {
        &self.thread_key
    }
    pub(crate) fn persistence(&self) -> PersistenceState {
        self.persistence
    }
    pub(crate) fn ephemeral(&self) -> EphemeralState {
        self.ephemeral
    }
    pub(crate) fn first_turn_id(&self) -> Option<&str> {
        self.first_turn.as_ref().map(|key| key.turn_id.as_str())
    }
    pub(crate) fn first_turn_outcome(&self) -> FirstTurnOutcome {
        self.first_turn_outcome
    }
    pub(crate) fn is_standard_persisted_session(&self) -> bool {
        self.persistence == PersistenceState::PersistenceConfirmed
            && self.ephemeral != EphemeralState::Ephemeral
    }

    /// Input must come from the existing persisted rollout reader, not a start
    /// response, a filename, or Desktop projection metadata. No I/O is performed.
    pub(crate) fn observe_persisted_session_meta(
        &mut self,
        home: &CodexHomeIdentity,
        meta: &SessionMetaRecord,
    ) -> Result<(), CreationFailure> {
        if self.thread_key != CodexThreadKey::new(&home.identity, &meta.id) {
            return Err(CreationFailure::IdentityConflict);
        }
        if self.ephemeral == EphemeralState::Ephemeral {
            return Err(CreationFailure::EvidenceConflict);
        }
        self.persistence = PersistenceState::PersistenceConfirmed;
        Ok(())
    }
    pub(crate) fn observe_first_turn_accepted(
        &mut self,
        key: &CodexTurnKey,
    ) -> Result<(), CreationFailure> {
        if key.thread_key != self.thread_key
            || self.first_turn.as_ref().is_some_and(|first| first != key)
        {
            return Err(CreationFailure::IdentityConflict);
        }
        if !valid_full_id(&key.turn_id) {
            return Err(CreationFailure::InvalidResponse);
        }
        self.first_turn_acceptance = FirstTurnAcceptance::FirstTurnAccepted;
        self.first_turn = Some(key.clone());
        Ok(())
    }
    pub(crate) fn observe_first_turn_outcome(
        &mut self,
        key: Option<&CodexTurnKey>,
        outcome: FirstTurnOutcome,
    ) -> Result<(), CreationFailure> {
        match (key, outcome) {
            (None, FirstTurnOutcome::Rejected) if self.first_turn.is_none() => {}
            (
                Some(key),
                FirstTurnOutcome::Completed
                | FirstTurnOutcome::Failed
                | FirstTurnOutcome::Interrupted,
            ) if self.first_turn.as_ref() == Some(key) => {}
            _ => return Err(CreationFailure::IdentityConflict),
        }
        self.first_turn_outcome = outcome;
        Ok(())
    }
}

fn valid_full_id(id: &str) -> bool {
    // Current Codex full IDs are hyphenated UUIDs. Do not normalize the value
    // returned by the server or constrain its UUID version.
    id.len() == 36
        && [8, 13, 18, 23]
            .iter()
            .all(|index| id.as_bytes()[*index] == b'-')
        && uuid::Uuid::parse_str(id).is_ok_and(|id| !id.is_nil())
}

pub(crate) fn acknowledge_thread_start(
    home: &str,
    expected: Option<&CodexThreadKey>,
    response: &Value,
) -> Result<CreationAcknowledgement, CreationFailure> {
    if response.get("error").is_some() {
        return Err(CreationFailure::ServerRejected);
    }
    let thread = response
        .pointer("/result/thread")
        .filter(|thread| thread.is_object())
        .ok_or(CreationFailure::InvalidResponse)?;
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| valid_full_id(id))
        .ok_or(CreationFailure::InvalidResponse)?;
    if home.trim().is_empty() {
        return Err(CreationFailure::IdentityConflict);
    }
    let thread_key = CodexThreadKey::new(home, id);
    if expected.is_some_and(|expected| expected != &thread_key) {
        return Err(CreationFailure::IdentityConflict);
    }
    // Never silently select one of conflicting response identity locations.
    if let Some(other) = response.get("thread") {
        if other.get("id").and_then(Value::as_str) != Some(id) {
            return Err(CreationFailure::IdentityConflict);
        }
    }
    let ephemeral = match thread.get("ephemeral").and_then(Value::as_bool) {
        Some(true) => EphemeralState::Ephemeral,
        Some(false) => EphemeralState::NonEphemeral,
        None => EphemeralState::Unknown,
    };
    Ok(CreationAcknowledgement {
        state: CreationState::ThreadAcknowledged,
        thread_key,
        persistence: PersistenceState::NotYetConfirmed,
        ephemeral,
        first_turn_acceptance: FirstTurnAcceptance::NotYetAccepted,
        first_turn: None,
        first_turn_outcome: FirstTurnOutcome::Unknown,
    })
}

impl std::fmt::Display for CreationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::InvalidResponse => "INVALID_RESPONSE",
            Self::ServerRejected => "SERVER_REJECTED",
            Self::IdentityConflict => "IDENTITY_CONFLICT",
            Self::EvidenceConflict => "EVIDENCE_CONFLICT",
        };
        write!(f, "CREATION_FAILED / {reason}")
    }
}

/// Exactly one dispatch. Transport errors remain transport errors; this slice
/// performs no timeout classification, discovery, retry, or reconnect recovery.
pub(crate) async fn start_thread_with_acknowledgement<F, Fut>(
    home: &str,
    cwd: &str,
    send: F,
) -> Result<Value, String>
where
    F: FnOnce(&'static str, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    let mut response = send(
        "thread/start",
        json!({"cwd":cwd, "approvalPolicy":"on-request"}),
    )
    .await?;
    let acknowledgement =
        acknowledge_thread_start(home, None, &response).map_err(|error| error.to_string())?;
    response["result"]["creationAcknowledgement"] =
        serde_json::to_value(acknowledgement).map_err(|error| error.to_string())?;
    Ok(response)
}

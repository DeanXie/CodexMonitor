use super::approval_decision_provenance::ApprovalDecisionAttemptSnapshot;
use super::approval_observation::ApprovalObservationSnapshot;
use super::delete_mutation_observation::DeleteMutationObservation;
use super::thread_lifecycle_observation::{
    ThreadRuntimeAvailabilityObservationSnapshot, ThreadSubscriptionObservationSnapshot,
};
use super::writer_admission_observation::WriterAdmissionObservationSnapshot;
use crate::shared::codex_identity::CodexThreadKey;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A read-only, generation-scoped projection of the independent observation authorities.
///
/// The fields deliberately remain separate. This structure is a hydration payload, not a
/// combined business state machine and not evidence of writer/subscription ownership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoritativeObservationSnapshot {
    pub workspace_id: String,
    pub thread_key: CodexThreadKey,
    pub workspace_session_generation: String,
    pub app_server_connection_generation: String,
    pub writer: WriterAdmissionObservationSnapshot,
    pub subscription: ThreadSubscriptionObservationSnapshot,
    pub runtime: ThreadRuntimeAvailabilityObservationSnapshot,
    pub pending_approvals: Vec<ApprovalObservationSnapshot>,
    pub approval_history: Vec<ApprovalObservationSnapshot>,
    pub approval_decision_attempts: Vec<ApprovalDecisionAttemptSnapshot>,
    pub delete_observation: Option<DeleteMutationObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthoritativeObservationQueryError {
    WorkspaceIdRequired,
    FullThreadIdRequired,
    WorkspaceNotFound,
    WorkspaceSessionUnavailable,
}

impl fmt::Display for AuthoritativeObservationQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WorkspaceIdRequired => "workspaceId is required",
            Self::FullThreadIdRequired => "fullThreadId is required",
            Self::WorkspaceNotFound => "workspace not found",
            Self::WorkspaceSessionUnavailable => "workspace session unavailable",
        })
    }
}

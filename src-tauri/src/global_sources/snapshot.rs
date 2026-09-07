use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::shared::global_sources_core::source_registry::CanonicalSourceThread;
use crate::shared::surface_projection_core::{
    ObservationCoverage, ProjectionActionCapability, ProjectionMembershipExpectation,
    ProjectionReconciliationState, SurfaceProjectionKind, SurfaceProjectionObservation,
    SurfaceProjectionState, SurfaceProjectionSurface,
};
use crate::state::AppState;

pub(crate) const GLOBAL_SOURCE_SNAPSHOT_UPDATED_EVENT: &str = "global-source-snapshot-updated";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalSourceSnapshot {
    pub revision: u64,
    pub generated_at_ms: i64,
    pub workspace_codex_home_identities: HashMap<String, String>,
    pub threads: Vec<CanonicalSourceThread>,
    #[serde(default)]
    pub surface_projections: Vec<SurfaceProjectionSnapshotEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SurfaceProjectionSnapshotKey {
    pub thread_key: crate::shared::global_sources_core::rollout_identity::CodexThreadKey,
    pub surface: String,
    pub projection_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SurfaceProjectionSnapshotEntry {
    pub key: SurfaceProjectionSnapshotKey,
    pub state: String,
    pub coverage: String,
    pub observed_at: u64,
    pub provenance: Vec<String>,
    pub diagnostics: Vec<String>,
    pub reconciliation_state: String,
    pub action_capability: String,
    pub membership_expectation: String,
}

impl From<SurfaceProjectionObservation> for SurfaceProjectionSnapshotEntry {
    fn from(observation: SurfaceProjectionObservation) -> Self {
        Self {
            key: SurfaceProjectionSnapshotKey {
                thread_key: observation.key.thread_key,
                surface: surface_name(observation.key.surface).to_string(),
                projection_kind: projection_kind_name(observation.key.projection_kind).to_string(),
            },
            state: state_name(observation.state).to_string(),
            coverage: coverage_name(observation.coverage).to_string(),
            observed_at: observation.observed_at,
            provenance: observation.provenance,
            diagnostics: observation.diagnostics,
            reconciliation_state: reconciliation_name(observation.reconciliation_state).to_string(),
            action_capability: capability_name(observation.action_capability).to_string(),
            membership_expectation: expectation_name(observation.membership_expectation)
                .to_string(),
        }
    }
}

fn surface_name(value: SurfaceProjectionSurface) -> &'static str {
    match value {
        SurfaceProjectionSurface::Monitor => "MONITOR",
        SurfaceProjectionSurface::Desktop => "DESKTOP",
        SurfaceProjectionSurface::Cli => "CLI",
    }
}

fn projection_kind_name(value: SurfaceProjectionKind) -> &'static str {
    match value {
        SurfaceProjectionKind::SessionList => "SESSION_LIST",
        SurfaceProjectionKind::GlobalSourceSnapshot => "GLOBAL_SOURCE_SNAPSHOT",
        SurfaceProjectionKind::CurrentSession => "CURRENT_SESSION",
        SurfaceProjectionKind::HistoryList => "HISTORY_LIST",
        SurfaceProjectionKind::Catalog => "CATALOG",
        SurfaceProjectionKind::Sidebar => "SIDEBAR",
        SurfaceProjectionKind::Project => "PROJECT",
        SurfaceProjectionKind::Discoverability => "DISCOVERABILITY",
    }
}

fn state_name(value: SurfaceProjectionState) -> &'static str {
    match value {
        SurfaceProjectionState::Present => "PRESENT",
        SurfaceProjectionState::Absent => "ABSENT",
        SurfaceProjectionState::Stale => "STALE",
        SurfaceProjectionState::Unknown => "UNKNOWN",
        SurfaceProjectionState::NotApplicable => "NOT_APPLICABLE",
    }
}

fn coverage_name(value: ObservationCoverage) -> &'static str {
    match value {
        ObservationCoverage::Complete => "COMPLETE",
        ObservationCoverage::Bounded => "BOUNDED",
        ObservationCoverage::Partial => "PARTIAL",
        ObservationCoverage::Failed => "FAILED",
        ObservationCoverage::NotObserved => "NOT_OBSERVED",
        ObservationCoverage::NotApplicable => "NOT_APPLICABLE",
    }
}

fn reconciliation_name(value: ProjectionReconciliationState) -> &'static str {
    match value {
        ProjectionReconciliationState::NotRequired => "NOT_REQUIRED",
        ProjectionReconciliationState::Pending => "PENDING",
        ProjectionReconciliationState::Reconciled => "RECONCILED",
        ProjectionReconciliationState::Unknown => "UNKNOWN",
    }
}

fn capability_name(value: ProjectionActionCapability) -> &'static str {
    match value {
        ProjectionActionCapability::Refreshable => "REFRESHABLE",
        ProjectionActionCapability::Invalidatable => "INVALIDATABLE",
        ProjectionActionCapability::ObserveOnly => "OBSERVE_ONLY",
        ProjectionActionCapability::Unsupported => "UNSUPPORTED",
    }
}

fn expectation_name(value: ProjectionMembershipExpectation) -> &'static str {
    match value {
        ProjectionMembershipExpectation::Required => "REQUIRED",
        ProjectionMembershipExpectation::Optional => "OPTIONAL",
        ProjectionMembershipExpectation::Unknown => "UNKNOWN",
    }
}

#[tauri::command]
pub(crate) fn global_source_snapshot(state: State<'_, AppState>) -> GlobalSourceSnapshot {
    state.global_rollout_runtime.snapshot()
}

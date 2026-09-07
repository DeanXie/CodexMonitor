//! Phase 3.4.2 adapters from already-observed source results into the frozen
//! Surface projection contract.
//!
//! This module performs no source I/O and owns no canonical Thread authority.
//! Callers supply source results and separately resolved canonical authority.

use std::sync::{Arc, Mutex};

use super::global_sources_core::desktop_metadata::DesktopMetadataSnapshot;
use super::global_sources_core::desktop_projection::{
    DesktopProjectionAssessment, DesktopProjectionState,
};
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::global_sources_core::source_registry::CanonicalSourceSnapshot;
use super::surface_projection_core::{
    CanonicalThreadProjectionState, ObservationCoverage, ProjectionActionCapability,
    ProjectionMembershipExpectation, SurfaceProjectionKey, SurfaceProjectionKind,
    SurfaceProjectionObservation, SurfaceProjectionStore, SurfaceProjectionSurface,
    DESKTOP_STALE_ORPHAN_DIAGNOSTIC,
};
use super::workspace_interop_core::{DesktopProjectProjection, WorkspaceResolutionState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactIdProjectionResult {
    Present,
    AuthoritativeNotFound,
    Failed(String),
}

#[derive(Default)]
struct ProjectionObservationEngineState {
    store: SurfaceProjectionStore,
    known_keys: Vec<SurfaceProjectionKey>,
}

#[derive(Clone, Default)]
pub(crate) struct ProjectionObservationEngine {
    state: Arc<Mutex<ProjectionObservationEngineState>>,
}

impl ProjectionObservationEngine {
    pub(crate) fn observe(&self, observation: SurfaceProjectionObservation) -> bool {
        let mut state = self.lock();
        if !state.known_keys.contains(&observation.key) {
            state.known_keys.push(observation.key.clone());
        }
        state.store.observe(observation)
    }

    pub(crate) fn history(&self, key: &SurfaceProjectionKey) -> Vec<SurfaceProjectionObservation> {
        self.lock().store.history(key).to_vec()
    }

    pub(crate) fn effective(
        &self,
        key: &SurfaceProjectionKey,
        canonical_state: CanonicalThreadProjectionState,
    ) -> Option<SurfaceProjectionObservation> {
        self.lock().store.effective(key, canonical_state)
    }

    pub(crate) fn known_keys(
        &self,
        codex_home_identity: &str,
        surface: SurfaceProjectionSurface,
        projection_kind: SurfaceProjectionKind,
    ) -> Vec<SurfaceProjectionKey> {
        let mut keys = self
            .lock()
            .known_keys
            .iter()
            .filter(|key| {
                key.thread_key.codex_home_identity == codex_home_identity
                    && key.surface == surface
                    && key.projection_kind == projection_kind
            })
            .cloned()
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.thread_key.thread_id.cmp(&right.thread_key.thread_id));
        keys
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ProjectionObservationEngineState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) fn monitor_exact_read_observation(
    thread_key: CodexThreadKey,
    result: ExactIdProjectionResult,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    exact_id_observation(
        SurfaceProjectionKey::new(
            thread_key,
            SurfaceProjectionSurface::Monitor,
            SurfaceProjectionKind::CurrentSession,
        ),
        result,
        observed_at,
        "monitor.app-server.thread-read",
        ProjectionActionCapability::Refreshable,
        ProjectionMembershipExpectation::Required,
    )
}

pub(crate) fn monitor_list_observation(
    thread_key: CodexThreadKey,
    observed_thread_ids: &[String],
    coverage: ObservationCoverage,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    inventory_observation(
        SurfaceProjectionKey::new(
            thread_key.clone(),
            SurfaceProjectionSurface::Monitor,
            SurfaceProjectionKind::SessionList,
        ),
        observed_thread_ids,
        coverage,
        observed_at,
        "monitor.app-server.thread-list",
        ProjectionActionCapability::Refreshable,
        ProjectionMembershipExpectation::Optional,
        &thread_key.thread_id,
    )
}

pub(crate) fn global_source_snapshot_observation(
    thread_key: CodexThreadKey,
    snapshot: &CanonicalSourceSnapshot,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    SurfaceProjectionObservation::membership(
        SurfaceProjectionKey::new(
            thread_key.clone(),
            SurfaceProjectionSurface::Monitor,
            SurfaceProjectionKind::GlobalSourceSnapshot,
        ),
        snapshot
            .threads
            .iter()
            .any(|thread| thread.key == thread_key),
        ObservationCoverage::Complete,
        observed_at,
        vec!["monitor.global-source.snapshot".to_string()],
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
    )
}

pub(crate) fn desktop_catalog_observation(
    thread_key: CodexThreadKey,
    snapshot: &DesktopMetadataSnapshot,
    assessment: Option<&DesktopProjectionAssessment>,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    desktop_catalog_observation_with_expectation(
        thread_key,
        snapshot,
        assessment,
        observed_at,
        ProjectionMembershipExpectation::Optional,
    )
}

pub(crate) fn desktop_catalog_observation_with_expectation(
    thread_key: CodexThreadKey,
    snapshot: &DesktopMetadataSnapshot,
    assessment: Option<&DesktopProjectionAssessment>,
    observed_at: u64,
    membership_expectation: ProjectionMembershipExpectation,
) -> SurfaceProjectionObservation {
    if snapshot.codex_home_identity != thread_key.codex_home_identity {
        return SurfaceProjectionObservation::membership(
            SurfaceProjectionKey::new(
                thread_key,
                SurfaceProjectionSurface::Desktop,
                SurfaceProjectionKind::Catalog,
            ),
            false,
            ObservationCoverage::Failed,
            observed_at,
            vec!["desktop.metadata.local-thread-catalog".to_string()],
            ProjectionActionCapability::ObserveOnly,
            membership_expectation,
        )
        .with_diagnostic("desktop metadata CODEX_HOME identity mismatch");
    }
    let coverage = if snapshot.catalog_available {
        ObservationCoverage::Complete
    } else if snapshot.diagnostics.is_empty() {
        ObservationCoverage::NotObserved
    } else {
        ObservationCoverage::Failed
    };
    let ids = snapshot
        .catalog_entries
        .iter()
        .map(|entry| entry.thread_id.clone())
        .collect::<Vec<_>>();
    let mut observation = inventory_observation(
        SurfaceProjectionKey::new(
            thread_key.clone(),
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Catalog,
        ),
        &ids,
        coverage,
        observed_at,
        "desktop.metadata.local-thread-catalog",
        ProjectionActionCapability::ObserveOnly,
        membership_expectation,
        &thread_key.thread_id,
    );
    for diagnostic in &snapshot.diagnostics {
        observation = observation.with_diagnostic(format!(
            "{}:{}:{}",
            diagnostic.source, diagnostic.code, diagnostic.message
        ));
    }
    if assessment.is_some_and(|value| value.state == DesktopProjectionState::DesktopStaleOrphan) {
        observation = observation.with_diagnostic(DESKTOP_STALE_ORPHAN_DIAGNOSTIC);
    }
    observation
}

pub(crate) fn desktop_inventory_observation(
    thread_key: CodexThreadKey,
    projection_kind: SurfaceProjectionKind,
    observed_thread_ids: &[String],
    coverage: ObservationCoverage,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    inventory_observation(
        SurfaceProjectionKey::new(
            thread_key.clone(),
            SurfaceProjectionSurface::Desktop,
            projection_kind,
        ),
        observed_thread_ids,
        coverage,
        observed_at,
        "desktop.projection.inventory",
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
        &thread_key.thread_id,
    )
}

pub(crate) fn desktop_project_observation(
    projection: &DesktopProjectProjection,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    let (present, coverage) = match projection.state {
        WorkspaceResolutionState::Assigned => (true, ObservationCoverage::Complete),
        WorkspaceResolutionState::Unassigned => (false, ObservationCoverage::Complete),
        // Conflicting Project candidates still contain direct exact-thread
        // membership evidence; ambiguity belongs to Project identity, not
        // Surface projection presence.
        WorkspaceResolutionState::Ambiguous => (true, ObservationCoverage::Partial),
        WorkspaceResolutionState::Unknown => (false, ObservationCoverage::Failed),
    };
    let mut observation = SurfaceProjectionObservation::membership(
        SurfaceProjectionKey::new(
            projection.thread_key.clone(),
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Project,
        ),
        present,
        coverage,
        observed_at,
        projection.provenance.clone(),
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
    );
    for diagnostic in &projection.diagnostics {
        observation = observation.with_diagnostic(format!(
            "{}:{}:{}",
            diagnostic.source, diagnostic.code, diagnostic.message
        ));
    }
    observation
}

pub(crate) fn desktop_sidebar_observation(
    thread_key: CodexThreadKey,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    SurfaceProjectionObservation::membership(
        SurfaceProjectionKey::new(
            thread_key,
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Sidebar,
        ),
        false,
        ObservationCoverage::NotObserved,
        observed_at,
        vec!["desktop.sidebar.inventory-not-observed".to_string()],
        ProjectionActionCapability::Unsupported,
        ProjectionMembershipExpectation::Optional,
    )
}

pub(crate) fn cli_exact_id_observation(
    thread_key: CodexThreadKey,
    result: ExactIdProjectionResult,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    exact_id_observation(
        SurfaceProjectionKey::new(
            thread_key,
            SurfaceProjectionSurface::Cli,
            SurfaceProjectionKind::Discoverability,
        ),
        result,
        observed_at,
        "cli.exact-id",
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
    )
}

pub(crate) fn cli_picker_observation(
    thread_key: CodexThreadKey,
    observed_thread_ids: &[String],
    coverage: ObservationCoverage,
    observed_at: u64,
) -> SurfaceProjectionObservation {
    inventory_observation(
        SurfaceProjectionKey::new(
            thread_key.clone(),
            SurfaceProjectionSurface::Cli,
            SurfaceProjectionKind::HistoryList,
        ),
        observed_thread_ids,
        coverage,
        observed_at,
        "cli.picker-history",
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
        &thread_key.thread_id,
    )
}

fn exact_id_observation(
    key: SurfaceProjectionKey,
    result: ExactIdProjectionResult,
    observed_at: u64,
    provenance: &str,
    action_capability: ProjectionActionCapability,
    membership_expectation: ProjectionMembershipExpectation,
) -> SurfaceProjectionObservation {
    match result {
        ExactIdProjectionResult::Present => SurfaceProjectionObservation::membership(
            key,
            true,
            ObservationCoverage::Complete,
            observed_at,
            vec![provenance.to_string()],
            action_capability,
            membership_expectation,
        ),
        ExactIdProjectionResult::AuthoritativeNotFound => SurfaceProjectionObservation::membership(
            key,
            false,
            ObservationCoverage::Complete,
            observed_at,
            vec![provenance.to_string()],
            action_capability,
            membership_expectation,
        ),
        ExactIdProjectionResult::Failed(diagnostic) => SurfaceProjectionObservation::membership(
            key,
            false,
            ObservationCoverage::Failed,
            observed_at,
            vec![provenance.to_string()],
            action_capability,
            membership_expectation,
        )
        .with_diagnostic(diagnostic),
    }
}

#[allow(clippy::too_many_arguments)]
fn inventory_observation(
    key: SurfaceProjectionKey,
    observed_thread_ids: &[String],
    coverage: ObservationCoverage,
    observed_at: u64,
    provenance: &str,
    action_capability: ProjectionActionCapability,
    membership_expectation: ProjectionMembershipExpectation,
    exact_thread_id: &str,
) -> SurfaceProjectionObservation {
    SurfaceProjectionObservation::membership(
        key,
        observed_thread_ids.iter().any(|id| id == exact_thread_id),
        coverage,
        observed_at,
        vec![provenance.to_string()],
        action_capability,
        membership_expectation,
    )
}

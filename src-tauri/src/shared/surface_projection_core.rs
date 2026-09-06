//! Pure Phase 3.4 Surface projection observation contract.
//!
//! This module classifies already-observed Surface membership and reconciles it
//! against an already-resolved canonical Thread state. It performs no Desktop,
//! CLI, app-server, workspace, project, UI, or persistence I/O.

#![allow(dead_code)] // Phase 3.4.1 defines the contract before adapter ingestion.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::global_sources_core::rollout_identity::CodexThreadKey;

pub(crate) const DESKTOP_STALE_ORPHAN_DIAGNOSTIC: &str = "DESKTOP_STALE_ORPHAN";
pub(crate) const MISSING_PROJECTION_DIAGNOSTIC: &str = "MISSING_PROJECTION";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SurfaceProjectionSurface {
    Monitor,
    Desktop,
    Cli,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SurfaceProjectionKind {
    SessionList,
    GlobalSourceSnapshot,
    CurrentSession,
    HistoryList,
    Catalog,
    Sidebar,
    Project,
    Discoverability,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceProjectionKey {
    pub thread_key: CodexThreadKey,
    pub surface: SurfaceProjectionSurface,
    pub projection_kind: SurfaceProjectionKind,
}

impl SurfaceProjectionKey {
    pub(crate) fn new(
        thread_key: CodexThreadKey,
        surface: SurfaceProjectionSurface,
        projection_kind: SurfaceProjectionKind,
    ) -> Self {
        Self {
            thread_key,
            surface,
            projection_kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SurfaceProjectionState {
    Present,
    Absent,
    Stale,
    Unknown,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ObservationCoverage {
    Complete,
    Bounded,
    Partial,
    Failed,
    NotObserved,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ProjectionReconciliationState {
    NotRequired,
    Pending,
    Reconciled,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ProjectionActionCapability {
    Refreshable,
    Invalidatable,
    ObserveOnly,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ProjectionMembershipExpectation {
    Required,
    Optional,
    Unknown,
}

/// Canonical authority is resolved outside this model. Supplying one resolved
/// value keeps Surface projection evidence from becoming canonical authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CanonicalThreadProjectionState {
    Tombstoned,
    Present,
    Absent,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SurfaceProjectionObservation {
    pub key: SurfaceProjectionKey,
    pub state: SurfaceProjectionState,
    pub coverage: ObservationCoverage,
    pub observed_at: u64,
    pub provenance: Vec<String>,
    pub diagnostics: Vec<String>,
    pub reconciliation_state: ProjectionReconciliationState,
    pub action_capability: ProjectionActionCapability,
    pub membership_expectation: ProjectionMembershipExpectation,
}

impl SurfaceProjectionObservation {
    pub(crate) fn membership(
        key: SurfaceProjectionKey,
        exact_thread_id_present: bool,
        coverage: ObservationCoverage,
        observed_at: u64,
        provenance: Vec<String>,
        action_capability: ProjectionActionCapability,
        membership_expectation: ProjectionMembershipExpectation,
    ) -> Self {
        let state = if exact_thread_id_present {
            SurfaceProjectionState::Present
        } else if coverage == ObservationCoverage::Complete {
            SurfaceProjectionState::Absent
        } else {
            SurfaceProjectionState::Unknown
        };
        let reconciliation_state = if state == SurfaceProjectionState::Unknown {
            ProjectionReconciliationState::Unknown
        } else {
            ProjectionReconciliationState::NotRequired
        };
        Self {
            key,
            state,
            coverage,
            observed_at,
            provenance: normalized(provenance),
            diagnostics: Vec::new(),
            reconciliation_state,
            action_capability,
            membership_expectation,
        }
    }

    pub(crate) fn not_applicable(
        key: SurfaceProjectionKey,
        observed_at: u64,
        provenance: Vec<String>,
    ) -> Self {
        Self {
            key,
            state: SurfaceProjectionState::NotApplicable,
            coverage: ObservationCoverage::NotApplicable,
            observed_at,
            provenance: normalized(provenance),
            diagnostics: Vec::new(),
            reconciliation_state: ProjectionReconciliationState::NotRequired,
            action_capability: ProjectionActionCapability::Unsupported,
            membership_expectation: ProjectionMembershipExpectation::Optional,
        }
    }

    pub(crate) fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostics.push(diagnostic.into());
        self.diagnostics = normalized(self.diagnostics);
        self
    }
}

#[derive(Default)]
pub(crate) struct SurfaceProjectionStore {
    history_by_key: HashMap<SurfaceProjectionKey, Vec<SurfaceProjectionObservation>>,
}

impl SurfaceProjectionStore {
    pub(crate) fn observe(&mut self, observation: SurfaceProjectionObservation) -> bool {
        let history = self
            .history_by_key
            .entry(observation.key.clone())
            .or_default();
        if history.contains(&observation) {
            return false;
        }
        history.push(observation);
        true
    }

    pub(crate) fn history(&self, key: &SurfaceProjectionKey) -> &[SurfaceProjectionObservation] {
        self.history_by_key
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn effective(
        &self,
        key: &SurfaceProjectionKey,
        canonical_state: CanonicalThreadProjectionState,
    ) -> Option<SurfaceProjectionObservation> {
        let history = self.history(key);
        let latest = history
            .iter()
            .max_by(|left, right| compare_observations(left, right))?;
        let mut effective = latest.clone();
        let canonical_absent = matches!(
            canonical_state,
            CanonicalThreadProjectionState::Tombstoned | CanonicalThreadProjectionState::Absent
        );
        let prior_present = history.iter().any(|observation| {
            observation.observed_at < latest.observed_at
                && observation.state == SurfaceProjectionState::Present
        });

        if canonical_absent {
            match latest.state {
                SurfaceProjectionState::Present => {
                    effective.state = SurfaceProjectionState::Stale;
                    effective.reconciliation_state = ProjectionReconciliationState::Pending;
                    if is_desktop_stale_orphan_projection(key) {
                        effective
                            .diagnostics
                            .push(DESKTOP_STALE_ORPHAN_DIAGNOSTIC.to_string());
                    }
                }
                SurfaceProjectionState::Absent if prior_present => {
                    effective.reconciliation_state = ProjectionReconciliationState::Reconciled;
                }
                SurfaceProjectionState::Unknown if prior_present => {
                    effective.reconciliation_state = ProjectionReconciliationState::Pending;
                }
                SurfaceProjectionState::Absent | SurfaceProjectionState::NotApplicable => {
                    effective.reconciliation_state = ProjectionReconciliationState::NotRequired;
                }
                SurfaceProjectionState::Stale => {
                    effective.reconciliation_state = ProjectionReconciliationState::Pending;
                }
                SurfaceProjectionState::Unknown => {
                    effective.reconciliation_state = ProjectionReconciliationState::Unknown;
                }
            }
        } else if canonical_state == CanonicalThreadProjectionState::Present {
            effective.reconciliation_state = ProjectionReconciliationState::NotRequired;
            if effective.state == SurfaceProjectionState::Absent
                && effective.membership_expectation == ProjectionMembershipExpectation::Required
            {
                effective
                    .diagnostics
                    .push(MISSING_PROJECTION_DIAGNOSTIC.to_string());
            }
        } else {
            effective.reconciliation_state = ProjectionReconciliationState::Unknown;
        }

        effective.provenance = normalized(
            history
                .iter()
                .flat_map(|observation| observation.provenance.iter().cloned())
                .collect(),
        );
        effective.diagnostics = normalized(effective.diagnostics);
        Some(effective)
    }
}

fn is_desktop_stale_orphan_projection(key: &SurfaceProjectionKey) -> bool {
    key.surface == SurfaceProjectionSurface::Desktop
        && matches!(
            key.projection_kind,
            SurfaceProjectionKind::Catalog | SurfaceProjectionKind::Sidebar
        )
}

fn compare_observations(
    left: &SurfaceProjectionObservation,
    right: &SurfaceProjectionObservation,
) -> Ordering {
    left.observed_at
        .cmp(&right.observed_at)
        .then_with(|| left.state.cmp(&right.state))
        .then_with(|| left.coverage.cmp(&right.coverage))
        .then_with(|| left.provenance.cmp(&right.provenance))
        .then_with(|| left.diagnostics.cmp(&right.diagnostics))
        .then_with(|| left.action_capability.cmp(&right.action_capability))
        .then_with(|| {
            left.membership_expectation
                .cmp(&right.membership_expectation)
        })
}

fn normalized(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

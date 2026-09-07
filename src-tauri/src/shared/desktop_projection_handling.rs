//! Phase 3.4.3 Desktop stale/missing projection lifecycle handling.
//!
//! This pure coordinator consumes already-loaded deletion tombstones and
//! already-observed Desktop projection evidence. It performs no Desktop I/O,
//! owns no canonical Thread registry, and exposes no repair operation.

use std::collections::HashSet;

use super::global_sources_core::deletion_tombstone::DeletionTombstoneDocument;
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::surface_projection_core::{
    CanonicalThreadProjectionState, SurfaceProjectionKey, SurfaceProjectionObservation,
    SurfaceProjectionSurface,
};
use super::surface_projection_engine::ProjectionObservationEngine;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DesktopProjectionHandlingError {
    NonDesktopObservation,
}

#[derive(Clone, Default)]
pub(crate) struct DesktopProjectionHandling {
    engine: ProjectionObservationEngine,
    restored_tombstoned_thread_keys: HashSet<CodexThreadKey>,
}

impl DesktopProjectionHandling {
    /// Restores only the persisted deletion authority needed to reconcile
    /// Desktop projections after process restart. The caller remains
    /// responsible for loading and validating the tombstone document.
    pub(crate) fn restore(tombstones: &DeletionTombstoneDocument) -> Self {
        Self {
            engine: ProjectionObservationEngine::default(),
            restored_tombstoned_thread_keys: tombstones
                .operations
                .iter()
                .flat_map(|operation| operation.thread_keys().cloned())
                .collect(),
        }
    }

    pub(crate) fn observe(
        &self,
        observation: SurfaceProjectionObservation,
    ) -> Result<bool, DesktopProjectionHandlingError> {
        if observation.key.surface != SurfaceProjectionSurface::Desktop {
            return Err(DesktopProjectionHandlingError::NonDesktopObservation);
        }
        Ok(self.engine.observe(observation))
    }

    pub(crate) fn effective(
        &self,
        key: &SurfaceProjectionKey,
        externally_resolved_canonical_state: CanonicalThreadProjectionState,
    ) -> Option<SurfaceProjectionObservation> {
        let canonical_state = if self.is_tombstoned(&key.thread_key) {
            CanonicalThreadProjectionState::Tombstoned
        } else {
            externally_resolved_canonical_state
        };
        self.engine.effective(key, canonical_state)
    }

    pub(crate) fn is_tombstoned(&self, key: &CodexThreadKey) -> bool {
        self.restored_tombstoned_thread_keys.contains(key)
    }
}

#![allow(dead_code, unused_imports)]

mod desktop_project_projection;
mod locator;
mod relation;
mod resolver;
mod runtime_reconciliation;
mod value_types;

pub(crate) use desktop_project_projection::{
    resolve_desktop_project_projection, DesktopDirectProjectAssignment, DesktopProjectCandidate,
    DesktopProjectMigrationMappingState, DesktopProjectProjection, DesktopProjectProjectionInput,
};
pub(crate) use locator::{NormalizedRootLocator, RootLocatorPlatform};
pub(crate) use relation::{
    resolve_origin_workspace_relation, resolve_turn_execution_workspace_relation,
    OriginWorkspaceRelationInput, ParentWorkspaceFallback, ThreadWorkspaceProvenance,
    ThreadWorkspaceProvenanceKind, ThreadWorkspaceRelation, ThreadWorkspaceRelationBasis,
    ThreadWorkspaceRelationConfidence, ThreadWorkspaceRelationKey, ThreadWorkspaceRelationScope,
    ThreadWorkspaceRelationStore, TurnExecutionWorkspaceRelationInput,
    WorkspaceRelationObservation,
};
pub(crate) use resolver::{
    resolve_workspace_root, ConfiguredWorkspaceRoot, PhysicalIdentityStatus, WorkspaceResolution,
    WorkspaceResolutionInput, WorkspaceResolutionState,
};
pub(crate) use runtime_reconciliation::{
    RuntimeOriginWorkspaceObservation, RuntimeTurnWorkspaceObservation, RuntimeWorkspaceReconciler,
    RuntimeWorkspaceRoute,
};
pub(crate) use value_types::{ExecutionEnvironmentKey, WorkspaceKey};

#[cfg(test)]
#[path = "workspace_interop_core/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_interop_core/desktop_project_projection_tests.rs"]
mod desktop_project_projection_tests;

#[cfg(test)]
#[path = "workspace_interop_core/contract_fixture_tests.rs"]
mod contract_fixture_tests;

use super::WorkspaceResolutionState;
use crate::shared::global_sources_core::desktop_metadata::{
    DesktopMetadataDiagnostic, DesktopMetadataSnapshot, DesktopProjectMigrationState,
};
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DesktopProjectMigrationMappingState {
    Confirmed,
    Unresolved,
    NotApplicable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopDirectProjectAssignment {
    pub legacy_project_id: Option<String>,
    pub app_server_project_id: Option<String>,
    pub provenance: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopProjectCandidate {
    pub legacy_project_id: Option<String>,
    pub app_server_project_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopProjectProjection {
    pub thread_key: CodexThreadKey,
    pub state: WorkspaceResolutionState,
    pub legacy_project_id: Option<String>,
    pub app_server_project_id: Option<String>,
    pub explicit_thread_assignment: bool,
    pub direct_assignments: Vec<DesktopDirectProjectAssignment>,
    pub candidate_projects: Vec<DesktopProjectCandidate>,
    pub configured_roots: Vec<String>,
    pub migration_state: DesktopProjectMigrationState,
    pub migration_mapping: DesktopProjectMigrationMappingState,
    pub provenance: Vec<String>,
    pub diagnostics: Vec<DesktopMetadataDiagnostic>,
}

pub(crate) struct DesktopProjectProjectionInput<'a> {
    pub thread_key: &'a CodexThreadKey,
    pub desktop_host_identity: &'a str,
    pub metadata: &'a DesktopMetadataSnapshot,
}

pub(crate) fn resolve_desktop_project_projection(
    input: &DesktopProjectProjectionInput<'_>,
) -> DesktopProjectProjection {
    let migration_state = input
        .metadata
        .project_migrations_by_host
        .get(input.desktop_host_identity)
        .cloned()
        .unwrap_or_default();
    let aliases = input
        .metadata
        .project_id_mappings_by_host
        .get(input.desktop_host_identity);
    let mut direct_assignments = Vec::new();

    if let Some(legacy_project_id) = input
        .metadata
        .project_assignments
        .get(&input.thread_key.thread_id)
    {
        direct_assignments.push(DesktopDirectProjectAssignment {
            legacy_project_id: Some(legacy_project_id.clone()),
            app_server_project_id: aliases
                .and_then(|values| values.get(legacy_project_id))
                .cloned(),
            provenance: "desktop.global-state.thread-project-assignments".to_string(),
        });
    }

    if let Some(app_server_project_id) = input
        .metadata
        .persisted_threads
        .get(&input.thread_key.thread_id)
        .and_then(|thread| thread.project_id.as_ref())
    {
        let legacy_project_id = aliases.and_then(|values| {
            values
                .iter()
                .filter_map(|(legacy, app_server)| {
                    (app_server == app_server_project_id).then_some(legacy)
                })
                .min()
                .cloned()
        });
        direct_assignments.push(DesktopDirectProjectAssignment {
            legacy_project_id,
            app_server_project_id: Some(app_server_project_id.clone()),
            provenance: "desktop.state-5.threads.project-id".to_string(),
        });
    }

    let mut candidates = BTreeMap::<String, DesktopProjectCandidate>::new();
    for assignment in &direct_assignments {
        let canonical_key = assignment
            .app_server_project_id
            .as_ref()
            .map(|id| format!("app-server:{id}"))
            .or_else(|| {
                assignment
                    .legacy_project_id
                    .as_ref()
                    .map(|id| format!("legacy:{id}"))
            });
        let Some(canonical_key) = canonical_key else {
            continue;
        };
        let candidate =
            candidates
                .entry(canonical_key)
                .or_insert_with(|| DesktopProjectCandidate {
                    legacy_project_id: assignment.legacy_project_id.clone(),
                    app_server_project_id: assignment.app_server_project_id.clone(),
                });
        if candidate.legacy_project_id.is_none() {
            candidate.legacy_project_id = assignment.legacy_project_id.clone();
        }
        if candidate.app_server_project_id.is_none() {
            candidate.app_server_project_id = assignment.app_server_project_id.clone();
        }
    }
    let candidate_projects = candidates.into_values().collect::<Vec<_>>();
    let assignment_source_complete = match migration_state.thread_assignments_migrated {
        Some(false) => input.metadata.legacy_project_assignments_available,
        Some(true) => {
            input.metadata.persisted_state_available
                && input.metadata.persisted_project_id_available
        }
        None => false,
    };
    let state = match candidate_projects.len() {
        0 if assignment_source_complete => WorkspaceResolutionState::Unassigned,
        0 => WorkspaceResolutionState::Unknown,
        1 => WorkspaceResolutionState::Assigned,
        _ => WorkspaceResolutionState::Ambiguous,
    };
    let selected = (state == WorkspaceResolutionState::Assigned)
        .then(|| candidate_projects.first())
        .flatten();
    let legacy_project_id = selected.and_then(|candidate| candidate.legacy_project_id.clone());
    let app_server_project_id =
        selected.and_then(|candidate| candidate.app_server_project_id.clone());
    let migration_mapping = match selected {
        Some(candidate)
            if candidate.legacy_project_id.is_some()
                && candidate.app_server_project_id.is_some() =>
        {
            DesktopProjectMigrationMappingState::Confirmed
        }
        Some(_) => DesktopProjectMigrationMappingState::Unresolved,
        None if candidate_projects.is_empty() => DesktopProjectMigrationMappingState::NotApplicable,
        None if candidate_projects.iter().all(|candidate| {
            candidate.legacy_project_id.is_some() && candidate.app_server_project_id.is_some()
        }) =>
        {
            DesktopProjectMigrationMappingState::Confirmed
        }
        None => DesktopProjectMigrationMappingState::Unresolved,
    };
    let mut configured_roots = legacy_project_id
        .as_ref()
        .and_then(|project_id| input.metadata.projects.get(project_id))
        .map(|project| project.root_paths.clone())
        .unwrap_or_default();
    configured_roots.sort();
    configured_roots.dedup();
    let provenance = direct_assignments
        .iter()
        .map(|assignment| assignment.provenance.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    DesktopProjectProjection {
        thread_key: input.thread_key.clone(),
        state,
        legacy_project_id,
        app_server_project_id,
        explicit_thread_assignment: !direct_assignments.is_empty(),
        direct_assignments,
        candidate_projects,
        configured_roots,
        migration_state,
        migration_mapping,
        provenance,
        diagnostics: input.metadata.diagnostics.clone(),
    }
}

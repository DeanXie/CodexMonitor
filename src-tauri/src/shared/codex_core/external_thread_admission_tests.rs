use super::external_thread_admission::{
    ExternalThreadAdmissionEvidence, ExternalThreadAdmissionRegistry, KnowledgeState, Surface,
    SurfaceProjectionEvidence, WriterOccupancy,
};
use crate::shared::global_sources_core::desktop_projection::{
    resolve_workspace_assignment, WorkspaceAssignment, WorkspaceAssignmentState,
    WorkspaceMappingInput, WorkspaceRoot,
};
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;

fn key(thread_id: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home-a", thread_id)
}

fn unassigned_workspace() -> WorkspaceAssignment {
    WorkspaceAssignment {
        state: WorkspaceAssignmentState::Unassigned,
        workspace_id: None,
        provenance: "no-workspace-evidence".to_string(),
        matched_path: None,
        candidate_workspace_ids: Vec::new(),
    }
}

fn evidence(thread_id: &str) -> ExternalThreadAdmissionEvidence {
    ExternalThreadAdmissionEvidence {
        thread_key: key(thread_id),
        title: None,
        exact_read_exists: None,
        tombstoned: false,
        workspace_assignment: unassigned_workspace(),
        surface_projection: None,
        writer_occupancy: None,
    }
}

#[test]
fn same_full_id_from_multiple_surfaces_has_one_admission_record() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut desktop = evidence("thread-a");
    desktop.surface_projection = Some(SurfaceProjectionEvidence {
        surface: Surface::Desktop,
        project_assigned: None,
        sidebar_visible: Some(true),
    });
    let mut cli = evidence("thread-a");
    cli.surface_projection = Some(SurfaceProjectionEvidence {
        surface: Surface::Cli,
        project_assigned: None,
        sidebar_visible: None,
    });

    registry.observe(desktop);
    registry.observe(cli);

    assert_eq!(registry.len(), 1);
    let state = registry.get(&key("thread-a")).expect("canonical record");
    assert_eq!(state.surface_projections.len(), 2);
}

#[test]
fn same_title_with_different_full_ids_stays_independent() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut first = evidence("thread-a");
    first.title = Some("same title".to_string());
    let mut second = evidence("thread-b");
    second.title = Some("same title".to_string());

    registry.observe(first);
    registry.observe(second);

    assert_eq!(registry.len(), 2);
    assert!(registry.get(&key("thread-a")).is_some());
    assert!(registry.get(&key("thread-b")).is_some());
}

#[test]
fn tombstone_overrides_exists_and_resumable_evidence() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut deleted = evidence("thread-a");
    deleted.tombstoned = true;
    registry.observe(deleted);
    let mut later_read = evidence("thread-a");
    later_read.exact_read_exists = Some(true);
    registry.observe(later_read);

    let state = registry.get(&key("thread-a")).expect("retired record");
    assert_eq!(state.exists, KnowledgeState::No);
    assert_eq!(state.resumable, KnowledgeState::No);
}

#[test]
fn confirmed_exact_read_marks_exists_and_resumable() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.exact_read_exists = Some(true);

    let state = registry.observe(observed);

    assert_eq!(state.exists, KnowledgeState::Yes);
    assert_eq!(state.resumable, KnowledgeState::Yes);
}

#[test]
fn unavailable_read_keeps_exists_and_resumable_unknown() {
    let mut registry = ExternalThreadAdmissionRegistry::default();

    let state = registry.observe(evidence("thread-a"));

    assert_eq!(state.exists, KnowledgeState::Unknown);
    assert_eq!(state.resumable, KnowledgeState::Unknown);
}

#[test]
fn workspace_assignment_preserves_longest_root_match() {
    let roots = vec![
        WorkspaceRoot::new("parent", "F:/AI"),
        WorkspaceRoot::new("specific", "F:/AI/CodexMonitor"),
    ];
    let assignment = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: Some("F:/AI/CodexMonitor/src-tauri"),
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: None,
    });
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.workspace_assignment = assignment;

    let state = registry.observe(observed);

    assert_eq!(
        state.workspace_assignment.state,
        WorkspaceAssignmentState::Assigned
    );
    assert_eq!(
        state.workspace_assignment.workspace_id.as_deref(),
        Some("specific")
    );
}

#[test]
fn ambiguous_equal_longest_roots_remain_unassigned() {
    let roots = vec![
        WorkspaceRoot::new("first", "F:/AI/CodexMonitor"),
        WorkspaceRoot::new("second", "F:/AI/CodexMonitor"),
    ];
    let assignment = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: Some("F:/AI/CodexMonitor/src-tauri"),
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: None,
    });
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.workspace_assignment = assignment;

    let state = registry.observe(observed);

    assert_eq!(
        state.workspace_assignment.state,
        WorkspaceAssignmentState::Ambiguous
    );
    assert_eq!(state.workspace_assignment.workspace_id, None);
}

#[test]
fn writer_and_occupancy_stay_unknown_without_direct_evidence() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.exact_read_exists = Some(true);

    let state = registry.observe(observed);

    assert_eq!(state.writer_occupancy, WriterOccupancy::Unknown);
}

#[test]
fn surface_projection_evidence_sets_project_and_sidebar_state() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.surface_projection = Some(SurfaceProjectionEvidence {
        surface: Surface::Desktop,
        project_assigned: Some(true),
        sidebar_visible: Some(false),
    });

    let state = registry.observe(observed);

    assert_eq!(state.project_assigned, KnowledgeState::Yes);
    assert_eq!(state.sidebar_visible, KnowledgeState::No);
}

#[test]
fn cwd_or_catalog_presence_cannot_infer_project_or_sidebar_state() {
    let mut registry = ExternalThreadAdmissionRegistry::default();
    let mut observed = evidence("thread-a");
    observed.workspace_assignment = WorkspaceAssignment {
        state: WorkspaceAssignmentState::Assigned,
        workspace_id: Some("workspace-from-cwd".to_string()),
        provenance: "rollout-cwd-longest-root".to_string(),
        matched_path: Some("f:/ai/codexmonitor".to_string()),
        candidate_workspace_ids: vec!["workspace-from-cwd".to_string()],
    };
    observed.surface_projection = Some(SurfaceProjectionEvidence {
        surface: Surface::Desktop,
        project_assigned: None,
        sidebar_visible: None,
    });

    let state = registry.observe(observed);

    assert_eq!(state.project_assigned, KnowledgeState::Unknown);
    assert_eq!(state.sidebar_visible, KnowledgeState::Unknown);
}

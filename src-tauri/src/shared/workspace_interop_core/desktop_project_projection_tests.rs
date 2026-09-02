use super::*;
use crate::shared::global_sources_core::desktop_metadata::{
    DesktopMetadataPaths, DesktopMetadataReader, DesktopMetadataSnapshot, DesktopPersistedThread,
    DesktopProjectMetadata, DesktopProjectMigrationState,
};
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const HOST: &str = "local:fixture";
const THREAD: &str = "thread-fixture";

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home:fixture", THREAD)
}

fn complete_legacy_snapshot() -> DesktopMetadataSnapshot {
    DesktopMetadataSnapshot {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_available: true,
        legacy_project_assignments_available: true,
        project_migrations_by_host: HashMap::from([(
            HOST.to_string(),
            DesktopProjectMigrationState {
                projects_migrated: Some(true),
                thread_assignments_migrated: Some(false),
                version: Some(7),
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    }
}

fn resolve_projection(snapshot: &DesktopMetadataSnapshot) -> DesktopProjectProjection {
    resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &thread_key(),
        desktop_host_identity: HOST,
        metadata: snapshot,
    })
}

fn add_legacy_assignment(snapshot: &mut DesktopMetadataSnapshot, project_id: &str) {
    snapshot
        .project_assignments
        .insert(THREAD.to_string(), project_id.to_string());
}

fn add_app_server_assignment(snapshot: &mut DesktopMetadataSnapshot, project_id: Option<&str>) {
    snapshot.persisted_project_id_available = true;
    snapshot.persisted_threads.insert(
        THREAD.to_string(),
        DesktopPersistedThread {
            thread_id: THREAD.to_string(),
            project_id: project_id.map(str::to_string),
            ..DesktopPersistedThread::default()
        },
    );
}

fn map_alias(snapshot: &mut DesktopMetadataSnapshot, legacy: &str, app_server: &str) {
    snapshot
        .project_id_mappings_by_host
        .entry(HOST.to_string())
        .or_default()
        .insert(legacy.to_string(), app_server.to_string());
}

fn assigned_workspace() -> WorkspaceResolution {
    resolve_workspace_root(&WorkspaceResolutionInput {
        cwd: r"C:\Repo\src",
        platform: RootLocatorPlatform::Windows,
        execution_environment_key: &ExecutionEnvironmentKey::new("local:fixture").unwrap(),
        configured_roots: &[ConfiguredWorkspaceRoot::new(r"C:\Repo")],
    })
}

#[test]
fn legacy_project_maps_to_app_server_project() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");
    map_alias(&mut snapshot, "legacy-a", "app-project-p");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.legacy_project_id.as_deref(), Some("legacy-a"));
    assert_eq!(
        projection.app_server_project_id.as_deref(),
        Some("app-project-p")
    );
    assert_eq!(
        projection.migration_mapping,
        DesktopProjectMigrationMappingState::Confirmed
    );
}

#[test]
fn desktop_metadata_reader_parses_project_alias_and_migration_contract() {
    let snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &json!({
            "thread-project-assignments": {
                THREAD: { "projectKind": "local", "projectId": "legacy-a" }
            },
            "local-projects": {
                "legacy-a": {
                    "id": "legacy-a",
                    "name": "Fixture",
                    "rootPaths": [r"C:\Repo"]
                }
            },
            "app-server-project-id-by-legacy-project-id-by-host": {
                HOST: { "legacy-a": "app-project-p" }
            },
            "app-server-projects-migration-by-host": {
                HOST: {
                    "projectsMigrated": true,
                    "threadAssignmentsMigrated": false,
                    "version": 7
                }
            }
        }),
    );

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(
        projection.app_server_project_id.as_deref(),
        Some("app-project-p")
    );
    assert_eq!(projection.configured_roots, vec![r"C:\Repo"]);
    assert_eq!(projection.migration_state.projects_migrated, Some(true));
    assert_eq!(
        projection.migration_state.thread_assignments_migrated,
        Some(false)
    );
}

#[test]
fn missing_project_mapping_remains_unknown() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.legacy_project_id.as_deref(), Some("legacy-a"));
    assert!(projection.app_server_project_id.is_none());
    assert_eq!(
        projection.migration_mapping,
        DesktopProjectMigrationMappingState::Unresolved
    );
}

#[test]
fn legacy_direct_assignment_is_assigned_even_when_app_server_mapping_missing() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.candidate_projects.len(), 1);
    assert_eq!(projection.legacy_project_id.as_deref(), Some("legacy-a"));
    assert!(projection.app_server_project_id.is_none());
}

#[test]
fn explicit_thread_project_assignment_is_assigned() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.direct_assignments.len(), 1);
    assert_eq!(
        projection.direct_assignments[0].provenance,
        "desktop.global-state.thread-project-assignments"
    );
}

#[test]
fn no_assignment_after_complete_projection_read_is_unassigned() {
    let projection = resolve_projection(&complete_legacy_snapshot());

    assert_eq!(projection.state, WorkspaceResolutionState::Unassigned);
    assert!(projection.candidate_projects.is_empty());
}

#[test]
fn sqlite_null_project_id_does_not_override_legacy_assignment() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");
    add_app_server_assignment(&mut snapshot, None);

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.legacy_project_id.as_deref(), Some("legacy-a"));
}

#[test]
fn app_server_only_direct_assignment_is_assigned() {
    let mut snapshot = DesktopMetadataSnapshot {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_available: true,
        persisted_state_available: true,
        persisted_project_id_available: true,
        project_migrations_by_host: HashMap::from([(
            HOST.to_string(),
            DesktopProjectMigrationState {
                projects_migrated: Some(true),
                thread_assignments_migrated: Some(true),
                version: None,
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    };
    add_app_server_assignment(&mut snapshot, Some("app-project-p"));

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert!(projection.legacy_project_id.is_none());
    assert_eq!(
        projection.app_server_project_id.as_deref(),
        Some("app-project-p")
    );
}

#[test]
fn migrated_app_server_assignment_source_can_confirm_unassigned() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_available: true,
        persisted_state_available: true,
        persisted_project_id_available: true,
        project_migrations_by_host: HashMap::from([(
            HOST.to_string(),
            DesktopProjectMigrationState {
                projects_migrated: Some(true),
                thread_assignments_migrated: Some(true),
                version: None,
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    };

    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Unassigned
    );
}

#[test]
fn migrated_app_server_assignment_schema_drift_remains_unknown() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_available: true,
        persisted_state_available: true,
        persisted_project_id_available: false,
        project_migrations_by_host: HashMap::from([(
            HOST.to_string(),
            DesktopProjectMigrationState {
                projects_migrated: Some(true),
                thread_assignments_migrated: Some(true),
                version: None,
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    };

    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Unknown
    );
}

#[test]
fn legacy_and_app_server_aliases_of_same_project_do_not_create_ambiguity() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");
    add_app_server_assignment(&mut snapshot, Some("app-project-p"));
    map_alias(&mut snapshot, "legacy-a", "app-project-p");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.direct_assignments.len(), 2);
    assert_eq!(projection.candidate_projects.len(), 1);
}

#[test]
fn multiple_distinct_direct_project_candidates_are_ambiguous() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");
    add_app_server_assignment(&mut snapshot, Some("app-project-other"));
    map_alias(&mut snapshot, "legacy-a", "app-project-p");

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Ambiguous);
    assert_eq!(projection.candidate_projects.len(), 2);
    assert!(projection.legacy_project_id.is_none());
    assert!(projection.app_server_project_id.is_none());
}

#[test]
fn conflicting_direct_assignments_are_ambiguous() {
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");
    add_app_server_assignment(&mut snapshot, Some("app-project-b"));

    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Ambiguous
    );
}

#[test]
fn overlapping_project_roots_do_not_create_assignment() {
    let mut snapshot = complete_legacy_snapshot();
    snapshot.projects.insert(
        "legacy-a".to_string(),
        DesktopProjectMetadata {
            project_id: "legacy-a".to_string(),
            name: Some("A".to_string()),
            root_paths: vec![r"C:\Repo".to_string()],
        },
    );
    snapshot.projects.insert(
        "legacy-b".to_string(),
        DesktopProjectMetadata {
            project_id: "legacy-b".to_string(),
            name: Some("B".to_string()),
            root_paths: vec![r"C:\Repo".to_string()],
        },
    );

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Unassigned);
    assert!(projection.configured_roots.is_empty());
}

#[test]
fn workspace_assignment_does_not_imply_project_assignment() {
    let workspace = assigned_workspace();
    let project = resolve_projection(&complete_legacy_snapshot());

    assert_eq!(workspace.state, WorkspaceResolutionState::Assigned);
    assert_eq!(project.state, WorkspaceResolutionState::Unassigned);
}

#[test]
fn project_assignment_does_not_change_workspace_key() {
    let workspace = assigned_workspace();
    let original_key = workspace.workspace_key.clone();
    let mut snapshot = complete_legacy_snapshot();
    add_legacy_assignment(&mut snapshot, "legacy-a");

    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Assigned
    );
    assert_eq!(workspace.workspace_key, original_key);
}

#[test]
fn migration_state_is_preserved() {
    let projection = resolve_projection(&complete_legacy_snapshot());

    assert_eq!(projection.migration_state.projects_migrated, Some(true));
    assert_eq!(
        projection.migration_state.thread_assignments_migrated,
        Some(false)
    );
    assert_eq!(projection.migration_state.version, Some(7));
}

#[test]
fn missing_or_malformed_global_state_degrades_to_unknown() {
    for contents in [None, Some("{not-json")] {
        let root = temp_root();
        if let Some(contents) = contents {
            fs::write(root.join(".codex-global-state.json"), contents).unwrap();
        }
        let snapshot = DesktopMetadataReader::read(&DesktopMetadataPaths::for_codex_home(
            "codex-home:fixture",
            &root,
        ));

        assert_eq!(
            resolve_projection(&snapshot).state,
            WorkspaceResolutionState::Unknown
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn sqlite_schema_drift_does_not_block_workspace_runtime() {
    let root = temp_root();
    write_complete_legacy_global_state(&root, json!({}));
    let sqlite = root.join("sqlite");
    fs::create_dir_all(&sqlite).unwrap();
    rusqlite::Connection::open(sqlite.join("state_5.sqlite"))
        .unwrap()
        .execute("CREATE TABLE threads (unexpected TEXT)", [])
        .unwrap();
    let snapshot = DesktopMetadataReader::read(&DesktopMetadataPaths::for_codex_home(
        "codex-home:fixture",
        &root,
    ));

    assert_eq!(
        assigned_workspace().state,
        WorkspaceResolutionState::Assigned
    );
    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Unassigned
    );
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "private-schema-drift"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_is_read_only() {
    let root = temp_root();
    write_complete_legacy_global_state(&root, json!({}));
    let sqlite = root.join("sqlite");
    fs::create_dir_all(&sqlite).unwrap();
    let state_path = sqlite.join("state_5.sqlite");
    rusqlite::Connection::open(&state_path)
        .unwrap()
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, project_id TEXT)",
            [],
        )
        .unwrap();
    let global_path = root.join(".codex-global-state.json");
    let global_before = fs::read(&global_path).unwrap();
    let sqlite_before = fs::read(&state_path).unwrap();

    let _ = DesktopMetadataReader::read(&DesktopMetadataPaths::for_codex_home(
        "codex-home:fixture",
        &root,
    ));

    assert_eq!(fs::read(&global_path).unwrap(), global_before);
    assert_eq!(fs::read(&state_path).unwrap(), sqlite_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn same_thread_can_have_workspace_assigned_but_project_unassigned() {
    let workspace = assigned_workspace();
    let project = resolve_projection(&complete_legacy_snapshot());

    assert!(workspace.workspace_key.is_some());
    assert_eq!(project.state, WorkspaceResolutionState::Unassigned);
    assert!(project.legacy_project_id.is_none());
    assert!(project.app_server_project_id.is_none());
}

#[test]
fn migration_unknown_without_direct_assignment_keeps_assignment_unknown() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_available: true,
        legacy_project_assignments_available: true,
        ..DesktopMetadataSnapshot::default()
    };

    assert_eq!(
        resolve_projection(&snapshot).state,
        WorkspaceResolutionState::Unknown
    );
}

#[test]
fn malformed_migration_state_is_diagnostic_and_keeps_assignment_unknown() {
    let snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &json!({
            "thread-project-assignments": {},
            "app-server-projects-migration-by-host": {
                HOST: {
                    "projectsMigrated": true,
                    "threadAssignmentsMigrated": "not-a-boolean"
                }
            }
        }),
    );

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Unknown);
    assert!(projection
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "private-schema-drift"));
}

#[test]
fn malformed_legacy_assignment_record_keeps_assignment_unknown() {
    let snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &json!({
            "thread-project-assignments": {
                THREAD: { "projectKind": "local", "projectId": 42 }
            },
            "app-server-projects-migration-by-host": {
                HOST: {
                    "projectsMigrated": true,
                    "threadAssignmentsMigrated": false
                }
            }
        }),
    );

    let projection = resolve_projection(&snapshot);

    assert_eq!(projection.state, WorkspaceResolutionState::Unknown);
    assert!(projection
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "private-schema-drift"));
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "codex-monitor-phase-3-2-4-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_complete_legacy_global_state(root: &Path, assignments: serde_json::Value) {
    fs::write(
        root.join(".codex-global-state.json"),
        serde_json::to_vec(&json!({
            "thread-project-assignments": assignments,
            "local-projects": {},
            "app-server-project-id-by-legacy-project-id-by-host": {
                HOST: {}
            },
            "app-server-projects-migration-by-host": {
                HOST: {
                    "projectsMigrated": true,
                    "threadAssignmentsMigrated": false,
                    "version": 7
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

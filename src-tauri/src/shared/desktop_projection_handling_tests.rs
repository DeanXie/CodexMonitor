use std::collections::HashMap;

use super::desktop_projection_handling::{
    DesktopProjectionHandling, DesktopProjectionHandlingError,
};
use super::global_sources_core::deletion_tombstone::{
    DeletionTombstone, DeletionTombstoneDocument, DeletionTombstoneStore,
};
use super::global_sources_core::desktop_metadata::{
    DesktopCatalogEntry, DesktopMetadataSnapshot, DesktopProjectMigrationState,
};
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::surface_projection_core::{
    CanonicalThreadProjectionState, ObservationCoverage, ProjectionActionCapability,
    ProjectionMembershipExpectation, ProjectionReconciliationState, SurfaceProjectionKind,
    SurfaceProjectionState, DESKTOP_STALE_ORPHAN_DIAGNOSTIC, MISSING_PROJECTION_DIAGNOSTIC,
};
use super::surface_projection_engine::{
    desktop_catalog_observation, desktop_catalog_observation_with_expectation,
    desktop_inventory_observation, desktop_project_observation, desktop_sidebar_observation,
    monitor_list_observation,
};
use super::workspace_interop_core::{
    resolve_desktop_project_projection, DesktopProjectProjectionInput, WorkspaceResolutionState,
};

const HOME: &str = "codex-home:phase-3-4-3";

fn thread(id: &str) -> CodexThreadKey {
    CodexThreadKey::new(HOME, id)
}

fn tombstones(key: &CodexThreadKey) -> DeletionTombstoneDocument {
    let mut document = DeletionTombstoneDocument::default();
    document.upsert(DeletionTombstone::confirmed(
        "delete-operation",
        key.clone(),
        Vec::new(),
        1,
    ));
    document
}

fn catalog_snapshot(ids: &[&str]) -> DesktopMetadataSnapshot {
    DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: ids
            .iter()
            .map(|id| DesktopCatalogEntry {
                thread_id: (*id).to_string(),
                ..DesktopCatalogEntry::default()
            })
            .collect(),
        ..DesktopMetadataSnapshot::default()
    }
}

#[test]
fn tombstoned_thread_with_desktop_catalog_present_is_stale_pending() {
    let key = thread("thread-stale");
    let handling = DesktopProjectionHandling::restore(&tombstones(&key));
    let observed = desktop_catalog_observation(key, &catalog_snapshot(&["thread-stale"]), None, 10);
    handling.observe(observed.clone()).unwrap();

    let effective = handling
        .effective(&observed.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Pending
    );
    assert!(effective
        .diagnostics
        .contains(&DESKTOP_STALE_ORPHAN_DIAGNOSTIC.to_string()));
}

#[test]
fn stale_desktop_projection_cannot_revive_thread() {
    let key = thread("thread-deleted");
    let handling = DesktopProjectionHandling::restore(&tombstones(&key));
    let observed = desktop_catalog_observation(
        key.clone(),
        &catalog_snapshot(&["thread-deleted"]),
        None,
        10,
    );
    handling.observe(observed.clone()).unwrap();

    assert!(handling.is_tombstoned(&key));
    assert_eq!(
        handling
            .effective(&observed.key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Stale
    );
}

#[test]
fn later_complete_catalog_absence_marks_reconciled() {
    let key = thread("thread-deleted");
    let handling = DesktopProjectionHandling::restore(&tombstones(&key));
    let present = desktop_catalog_observation(
        key.clone(),
        &catalog_snapshot(&["thread-deleted"]),
        None,
        10,
    );
    let projection_key = present.key.clone();
    handling.observe(present).unwrap();
    handling
        .observe(desktop_catalog_observation(
            key,
            &catalog_snapshot(&[]),
            None,
            20,
        ))
        .unwrap();

    let effective = handling
        .effective(&projection_key, CanonicalThreadProjectionState::Unknown)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Reconciled
    );
}

#[test]
fn bounded_desktop_miss_stays_unknown() {
    let key = thread("thread-bounded");
    let observation = desktop_inventory_observation(
        key,
        SurfaceProjectionKind::Catalog,
        &[],
        ObservationCoverage::Bounded,
        10,
    );
    let handling = DesktopProjectionHandling::default();
    handling.observe(observation.clone()).unwrap();

    assert_eq!(
        handling
            .effective(&observation.key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Unknown
    );
}

#[test]
fn canonical_present_desktop_absent_does_not_mean_thread_missing() {
    let key = thread("thread-canonical");
    let observation = desktop_catalog_observation(key.clone(), &catalog_snapshot(&[]), None, 10);
    let handling = DesktopProjectionHandling::default();
    handling.observe(observation.clone()).unwrap();

    let effective = handling
        .effective(&observation.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert_eq!(effective.key.thread_key, key);
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert!(!handling.is_tombstoned(&effective.key.thread_key));
}

#[test]
fn absent_projection_without_membership_expectation_has_no_missing_diagnostic() {
    let observation =
        desktop_catalog_observation(thread("thread-optional"), &catalog_snapshot(&[]), None, 10);
    let handling = DesktopProjectionHandling::default();
    handling.observe(observation.clone()).unwrap();

    let effective = handling
        .effective(&observation.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert!(!effective
        .diagnostics
        .contains(&MISSING_PROJECTION_DIAGNOSTIC.to_string()));
}

#[test]
fn expected_but_absent_projection_can_add_missing_projection_diagnostic() {
    let observation = desktop_catalog_observation_with_expectation(
        thread("thread-required"),
        &catalog_snapshot(&[]),
        None,
        10,
        ProjectionMembershipExpectation::Required,
    );
    let handling = DesktopProjectionHandling::default();
    handling.observe(observation.clone()).unwrap();

    let effective = handling
        .effective(&observation.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert!(effective
        .diagnostics
        .contains(&MISSING_PROJECTION_DIAGNOSTIC.to_string()));
}

#[test]
fn desktop_sidebar_unknown_does_not_follow_catalog_absence() {
    let key = thread("thread-independent");
    let catalog = desktop_catalog_observation(key.clone(), &catalog_snapshot(&[]), None, 10);
    let sidebar = desktop_sidebar_observation(key, 10);
    let handling = DesktopProjectionHandling::default();
    handling.observe(catalog.clone()).unwrap();
    handling.observe(sidebar.clone()).unwrap();

    assert_eq!(
        handling
            .effective(&catalog.key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Absent
    );
    assert_eq!(
        handling
            .effective(&sidebar.key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Unknown
    );
}

#[test]
fn desktop_project_state_is_independent_from_catalog_sidebar() {
    let key = thread("thread-project");
    let mut snapshot = catalog_snapshot(&[]);
    snapshot.global_state_available = true;
    snapshot.legacy_project_assignments_available = true;
    snapshot.project_migrations_by_host.insert(
        "desktop-host".to_string(),
        DesktopProjectMigrationState {
            thread_assignments_migrated: Some(false),
            ..DesktopProjectMigrationState::default()
        },
    );
    snapshot.project_assignments =
        HashMap::from([(key.thread_id.clone(), "legacy-project".to_string())]);
    let project = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &key,
        desktop_host_identity: "desktop-host",
        metadata: &snapshot,
    });
    let catalog = desktop_catalog_observation(key.clone(), &snapshot, None, 10);
    let sidebar = desktop_sidebar_observation(key, 10);

    assert_eq!(project.state, WorkspaceResolutionState::Assigned);
    assert_eq!(catalog.state, SurfaceProjectionState::Absent);
    assert_eq!(sidebar.state, SurfaceProjectionState::Unknown);
    assert_eq!(
        desktop_project_observation(&project, 10).state,
        SurfaceProjectionState::Present
    );
}

#[test]
fn monitor_created_session_can_exist_with_desktop_project_unknown() {
    let key = thread("monitor-created");
    let snapshot = catalog_snapshot(&["monitor-created"]);
    let project = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &key,
        desktop_host_identity: "desktop-host",
        metadata: &snapshot,
    });
    let catalog = desktop_catalog_observation(key, &snapshot, None, 10);

    assert_eq!(catalog.state, SurfaceProjectionState::Present);
    assert_eq!(project.state, WorkspaceResolutionState::Unknown);
}

#[test]
fn cli_canonical_thread_with_incomplete_desktop_observation_stays_unknown() {
    let observation = desktop_inventory_observation(
        thread("cli-thread"),
        SurfaceProjectionKind::Catalog,
        &[],
        ObservationCoverage::Partial,
        10,
    );
    let handling = DesktopProjectionHandling::default();
    handling.observe(observation.clone()).unwrap();

    let effective = handling
        .effective(&observation.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Unknown);
    assert_eq!(effective.coverage, ObservationCoverage::Partial);
}

#[test]
fn restart_restores_pending_stale_reconciliation() {
    let key = thread("thread-restart");
    let document = tombstones(&key);
    let root = std::env::temp_dir().join(format!(
        "codex-monitor-phase-3-4-3-{}",
        uuid::Uuid::new_v4()
    ));
    let path = root.join("deletion-tombstones.json");
    let store = DeletionTombstoneStore::new(path);
    store.save(&document).unwrap();
    let restored = store.load().unwrap();
    let handling = DesktopProjectionHandling::restore(&restored);
    let observation =
        desktop_catalog_observation(key, &catalog_snapshot(&["thread-restart"]), None, 20);
    handling.observe(observation.clone()).unwrap();

    let effective = handling
        .effective(&observation.key, CanonicalThreadProjectionState::Unknown)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Pending
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn desktop_observe_only_capability_never_claims_active_repair() {
    let catalog = desktop_catalog_observation(
        thread("thread-catalog"),
        &catalog_snapshot(&["thread-catalog"]),
        None,
        10,
    );
    let sidebar = desktop_sidebar_observation(thread("thread-sidebar"), 10);

    assert_eq!(
        catalog.action_capability,
        ProjectionActionCapability::ObserveOnly
    );
    assert_eq!(
        sidebar.action_capability,
        ProjectionActionCapability::Unsupported
    );
    assert_ne!(
        catalog.reconciliation_state,
        ProjectionReconciliationState::Reconciled
    );
}

#[test]
fn non_desktop_observation_is_rejected_by_desktop_handler() {
    let observation = monitor_list_observation(
        thread("thread-monitor"),
        &[],
        ObservationCoverage::Complete,
        10,
    );
    let handling = DesktopProjectionHandling::default();

    assert_eq!(
        handling.observe(observation),
        Err(DesktopProjectionHandlingError::NonDesktopObservation)
    );
}

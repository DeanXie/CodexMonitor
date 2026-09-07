use std::collections::HashMap;

use super::global_sources_core::desktop_metadata::{
    DesktopCatalogEntry, DesktopMetadataDiagnostic, DesktopMetadataSnapshot,
    DesktopProjectMigrationState,
};
use super::global_sources_core::desktop_projection::{
    DesktopProjectionAssessment, DesktopProjectionState,
};
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::global_sources_core::source_registry::{CanonicalSourceSnapshot, CanonicalSourceThread};
use super::surface_projection_core::{
    CanonicalThreadProjectionState, ObservationCoverage, ProjectionReconciliationState,
    SurfaceProjectionKind, SurfaceProjectionState, DESKTOP_STALE_ORPHAN_DIAGNOSTIC,
};
use super::surface_projection_engine::{
    cli_exact_id_observation, cli_picker_observation, desktop_catalog_observation,
    desktop_project_observation, desktop_sidebar_observation, global_source_snapshot_observation,
    monitor_exact_read_observation, monitor_list_observation, ExactIdProjectionResult,
    ProjectionObservationEngine,
};
use super::workspace_interop_core::{
    resolve_desktop_project_projection, DesktopProjectProjectionInput,
};

const HOME: &str = "codex-home:fixture";

fn thread(id: &str) -> CodexThreadKey {
    CodexThreadKey::new(HOME, id)
}

fn canonical_thread(id: &str) -> CanonicalSourceThread {
    CanonicalSourceThread {
        key: thread(id),
        parent_thread_key: None,
        agent_path: None,
        current_turn: None,
        lifecycle: None,
        observed_model: None,
        token_snapshot: None,
        producer_surface: Default::default(),
        workspace_assignment: None,
        authority_provenance: None,
        live_lane_count: 0,
        near_live_lane_count: 1,
        historical_lane_count: 0,
    }
}

#[test]
fn exact_monitor_read_success_ingests_present() {
    let engine = ProjectionObservationEngine::default();
    let observation =
        monitor_exact_read_observation(thread("thread-1"), ExactIdProjectionResult::Present, 10);

    assert!(engine.observe(observation.clone()));
    let effective = engine
        .effective(&observation.key, CanonicalThreadProjectionState::Present)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Present);
    assert_eq!(effective.coverage, ObservationCoverage::Complete);
}

#[test]
fn exact_monitor_not_found_with_complete_authority_ingests_absent() {
    let observation = monitor_exact_read_observation(
        thread("thread-1"),
        ExactIdProjectionResult::AuthoritativeNotFound,
        10,
    );

    assert_eq!(observation.state, SurfaceProjectionState::Absent);
    assert_eq!(observation.coverage, ObservationCoverage::Complete);
}

#[test]
fn bounded_monitor_list_miss_is_unknown() {
    let observation = monitor_list_observation(
        thread("thread-missing"),
        &["thread-other".to_string()],
        ObservationCoverage::Bounded,
        10,
    );

    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    assert_eq!(observation.coverage, ObservationCoverage::Bounded);
}

#[test]
fn monitor_list_exact_hit_is_present_even_when_bounded() {
    let observation = monitor_list_observation(
        thread("thread-1"),
        &["thread-1".to_string()],
        ObservationCoverage::Bounded,
        10,
    );

    assert_eq!(observation.state, SurfaceProjectionState::Present);
}

#[test]
fn desktop_catalog_exact_hit_is_present() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: "thread-1".to_string(),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };

    let observation = desktop_catalog_observation(thread("thread-1"), &snapshot, None, 10);
    assert_eq!(observation.state, SurfaceProjectionState::Present);
}

#[test]
fn desktop_bounded_list_miss_is_unknown() {
    let observation = super::surface_projection_engine::desktop_inventory_observation(
        thread("thread-1"),
        SurfaceProjectionKind::HistoryList,
        &[],
        ObservationCoverage::Bounded,
        10,
    );

    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
}

#[test]
fn desktop_complete_inventory_miss_is_absent() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        ..DesktopMetadataSnapshot::default()
    };

    let catalog = desktop_catalog_observation(thread("thread-1"), &snapshot, None, 10);
    let sidebar = desktop_sidebar_observation(thread("thread-1"), 10);
    let project = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &thread("thread-1"),
        desktop_host_identity: "desktop-host",
        metadata: &snapshot,
    });
    let project_observation = desktop_project_observation(&project, 10);

    assert_eq!(catalog.state, SurfaceProjectionState::Absent);
    assert_eq!(catalog.coverage, ObservationCoverage::Complete);
    assert_eq!(sidebar.state, SurfaceProjectionState::Unknown);
    assert_eq!(sidebar.coverage, ObservationCoverage::NotObserved);
    assert_eq!(project_observation.state, SurfaceProjectionState::Unknown);
}

#[test]
fn desktop_project_assignment_uses_only_project_projection_evidence() {
    let mut snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        global_state_available: true,
        legacy_project_assignments_available: true,
        project_migrations_by_host: HashMap::from([(
            "desktop-host".to_string(),
            DesktopProjectMigrationState {
                thread_assignments_migrated: Some(false),
                ..DesktopProjectMigrationState::default()
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    };
    snapshot
        .project_assignments
        .insert("thread-1".to_string(), "legacy-project".to_string());
    let projection = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &thread("thread-1"),
        desktop_host_identity: "desktop-host",
        metadata: &snapshot,
    });

    let observation = desktop_project_observation(&projection, 10);
    assert_eq!(observation.state, SurfaceProjectionState::Present);
    assert_eq!(
        observation.key.projection_kind,
        SurfaceProjectionKind::Project
    );
}

#[test]
fn conflicting_desktop_project_candidates_still_observe_exact_thread_present() {
    let mut snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        legacy_project_assignments_available: true,
        persisted_state_available: true,
        persisted_project_id_available: true,
        project_migrations_by_host: HashMap::from([(
            "desktop-host".to_string(),
            DesktopProjectMigrationState {
                thread_assignments_migrated: Some(true),
                ..DesktopProjectMigrationState::default()
            },
        )]),
        ..DesktopMetadataSnapshot::default()
    };
    snapshot
        .project_assignments
        .insert("thread-1".to_string(), "legacy-project".to_string());
    snapshot.persisted_threads.insert(
        "thread-1".to_string(),
        super::global_sources_core::desktop_metadata::DesktopPersistedThread {
            thread_id: "thread-1".to_string(),
            project_id: Some("app-server-project".to_string()),
            ..Default::default()
        },
    );
    let projection = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &thread("thread-1"),
        desktop_host_identity: "desktop-host",
        metadata: &snapshot,
    });

    let observation = desktop_project_observation(&projection, 10);
    assert_eq!(projection.candidate_projects.len(), 2);
    assert_eq!(observation.state, SurfaceProjectionState::Present);
}

#[test]
fn cli_exact_id_discovery_is_separate_projection_from_picker() {
    let exact = cli_exact_id_observation(thread("thread-1"), ExactIdProjectionResult::Present, 10);
    let picker = cli_picker_observation(
        thread("thread-1"),
        &[],
        ObservationCoverage::NotObserved,
        10,
    );

    assert_eq!(
        exact.key.projection_kind,
        SurfaceProjectionKind::Discoverability
    );
    assert_eq!(exact.state, SurfaceProjectionState::Present);
    assert_eq!(
        picker.key.projection_kind,
        SurfaceProjectionKind::HistoryList
    );
    assert_eq!(picker.state, SurfaceProjectionState::Unknown);
}

#[test]
fn cli_picker_miss_without_complete_coverage_is_unknown() {
    for coverage in [
        ObservationCoverage::Bounded,
        ObservationCoverage::Partial,
        ObservationCoverage::Failed,
        ObservationCoverage::NotObserved,
    ] {
        let observation = cli_picker_observation(thread("thread-1"), &[], coverage, 10);
        assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    }
}

#[test]
fn global_source_snapshot_is_projection_input_not_canonical_authority() {
    let snapshot = CanonicalSourceSnapshot {
        threads: vec![canonical_thread("thread-1")],
    };
    let observation = global_source_snapshot_observation(thread("thread-1"), &snapshot, 10);
    let engine = ProjectionObservationEngine::default();
    engine.observe(observation.clone());

    let effective = engine
        .effective(&observation.key, CanonicalThreadProjectionState::Unknown)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Present);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Unknown
    );
}

#[test]
fn global_source_snapshot_requires_exact_codex_thread_key() {
    let snapshot = CanonicalSourceSnapshot {
        threads: vec![CanonicalSourceThread {
            key: CodexThreadKey::new("other-home", "thread-1"),
            ..canonical_thread("placeholder")
        }],
    };

    let observation = global_source_snapshot_observation(thread("thread-1"), &snapshot, 10);
    assert_eq!(observation.state, SurfaceProjectionState::Absent);
}

#[test]
fn desktop_snapshot_from_other_home_cannot_confirm_catalog_membership() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: "other-home".to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: "thread-1".to_string(),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };

    let observation = desktop_catalog_observation(thread("thread-1"), &snapshot, None, 10);
    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    assert_eq!(observation.coverage, ObservationCoverage::Failed);
}

#[test]
fn failed_exact_monitor_read_is_unknown_not_absent() {
    let observation = monitor_exact_read_observation(
        thread("thread-1"),
        ExactIdProjectionResult::Failed("transport disconnected".to_string()),
        10,
    );

    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    assert_eq!(observation.coverage, ObservationCoverage::Failed);
}

#[test]
fn tombstone_plus_desktop_present_becomes_stale_pending() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: "thread-1".to_string(),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };
    let assessment = DesktopProjectionAssessment {
        state: DesktopProjectionState::DesktopStaleOrphan,
        canonical_ingest_allowed: false,
        evidence: vec!["phase-2.5-stale-orphan".to_string()],
    };
    let observation =
        desktop_catalog_observation(thread("thread-1"), &snapshot, Some(&assessment), 10);
    let engine = ProjectionObservationEngine::default();
    engine.observe(observation.clone());

    let effective = engine
        .effective(&observation.key, CanonicalThreadProjectionState::Tombstoned)
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
fn later_complete_desktop_absence_becomes_reconciled() {
    let present_snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: "thread-1".to_string(),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };
    let absent_snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        ..DesktopMetadataSnapshot::default()
    };
    let engine = ProjectionObservationEngine::default();
    let present = desktop_catalog_observation(thread("thread-1"), &present_snapshot, None, 10);
    let key = present.key.clone();
    engine.observe(present);
    engine.observe(desktop_catalog_observation(
        thread("thread-1"),
        &absent_snapshot,
        None,
        20,
    ));

    let effective = engine
        .effective(&key, CanonicalThreadProjectionState::Tombstoned)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Reconciled
    );
}

#[test]
fn desktop_stale_orphan_diagnostic_is_preserved() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: "thread-1".to_string(),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };
    let assessment = DesktopProjectionAssessment {
        state: DesktopProjectionState::DesktopStaleOrphan,
        canonical_ingest_allowed: false,
        evidence: vec!["existing-phase-2.5-assessment".to_string()],
    };

    let observation =
        desktop_catalog_observation(thread("thread-1"), &snapshot, Some(&assessment), 10);
    assert!(observation
        .diagnostics
        .contains(&DESKTOP_STALE_ORPHAN_DIAGNOSTIC.to_string()));
}

#[test]
fn projection_ingestion_cannot_revive_tombstoned_thread() {
    let engine = ProjectionObservationEngine::default();
    let observation =
        cli_exact_id_observation(thread("thread-1"), ExactIdProjectionResult::Present, 10);
    engine.observe(observation.clone());

    let effective = engine
        .effective(&observation.key, CanonicalThreadProjectionState::Tombstoned)
        .unwrap();
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Pending
    );
}

#[test]
fn repeated_observation_is_idempotent() {
    let engine = ProjectionObservationEngine::default();
    let observation =
        monitor_exact_read_observation(thread("thread-1"), ExactIdProjectionResult::Present, 10);

    assert!(engine.observe(observation.clone()));
    assert!(!engine.observe(observation.clone()));
    assert_eq!(engine.history(&observation.key).len(), 1);
}

#[test]
fn observation_order_does_not_change_effective_result() {
    let older = cli_picker_observation(
        thread("thread-1"),
        &["thread-1".to_string()],
        ObservationCoverage::Bounded,
        10,
    );
    let newer = cli_picker_observation(thread("thread-1"), &[], ObservationCoverage::Partial, 20);
    let first = ProjectionObservationEngine::default();
    first.observe(older.clone());
    first.observe(newer.clone());
    let second = ProjectionObservationEngine::default();
    second.observe(newer.clone());
    second.observe(older.clone());

    assert_eq!(
        first.effective(&older.key, CanonicalThreadProjectionState::Present),
        second.effective(&older.key, CanonicalThreadProjectionState::Present)
    );
}

#[test]
fn schema_drift_produces_unknown_not_absent() {
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        diagnostics: vec![DesktopMetadataDiagnostic {
            source: "state_5.sqlite".to_string(),
            code: "private-schema-drift".to_string(),
            message: "threads.id missing".to_string(),
        }],
        ..DesktopMetadataSnapshot::default()
    };

    let observation = desktop_catalog_observation(thread("thread-1"), &snapshot, None, 10);
    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    assert_eq!(observation.coverage, ObservationCoverage::Failed);
    assert!(!observation.diagnostics.is_empty());
}

#[test]
fn projection_ingestion_does_not_change_workspace_project_or_thread_identity() {
    let original = thread("thread-1");
    let snapshot = DesktopMetadataSnapshot {
        codex_home_identity: HOME.to_string(),
        catalog_available: true,
        catalog_entries: vec![DesktopCatalogEntry {
            thread_id: original.thread_id.clone(),
            cwd: Some("F:\\somewhere-else".to_string()),
            project_id: Some("project-other".to_string()),
            ..DesktopCatalogEntry::default()
        }],
        ..DesktopMetadataSnapshot::default()
    };

    let observation = desktop_catalog_observation(original.clone(), &snapshot, None, 10);
    assert_eq!(observation.key.thread_key, original);
    assert_eq!(observation.state, SurfaceProjectionState::Present);
}

use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::surface_projection_core::{
    CanonicalThreadProjectionState, ObservationCoverage, ProjectionActionCapability,
    ProjectionMembershipExpectation, ProjectionReconciliationState, SurfaceProjectionKey,
    SurfaceProjectionKind, SurfaceProjectionObservation, SurfaceProjectionState,
    SurfaceProjectionStore, SurfaceProjectionSurface, DESKTOP_STALE_ORPHAN_DIAGNOSTIC,
    MISSING_PROJECTION_DIAGNOSTIC,
};

fn key(
    thread_id: &str,
    surface: SurfaceProjectionSurface,
    kind: SurfaceProjectionKind,
) -> SurfaceProjectionKey {
    SurfaceProjectionKey::new(CodexThreadKey::new("home-a", thread_id), surface, kind)
}

fn membership(
    key: SurfaceProjectionKey,
    exact_thread_id_present: bool,
    coverage: ObservationCoverage,
    observed_at: u64,
    source: &str,
) -> SurfaceProjectionObservation {
    SurfaceProjectionObservation::membership(
        key,
        exact_thread_id_present,
        coverage,
        observed_at,
        vec![source.to_string()],
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
    )
}

#[test]
fn canonical_present_and_complete_surface_miss_is_absent() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Catalog,
    );
    let observation = membership(
        key.clone(),
        false,
        ObservationCoverage::Complete,
        1,
        "catalog",
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(observation);

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Present)
        .expect("effective projection");
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::NotRequired
    );
}

#[test]
fn bounded_or_partial_surface_miss_is_unknown() {
    for coverage in [ObservationCoverage::Bounded, ObservationCoverage::Partial] {
        let key = key(
            "thread-a",
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Sidebar,
        );
        let observation = membership(key, false, coverage, 1, "recent-list");
        assert_eq!(observation.state, SurfaceProjectionState::Unknown);
    }
}

#[test]
fn failed_surface_read_is_unknown() {
    let observation = membership(
        key(
            "thread-a",
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Catalog,
        ),
        false,
        ObservationCoverage::Failed,
        1,
        "read-error",
    );
    assert_eq!(observation.state, SurfaceProjectionState::Unknown);
}

#[test]
fn tombstone_plus_surface_present_is_stale_and_pending() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Catalog,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Partial,
        1,
        "catalog",
    ));

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Tombstoned)
        .expect("effective projection");
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Pending
    );
}

#[test]
fn stale_projection_cannot_revive_deleted_thread() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Sidebar,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        2,
        "sidebar",
    ));

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Tombstoned)
        .expect("effective projection");
    assert_eq!(
        effective.key.thread_key,
        CodexThreadKey::new("home-a", "thread-a")
    );
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
}

#[test]
fn later_complete_absence_marks_reconciled() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Catalog,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        "catalog-present",
    ));
    store.observe(membership(
        key.clone(),
        false,
        ObservationCoverage::Complete,
        2,
        "catalog-absent",
    ));

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Tombstoned)
        .expect("effective projection");
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Reconciled
    );
}

#[test]
fn optional_absent_projection_does_not_imply_missing_projection() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Sidebar,
    );
    let observation = SurfaceProjectionObservation::membership(
        key.clone(),
        false,
        ObservationCoverage::Complete,
        1,
        vec!["sidebar".to_string()],
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Optional,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(observation);

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Present)
        .expect("effective projection");
    assert_eq!(effective.state, SurfaceProjectionState::Absent);
    assert!(!effective
        .diagnostics
        .iter()
        .any(|value| value == MISSING_PROJECTION_DIAGNOSTIC));
}

#[test]
fn required_absent_projection_adds_missing_projection_diagnostic() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Sidebar,
    );
    let observation = SurfaceProjectionObservation::membership(
        key.clone(),
        false,
        ObservationCoverage::Complete,
        1,
        vec!["sidebar-contract".to_string()],
        ProjectionActionCapability::ObserveOnly,
        ProjectionMembershipExpectation::Required,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(observation);

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Present)
        .expect("effective projection");
    assert!(effective
        .diagnostics
        .iter()
        .any(|value| value == MISSING_PROJECTION_DIAGNOSTIC));
}

#[test]
fn desktop_stale_orphan_remains_compatible() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Catalog,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        "catalog",
    ));

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Absent)
        .expect("effective projection");
    assert_eq!(effective.state, SurfaceProjectionState::Stale);
    assert!(effective
        .diagnostics
        .iter()
        .any(|value| value == DESKTOP_STALE_ORPHAN_DIAGNOSTIC));
}

#[test]
fn repeated_projection_evidence_is_idempotent() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Monitor,
        SurfaceProjectionKind::SessionList,
    );
    let observation = membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        "thread-list",
    );
    let mut store = SurfaceProjectionStore::default();
    assert!(store.observe(observation.clone()));
    assert!(!store.observe(observation));
    assert_eq!(store.history(&key).len(), 1);
}

#[test]
fn distinct_projection_provenance_is_preserved() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Monitor,
        SurfaceProjectionKind::SessionList,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        "thread-list-a",
    ));
    store.observe(membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        2,
        "thread-list-b",
    ));
    assert_eq!(store.history(&key).len(), 2);
}

#[test]
fn evidence_order_does_not_change_effective_state() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Catalog,
    );
    let present = membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        10,
        "present",
    );
    let absent = membership(
        key.clone(),
        false,
        ObservationCoverage::Complete,
        20,
        "absent",
    );
    let mut first = SurfaceProjectionStore::default();
    first.observe(present.clone());
    first.observe(absent.clone());
    let mut second = SurfaceProjectionStore::default();
    second.observe(absent);
    second.observe(present);

    assert_eq!(
        first.effective(&key, CanonicalThreadProjectionState::Tombstoned),
        second.effective(&key, CanonicalThreadProjectionState::Tombstoned)
    );
}

#[test]
fn exact_thread_keys_remain_isolated() {
    let first_key = key(
        "thread-a",
        SurfaceProjectionSurface::Cli,
        SurfaceProjectionKind::Discoverability,
    );
    let second_key = key(
        "thread-b",
        SurfaceProjectionSurface::Cli,
        SurfaceProjectionKind::Discoverability,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(membership(
        first_key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        "cli",
    ));
    store.observe(membership(
        second_key.clone(),
        false,
        ObservationCoverage::Complete,
        1,
        "cli",
    ));

    assert_eq!(
        store
            .effective(&first_key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Present
    );
    assert_eq!(
        store
            .effective(&second_key, CanonicalThreadProjectionState::Present)
            .unwrap()
            .state,
        SurfaceProjectionState::Absent
    );
}

#[test]
fn projection_state_does_not_change_thread_identity() {
    let thread_key = CodexThreadKey::new("home-a", "thread-a");
    let key = SurfaceProjectionKey::new(
        thread_key.clone(),
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Sidebar,
    );
    let observation = membership(key, true, ObservationCoverage::Complete, 1, "sidebar");
    assert_eq!(observation.key.thread_key, thread_key);
}

#[test]
fn projection_state_does_not_change_workspace_or_project_identity() {
    let workspace_key = "environment-a::root-a".to_string();
    let project_id = "desktop-project-a".to_string();
    let observation = membership(
        key(
            "thread-a",
            SurfaceProjectionSurface::Desktop,
            SurfaceProjectionKind::Sidebar,
        ),
        false,
        ObservationCoverage::Complete,
        1,
        "sidebar",
    );
    assert_eq!(observation.state, SurfaceProjectionState::Absent);
    assert_eq!(workspace_key, "environment-a::root-a");
    assert_eq!(project_id, "desktop-project-a");
}

#[test]
fn not_applicable_is_explicit() {
    let observation = SurfaceProjectionObservation::not_applicable(
        key(
            "thread-a",
            SurfaceProjectionSurface::Cli,
            SurfaceProjectionKind::Project,
        ),
        1,
        vec!["cli-has-no-project-projection".to_string()],
    );
    assert_eq!(observation.state, SurfaceProjectionState::NotApplicable);
    assert_eq!(observation.coverage, ObservationCoverage::NotApplicable);
}

#[test]
fn unsupported_action_capability_never_claims_reconciliation_success() {
    let key = key(
        "thread-a",
        SurfaceProjectionSurface::Desktop,
        SurfaceProjectionKind::Sidebar,
    );
    let observation = SurfaceProjectionObservation::membership(
        key.clone(),
        true,
        ObservationCoverage::Complete,
        1,
        vec!["sidebar".to_string()],
        ProjectionActionCapability::Unsupported,
        ProjectionMembershipExpectation::Optional,
    );
    let mut store = SurfaceProjectionStore::default();
    store.observe(observation);

    let effective = store
        .effective(&key, CanonicalThreadProjectionState::Tombstoned)
        .expect("effective projection");
    assert_eq!(
        effective.action_capability,
        ProjectionActionCapability::Unsupported
    );
    assert_eq!(
        effective.reconciliation_state,
        ProjectionReconciliationState::Pending
    );
}

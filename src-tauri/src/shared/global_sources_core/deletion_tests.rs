use super::deletion_tombstone::{
    DeletionReconciliationState, DeletionTombstone, DeletionTombstoneDocument,
    DeletionTombstoneStore, DesktopReconciliationState,
};
use super::rollout_identity::CodexThreadKey;
use super::source_envelope::{FreshnessEvidence, FreshnessState, SourceKind, SourceTemporalClass};
use super::source_registry::{ExternalLifecycle, SourceAuthorityRegistry, SourceLaneUpdate};
use uuid::Uuid;

fn lane(
    key: CodexThreadKey,
    temporal_class: SourceTemporalClass,
    observation: &str,
) -> SourceLaneUpdate {
    SourceLaneUpdate {
        observation_id: observation.to_string(),
        thread_key: key,
        turn_key: None,
        source_kind: match temporal_class {
            SourceTemporalClass::Live => SourceKind::MonitorAppServer,
            SourceTemporalClass::NearLive => SourceKind::CodexCliRollout,
            SourceTemporalClass::Historical => SourceKind::HistoricalRolloutScan,
        },
        temporal_class,
        source_instance_id: "fixture-source".to_string(),
        source_generation: "fixture-generation".to_string(),
        source_timestamp_ms: Some(1_000),
        observed_timestamp_ms: 1_001,
        freshness: FreshnessEvidence {
            state: FreshnessState::Fresh,
            last_complete_record_observed_at_ms: Some(1_001),
            reason: "fixture".to_string(),
        },
        lifecycle: Some(ExternalLifecycle::Running),
        observed_model: None,
        token_snapshot: None,
    }
}

#[test]
fn tombstone_document_round_trips_spec_contract_and_exact_desktop_absence() {
    let root = CodexThreadKey::new("home-a", "thread-root");
    let child = CodexThreadKey::new("home-a", "thread-child");
    let mut tombstone = DeletionTombstone::confirmed(
        "7a762c32-1fd2-43f1-b4da-72f462a9714f",
        root.clone(),
        vec![child.clone()],
        1_000,
    );
    assert_eq!(tombstone.upstream_request_id, None);
    assert_eq!(
        tombstone.reconciliation_state,
        DeletionReconciliationState::Pending
    );
    assert_eq!(
        tombstone.desktop_reconciliation,
        DesktopReconciliationState::Unknown
    );

    tombstone.mark_local_reconciliation_completed();
    assert_eq!(
        tombstone.desktop_reconciliation,
        DesktopReconciliationState::RefreshPending
    );
    assert!(!tombstone.record_desktop_absence(["same-title-new-id"]));
    assert!(tombstone.record_desktop_absence(["thread-root", "thread-child"]));
    assert_eq!(
        tombstone.desktop_reconciliation,
        DesktopReconciliationState::Reconciled
    );

    let directory = std::env::temp_dir().join(format!("codex-monitor-deletion-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("create tempdir");
    let store = DeletionTombstoneStore::new(directory.join("deletion-tombstones.json"));
    let document = DeletionTombstoneDocument {
        version: 1,
        operations: vec![tombstone],
    };
    store.save(&document).expect("save tombstones");
    assert_eq!(store.load().expect("reload tombstones"), document);
    std::fs::remove_dir_all(directory).expect("remove tempdir");
}

#[test]
fn registry_retirement_rejects_all_temporal_lanes_and_preserves_other_identity() {
    let retired = CodexThreadKey::new("home-a", "thread-deleted");
    let same_title_new_identity = CodexThreadKey::new("home-a", "thread-new");
    let mut registry = SourceAuthorityRegistry::default();
    assert!(registry
        .ingest(lane(retired.clone(), SourceTemporalClass::Live, "before"))
        .unwrap());

    assert_eq!(registry.retire_threads([retired.clone()]), 1);
    assert!(registry.is_tombstoned(&retired));
    assert!(registry.lanes(&retired).is_none());
    for (temporal_class, observation) in [
        (SourceTemporalClass::Live, "live-replay"),
        (SourceTemporalClass::NearLive, "near-live-replay"),
        (SourceTemporalClass::Historical, "historical-replay"),
    ] {
        assert!(!registry
            .ingest(lane(retired.clone(), temporal_class, observation))
            .unwrap());
    }
    assert!(registry
        .ingest(lane(
            same_title_new_identity.clone(),
            SourceTemporalClass::Historical,
            "new-id"
        ))
        .unwrap());
    assert!(registry.lanes(&same_title_new_identity).is_some());
    assert_eq!(registry.retire_threads([retired]), 0);
}

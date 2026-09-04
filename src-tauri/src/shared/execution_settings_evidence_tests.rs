use super::codex_core::creation_coordination::{CreationCoordinator, IntentId};
use super::execution_settings_evidence::*;
use super::global_sources_core::rollout_identity::CodexThreadKey;

fn thread(id: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home", id)
}

fn record(
    layer: ExecutionSettingsEvidenceLayer,
    value: &str,
    source: &str,
    comparison_id: &str,
    observed_at: u64,
) -> ExecutionSettingsEvidenceRecord<ExecutionSettingValue> {
    ExecutionSettingsEvidenceRecord::new(
        layer,
        ExecutionSettingValue::Text(value.to_string()),
        ExecutionSettingsProvenance::confirmed(source, comparison_id, observed_at),
    )
}

fn requested(value: &str) -> ExecutionSettingsEvidenceRecord<ExecutionSettingValue> {
    record(
        ExecutionSettingsEvidenceLayer::Requested,
        value,
        "monitor-request",
        "operation-1",
        10,
    )
}

fn effective(value: &str) -> ExecutionSettingsEvidenceRecord<ExecutionSettingValue> {
    record(
        ExecutionSettingsEvidenceLayer::ServerEffective,
        value,
        "app-server-response",
        "operation-1",
        20,
    )
}

fn observed(value: &str) -> ExecutionSettingsEvidenceRecord<ExecutionSettingValue> {
    record(
        ExecutionSettingsEvidenceLayer::PersistedObserved,
        value,
        "turn-context",
        "operation-1",
        30,
    )
}

fn select(
    records: impl IntoIterator<Item = ExecutionSettingsEvidenceRecord<ExecutionSettingValue>>,
) -> SettingEvidence<ExecutionSettingValue> {
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    for evidence in records {
        store.observe(key.clone(), ExecutionSettingField::Model, evidence);
    }
    store.select(&key, ExecutionSettingField::Model)
}

#[test]
fn no_reliable_evidence_is_unknown() {
    assert_eq!(select([]).assessment, ExecutionSettingsAssessment::Unknown);
}

#[test]
fn requested_does_not_imply_effective() {
    let selected = select([requested("gpt-requested")]);
    assert_eq!(
        selected.assessment,
        ExecutionSettingsAssessment::RequestedOnly
    );
    assert_eq!(selected.requested.len(), 1);
    assert!(selected.server_effective.is_empty());
    assert!(selected.persisted_observed.is_empty());
}

#[test]
fn effective_does_not_imply_persisted_observation() {
    let selected = select([effective("gpt-effective")]);
    assert_eq!(
        selected.assessment,
        ExecutionSettingsAssessment::EffectiveConfirmed
    );
    assert!(selected.requested.is_empty());
    assert_eq!(selected.server_effective.len(), 1);
    assert!(selected.persisted_observed.is_empty());
}

#[test]
fn observed_only_is_observed_confirmed() {
    let selected = select([observed("gpt-observed")]);
    assert_eq!(
        selected.assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
}

#[test]
fn requested_only_is_requested_only() {
    assert_eq!(
        select([requested("medium")]).assessment,
        ExecutionSettingsAssessment::RequestedOnly
    );
}

#[test]
fn requested_and_equal_observed_is_match() {
    assert_eq!(
        select([requested("gpt-a"), observed("gpt-a")]).assessment,
        ExecutionSettingsAssessment::Match
    );
}

#[test]
fn requested_and_different_observed_is_mismatch() {
    assert_eq!(
        select([requested("gpt-a"), observed("gpt-b")]).assessment,
        ExecutionSettingsAssessment::Mismatch
    );
}

#[test]
fn effective_and_different_observed_is_conflict() {
    assert_eq!(
        select([effective("gpt-a"), observed("gpt-b")]).assessment,
        ExecutionSettingsAssessment::Conflict
    );
}

#[test]
fn overridden_is_reason_not_assessment_state() {
    let mut overridden = observed("gpt-b");
    overridden.provenance.reason = Some(ExecutionSettingsEvidenceReason::Overridden);
    let selected = select([requested("gpt-a"), overridden]);
    assert_eq!(selected.assessment, ExecutionSettingsAssessment::Mismatch);
    assert!(selected
        .provenance
        .iter()
        .any(|item| item.reason == Some(ExecutionSettingsEvidenceReason::Overridden)));
}

#[test]
fn thread_default_and_turn_execution_are_independent() {
    let thread_key = thread("thread-1");
    let default_key = ExecutionSettingsObservationKey::thread_default(thread_key.clone());
    let turn_key = ExecutionSettingsObservationKey::turn(thread_key, "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(
        default_key.clone(),
        ExecutionSettingField::Model,
        requested("gpt-a"),
    );
    store.observe(
        turn_key.clone(),
        ExecutionSettingField::Model,
        observed("gpt-b"),
    );

    assert_eq!(
        store
            .select(&default_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::RequestedOnly
    );
    assert_eq!(
        store
            .select(&turn_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
}

#[test]
fn different_turns_do_not_overwrite_each_other() {
    let thread_key = thread("thread-1");
    let first = ExecutionSettingsObservationKey::turn(thread_key.clone(), "turn-1");
    let second = ExecutionSettingsObservationKey::turn(thread_key, "turn-2");
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(
        first.clone(),
        ExecutionSettingField::Model,
        observed("gpt-a"),
    );
    store.observe(
        second.clone(),
        ExecutionSettingField::Model,
        observed("gpt-b"),
    );

    assert_eq!(
        store
            .select(&first, ExecutionSettingField::Model)
            .canonical_observed_value(),
        Some(&ExecutionSettingValue::Text("gpt-a".to_string()))
    );
    assert_eq!(
        store
            .select(&second, ExecutionSettingField::Model)
            .canonical_observed_value(),
        Some(&ExecutionSettingValue::Text("gpt-b".to_string()))
    );
}

#[test]
fn later_thread_snapshot_does_not_conflict_with_old_turn_observation() {
    let thread_key = thread("thread-1");
    let turn_key = ExecutionSettingsObservationKey::turn(thread_key.clone(), "turn-1");
    let default_key = ExecutionSettingsObservationKey::thread_default(thread_key);
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(
        turn_key.clone(),
        ExecutionSettingField::Model,
        observed("gpt-a"),
    );
    store.observe(
        default_key.clone(),
        ExecutionSettingField::Model,
        record(
            ExecutionSettingsEvidenceLayer::ServerEffective,
            "gpt-b",
            "later-settings-update",
            "settings-revision-2",
            100,
        ),
    );

    assert_eq!(
        store
            .select(&turn_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
    assert_eq!(
        store
            .select(&default_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::EffectiveConfirmed
    );
}

#[test]
fn repeated_identical_evidence_is_idempotent() {
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    let evidence = observed("gpt-a");
    let mut store = ExecutionSettingsEvidenceStore::default();
    assert!(store.observe(key.clone(), ExecutionSettingField::Model, evidence.clone()));
    assert!(!store.observe(key.clone(), ExecutionSettingField::Model, evidence));
    assert_eq!(store.history(&key, ExecutionSettingField::Model).len(), 1);
}

#[test]
fn identical_value_from_distinct_provenance_preserves_raw_evidence() {
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(key.clone(), ExecutionSettingField::Model, observed("gpt-a"));
    store.observe(
        key.clone(),
        ExecutionSettingField::Model,
        record(
            ExecutionSettingsEvidenceLayer::PersistedObserved,
            "gpt-a",
            "session-reconstruction",
            "operation-1",
            40,
        ),
    );

    assert_eq!(store.history(&key, ExecutionSettingField::Model).len(), 2);
    let selected = store.select(&key, ExecutionSettingField::Model);
    assert_eq!(
        selected.canonical_observed_value(),
        Some(&ExecutionSettingValue::Text("gpt-a".to_string()))
    );
    assert_eq!(selected.provenance.len(), 2);
}

#[test]
fn evidence_order_does_not_change_effective_assessment() {
    let forward = select([requested("gpt-a"), effective("gpt-a"), observed("gpt-a")]);
    let reverse = select([observed("gpt-a"), effective("gpt-a"), requested("gpt-a")]);
    assert_eq!(forward, reverse);
    assert_eq!(forward.assessment, ExecutionSettingsAssessment::Match);
}

#[test]
fn distinct_comparison_groups_are_not_compared() {
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(
        key.clone(),
        ExecutionSettingField::Model,
        requested("gpt-a"),
    );
    store.observe(
        key.clone(),
        ExecutionSettingField::Model,
        record(
            ExecutionSettingsEvidenceLayer::PersistedObserved,
            "gpt-b",
            "unrelated-observation",
            "operation-2",
            40,
        ),
    );

    let selected = store.select(&key, ExecutionSettingField::Model);
    assert_eq!(
        selected.assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
    assert!(selected.requested.is_empty());
    assert_eq!(store.history(&key, ExecutionSettingField::Model).len(), 2);
}

#[test]
fn provenance_is_preserved_across_layers() {
    let selected = select([requested("gpt-a"), effective("gpt-a"), observed("gpt-a")]);
    let sources = selected
        .provenance
        .iter()
        .map(|item| item.source.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        sources,
        vec!["monitor-request", "app-server-response", "turn-context"]
    );
    assert_eq!(selected.observed_at, Some(30));
}

#[test]
fn settings_evidence_does_not_change_creation_coordination() {
    let coordinator = CreationCoordinator::default();
    let context = coordinator.context();
    let intent = IntentId {
        process_epoch: context["processEpoch"].as_str().unwrap().to_string(),
        id: "00000000-0000-4000-8000-000000000001".to_string(),
    };
    let before = coordinator.creation_status(&intent).unwrap();
    let _ = select([requested("gpt-a"), observed("gpt-b")]);
    let after = coordinator.creation_status(&intent).unwrap();
    assert_eq!(before, after);
    assert_eq!(after["state"], "INTENT_CREATED");
}

#[test]
fn settings_evidence_does_not_enter_thread_identity() {
    let thread_key = thread("thread-1");
    let key = ExecutionSettingsObservationKey::turn(thread_key.clone(), "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    for (field, value) in [
        (
            ExecutionSettingField::Model,
            ExecutionSettingValue::Text("gpt-a".into()),
        ),
        (
            ExecutionSettingField::Effort,
            ExecutionSettingValue::Text("high".into()),
        ),
        (
            ExecutionSettingField::ApprovalPolicy,
            ExecutionSettingValue::Text("never".into()),
        ),
        (
            ExecutionSettingField::SandboxPolicy,
            ExecutionSettingValue::Text("workspace-write".into()),
        ),
        (
            ExecutionSettingField::NetworkAccess,
            ExecutionSettingValue::Bool(true),
        ),
        (
            ExecutionSettingField::WritableRoots,
            ExecutionSettingValue::StringList(vec!["f:/work".into()]),
        ),
        (
            ExecutionSettingField::Cwd,
            ExecutionSettingValue::Text("f:/work".into()),
        ),
        (
            ExecutionSettingField::CollaborationMode,
            ExecutionSettingValue::Text("default".into()),
        ),
    ] {
        store.observe(
            key.clone(),
            field,
            ExecutionSettingsEvidenceRecord::new(
                ExecutionSettingsEvidenceLayer::Requested,
                value,
                ExecutionSettingsProvenance::confirmed("request", "operation-1", 10),
            ),
        );
    }
    assert_eq!(key.thread_key, thread_key);
}

#[test]
fn observation_groups_fields_without_merging_their_evidence() {
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    let mut store = ExecutionSettingsEvidenceStore::default();
    store.observe(
        key.clone(),
        ExecutionSettingField::Model,
        requested("gpt-a"),
    );
    store.observe(
        key.clone(),
        ExecutionSettingField::NetworkAccess,
        ExecutionSettingsEvidenceRecord::new(
            ExecutionSettingsEvidenceLayer::PersistedObserved,
            ExecutionSettingValue::Bool(true),
            ExecutionSettingsProvenance::confirmed("turn-context", "operation-1", 30),
        ),
    );

    let observation = store.observation(&key);
    assert_eq!(observation.key, key);
    assert_eq!(observation.fields.len(), 2);
    assert_eq!(
        observation.fields[&ExecutionSettingField::Model].assessment,
        ExecutionSettingsAssessment::RequestedOnly
    );
    assert_eq!(
        observation.fields[&ExecutionSettingField::NetworkAccess].assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
}

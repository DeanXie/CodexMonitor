use super::delete_mutation_observation::{
    DeleteMutationFailureKind, DeleteMutationObservationRuntime, DeleteMutationState,
};
use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::RemoteHostIdentity;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/fixtures/app-server/delete-mutation-observation")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read delete fixture"))
        .expect("parse delete fixture")
}

fn isolation_fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/fixtures/app-server/delete-mutation-isolation")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read isolation fixture"))
        .expect("parse isolation fixture")
}

fn runtime() -> DeleteMutationObservationRuntime {
    DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("fixture-workspace-generation").unwrap(),
        AppServerConnectionGeneration::new("fixture-app-server-generation").unwrap(),
    )
}

fn host() -> RemoteHostIdentity {
    RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap()
}

fn begin(
    runtime: &DeleteMutationObservationRuntime,
) -> super::delete_mutation_observation::DeleteAttemptId {
    runtime
        .begin_delete(
            host(),
            CodexThreadKey::new("codex-home-fixture", "0199a8c0-1111-7222-8333-444455556666"),
            "0199a8c0-1111-7222-8333-444455556666",
            "fixture-workspace-generation",
            "fixture-app-server-generation",
            10,
        )
        .unwrap()
}

#[test]
fn exact_delete_request_fixture_uses_only_full_thread_id() {
    assert_eq!(
        fixture("request-exact.json"),
        serde_json::json!({
            "method": "thread/delete",
            "params": {"threadId": "0199a8c0-1111-7222-8333-444455556666"}
        })
    );
}

#[test]
fn empty_success_fixture_confirms_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(&attempt, &fixture("response-success.json"), 12)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn active_writer_fixture_records_typed_rejection() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(&attempt, &fixture("response-active-writer.json"), 12)
        .unwrap();
    let observation = runtime.snapshot(&attempt).unwrap();
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(
        observation.failure_kind,
        Some(DeleteMutationFailureKind::BlockedByActiveWriter)
    );
}

#[test]
fn exact_thread_deleted_fixture_confirms_current_attempt() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    let notification = fixture("notification-thread-deleted.json");
    runtime
        .record_current_thread_deleted(
            notification
                .pointer("/params/threadId")
                .unwrap()
                .as_str()
                .unwrap(),
            12,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn response_loss_fixture_records_unknown_without_tombstone() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    assert_eq!(fixture("response-loss.json")["responseObserved"], false);
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteOutcomeUnknown
    );
    assert!(runtime.confirmed_tombstone(&attempt).is_none());
}

#[test]
fn session_end_fixture_preserves_unresolved_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    assert_eq!(
        fixture("session-ended-unresolved.json")["appServerGenerationEnded"],
        true
    );
    runtime.record_session_ended(13);
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::SessionEndedOutcomeUnknown
    );
    assert!(runtime.confirmed_tombstone(&attempt).is_none());
}

#[test]
fn fixtures_contain_no_real_paths_tokens_or_owner_semantics() {
    for name in [
        "request-exact.json",
        "response-success.json",
        "response-active-writer.json",
        "notification-thread-deleted.json",
        "response-loss.json",
        "session-ended-unresolved.json",
    ] {
        let text = serde_json::to_string(&fixture(name))
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in [
            "token",
            "\\users\\",
            "writerowner",
            "deleteowner",
            "lease",
            "remoteclientidentity",
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}

#[test]
fn delete_isolation_fixtures_freeze_sanitized_contract() {
    let names = [
        "simultaneous-delete-same-thread.json",
        "simultaneous-delete-different-thread.json",
        "stale-transport-delete.json",
        "pre-dispatch-transport-loss.json",
        "post-dispatch-response-loss.json",
        "direct-success-after-transport-loss.json",
        "direct-rejection-after-transport-loss.json",
        "session-end-after-unknown.json",
        "new-explicit-intent-after-unknown.json",
        "stale-projection-after-confirmed-delete.json",
    ];
    for name in names {
        let fixture = isolation_fixture(name);
        assert!(fixture.get("case").and_then(Value::as_str).is_some());
        let text = serde_json::to_string(&fixture)
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in [
            "auth token",
            "\\users\\",
            "remoteclientidentity",
            "deleteowner",
            "primarydeleter",
            "leaseid",
            "forcetakeover",
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
    assert_eq!(
        isolation_fixture("simultaneous-delete-same-thread.json")["expectedDispatchCount"],
        1
    );
    assert_eq!(
        isolation_fixture("pre-dispatch-transport-loss.json")["expectedState"],
        "delete_rejected"
    );
    assert_eq!(
        isolation_fixture("post-dispatch-response-loss.json")["expectedState"],
        "delete_outcome_unknown"
    );
}

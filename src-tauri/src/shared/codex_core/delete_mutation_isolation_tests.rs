use super::delete_mutation_observation::{
    DeleteMutationFailureKind, DeleteMutationObservationRuntime, DeleteMutationRejectionReason,
    DeleteMutationRejectionSource, DeleteMutationState, DeleteNonTransitionEvent,
};
use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::RemoteHostIdentity;
use serde_json::json;

const THREAD_A: &str = "0199a8c0-1111-7222-8333-444455556666";
const THREAD_B: &str = "0199a8c0-1111-7222-8333-444455556667";

fn host() -> RemoteHostIdentity {
    RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap()
}

fn key(thread_id: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home-delete-isolation", thread_id)
}

fn runtime() -> DeleteMutationObservationRuntime {
    DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-current").unwrap(),
        AppServerConnectionGeneration::new("app-server-generation-current").unwrap(),
    )
}

fn begin(
    runtime: &DeleteMutationObservationRuntime,
    thread_id: &str,
    workspace_generation: &str,
    app_server_generation: &str,
    observed_at: i64,
) -> super::delete_mutation_observation::DeleteAttemptAdmission {
    runtime.begin_delete_attempt(
        host(),
        key(thread_id),
        thread_id,
        workspace_generation,
        app_server_generation,
        observed_at,
    )
}

#[test]
fn simultaneous_clients_same_thread_dispatch_at_most_once() {
    let runtime = runtime();
    let first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    let second = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        11,
    );
    assert!(first.is_admitted());
    assert!(!second.is_admitted());
    runtime.record_dispatched(first.attempt_id(), 12).unwrap();
    assert_eq!(runtime.dispatch_count(), 1);
}

#[test]
fn simultaneous_clients_have_distinct_delete_attempt_ids() {
    let runtime = runtime();
    let first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    let second = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        11,
    );
    assert_ne!(first.attempt_id(), second.attempt_id());
}

#[test]
fn simultaneous_second_attempt_is_local_pre_dispatch_rejection() {
    let runtime = runtime();
    let _first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    let second = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        11,
    );
    let observation = runtime.snapshot(second.attempt_id()).unwrap();
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(observation.dispatch_count, 0);
    assert_eq!(
        observation.rejection_source,
        Some(DeleteMutationRejectionSource::LocalPreDispatchRejection)
    );
    assert_eq!(
        observation.rejection_reason,
        Some(DeleteMutationRejectionReason::DuplicateActiveAttempt)
    );
}

#[test]
fn different_threads_can_delete_concurrently() {
    let runtime = runtime();
    let first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    let second = begin(
        &runtime,
        THREAD_B,
        "workspace-generation-current",
        "app-server-generation-current",
        11,
    );
    assert!(first.is_admitted());
    assert!(second.is_admitted());
    runtime.record_dispatched(first.attempt_id(), 12).unwrap();
    runtime.record_dispatched(second.attempt_id(), 13).unwrap();
    assert_eq!(runtime.dispatch_count(), 2);
}

#[test]
fn stale_workspace_generation_fails_before_dispatch() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-stale",
        "app-server-generation-current",
        10,
    );
    assert!(!attempt.is_admitted());
    let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
    assert_eq!(observation.dispatch_count, 0);
    assert_eq!(
        observation.rejection_reason,
        Some(DeleteMutationRejectionReason::StaleWorkspaceSessionGeneration)
    );
}

#[test]
fn stale_app_server_generation_fails_before_dispatch() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-stale",
        10,
    );
    assert!(!attempt.is_admitted());
    let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
    assert_eq!(observation.dispatch_count, 0);
    assert_eq!(
        observation.rejection_reason,
        Some(DeleteMutationRejectionReason::StaleAppServerGeneration)
    );
}

#[test]
fn disconnect_before_dispatch_does_not_create_unknown_outcome() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime
        .record_local_pre_dispatch_rejection(
            attempt.attempt_id(),
            DeleteMutationFailureKind::DispatchDisconnected,
            DeleteMutationRejectionReason::TransportDisconnectedBeforeDispatch,
            11,
        )
        .unwrap();
    let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(observation.dispatch_count, 0);
}

#[test]
fn disconnect_after_dispatch_creates_delete_outcome_unknown() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteOutcomeUnknown
    );
}

#[test]
fn direct_success_after_transport_loss_upgrades_to_delete_confirmed() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result": {}}), 13)
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn direct_upstream_rejection_after_transport_loss_records_delete_rejected() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    runtime
        .record_response(
            attempt.attempt_id(),
            &json!({"error":{"code":-32600,"message":"already has an active writer"}}),
            13,
        )
        .unwrap();
    let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(
        observation.rejection_source,
        Some(DeleteMutationRejectionSource::UpstreamRejection)
    );
    assert_eq!(
        observation.rejection_reason,
        Some(DeleteMutationRejectionReason::UpstreamActiveWriter)
    );
}

#[test]
fn delete_confirmed_cannot_be_downgraded_by_late_transport_loss() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result": {}}), 12)
        .unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            13,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn upstream_rejection_cannot_be_downgraded_by_late_transport_loss() {
    for (thread_id, response, expected_reason) in [
        (
            THREAD_A,
            json!({"error":{"code":-32600,"message":"already has an active writer"}}),
            DeleteMutationRejectionReason::UpstreamActiveWriter,
        ),
        (
            THREAD_B,
            json!({"error":{"code":-32602,"message":"invalid request"}}),
            DeleteMutationRejectionReason::UpstreamOther,
        ),
    ] {
        let runtime = runtime();
        let attempt = begin(
            &runtime,
            thread_id,
            "workspace-generation-current",
            "app-server-generation-current",
            10,
        );
        runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
        runtime
            .record_response(attempt.attempt_id(), &response, 12)
            .unwrap();
        runtime
            .record_outcome_unknown(
                attempt.attempt_id(),
                DeleteMutationFailureKind::ResponseLost,
                13,
            )
            .unwrap();

        let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
        assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
        assert_eq!(
            observation.rejection_source,
            Some(DeleteMutationRejectionSource::UpstreamRejection)
        );
        assert_eq!(observation.rejection_reason, Some(expected_reason));
    }
}

#[test]
fn session_end_after_dispatched_unknown_becomes_session_ended_outcome_unknown() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    assert_eq!(runtime.record_session_ended(13), 1);
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::SessionEndedOutcomeUnknown
    );
}

#[test]
fn unknown_previous_attempt_allows_new_explicit_intent_with_new_attempt_id() {
    let runtime = runtime();
    let first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(first.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            first.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    let second = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        13,
    );
    assert!(second.is_admitted());
    assert_ne!(first.attempt_id(), second.attempt_id());
}

#[test]
fn new_attempt_does_not_rewrite_previous_unknown_history() {
    let runtime = runtime();
    let first = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(first.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            first.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    let second = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        13,
    );
    runtime.record_dispatched(second.attempt_id(), 14).unwrap();
    runtime
        .record_response(second.attempt_id(), &json!({"result": {}}), 15)
        .unwrap();
    assert_eq!(
        runtime.snapshot(first.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteOutcomeUnknown
    );
    assert_eq!(
        runtime.snapshot(second.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn confirmed_delete_tombstone_is_idempotent() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result": {}}), 12)
        .unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result": {}}), 13)
        .unwrap();
    assert_eq!(runtime.tombstone_count(), 1);
}

#[test]
fn delete_rejected_preserves_local_vs_upstream_source() {
    let runtime = runtime();
    let local = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-stale",
        "app-server-generation-current",
        10,
    );
    let upstream = begin(
        &runtime,
        THREAD_B,
        "workspace-generation-current",
        "app-server-generation-current",
        11,
    );
    runtime
        .record_dispatched(upstream.attempt_id(), 12)
        .unwrap();
    runtime
        .record_response(
            upstream.attempt_id(),
            &json!({"error":{"code":-32600,"message":"already has an active writer"}}),
            13,
        )
        .unwrap();
    assert_eq!(
        runtime
            .snapshot(local.attempt_id())
            .unwrap()
            .rejection_source,
        Some(DeleteMutationRejectionSource::LocalPreDispatchRejection)
    );
    assert_eq!(
        runtime
            .snapshot(upstream.attempt_id())
            .unwrap()
            .rejection_source,
        Some(DeleteMutationRejectionSource::UpstreamRejection)
    );
}

#[test]
fn delete_model_contains_no_remote_client_owner_delete_owner_or_lease() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    let serialized =
        serde_json::to_string(&runtime.snapshot(attempt.attempt_id()).unwrap()).unwrap();
    for forbidden in [
        "remoteClientIdentity",
        "clientOwner",
        "deleteOwner",
        "primaryDeleter",
        "lease",
        "forceTakeover",
    ] {
        assert!(!serialized.contains(forbidden), "forbidden {forbidden}");
    }
}

#[test]
fn automatic_delete_retry_count_and_replay_count_are_zero() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    let observation = runtime.snapshot(attempt.attempt_id()).unwrap();
    assert_eq!(observation.retry_count, 0);
    assert_eq!(runtime.dispatch_count(), 1);
}

#[test]
fn duplicate_same_transport_request_creates_no_second_attempt() {
    use crate::shared::remote_request_provenance::RemoteRequestProvenanceRuntime;

    let provenance = RemoteRequestProvenanceRuntime::new_authenticated_transport();
    provenance
        .record_received(1, "delete_thread", 10)
        .expect("first transport request");
    assert!(provenance.record_received(1, "delete_thread", 11).is_err());
    let runtime = runtime();
    assert_eq!(runtime.attempt_count(), 0);
    assert_eq!(runtime.dispatch_count(), 0);
}

#[test]
fn new_session_does_not_inherit_old_active_delete() {
    let old = runtime();
    let old_attempt = begin(
        &old,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    assert!(old_attempt.is_admitted());
    let new = DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-new").unwrap(),
        AppServerConnectionGeneration::new("app-server-generation-new").unwrap(),
    );
    assert!(new.latest_for_thread(&key(THREAD_A)).is_none());
    assert_eq!(new.dispatch_count(), 0);
}

#[test]
fn remote_reconnect_does_not_replay_delete() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    let _new_transport =
        crate::shared::remote_request_provenance::RemoteRequestProvenanceRuntime::new_authenticated_transport();
    assert_eq!(runtime.dispatch_count(), 1);
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteOutcomeUnknown
    );
}

#[test]
fn daemon_restart_does_not_replay_delete() {
    let old = runtime();
    let attempt = begin(
        &old,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    old.record_dispatched(attempt.attempt_id(), 11).unwrap();
    old.record_session_ended(12);
    let restarted = DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-restarted").unwrap(),
        AppServerConnectionGeneration::new("app-server-generation-restarted").unwrap(),
    );
    assert_eq!(restarted.dispatch_count(), 0);
    assert!(restarted.latest_for_thread(&key(THREAD_A)).is_none());
    assert_eq!(
        old.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::SessionEndedOutcomeUnknown
    );
}

#[test]
fn stale_app_server_thread_deleted_cannot_modify_new_generation() {
    let current = DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-new").unwrap(),
        AppServerConnectionGeneration::new("app-server-generation-new").unwrap(),
    );
    let attempt = current.begin_delete_attempt(
        host(),
        key(THREAD_A),
        THREAD_A,
        "workspace-generation-new",
        "app-server-generation-new",
        10,
    );
    current.record_dispatched(attempt.attempt_id(), 11).unwrap();
    assert!(current
        .record_thread_deleted(
            &host(),
            "workspace-generation-current",
            "app-server-generation-current",
            THREAD_A,
            12,
        )
        .is_err());
    assert_eq!(
        current.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeletePending
    );
}

#[test]
fn stale_projection_cannot_resurrect_confirmed_delete() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result": {}}), 12)
        .unwrap();
    runtime
        .observe_non_transition(
            attempt.attempt_id(),
            DeleteNonTransitionEvent::UiProjectionRemoved,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
    assert_eq!(runtime.tombstone_count(), 1);
}

#[test]
fn projection_absence_cannot_confirm_delete() {
    let runtime = runtime();
    let attempt = begin(
        &runtime,
        THREAD_A,
        "workspace-generation-current",
        "app-server-generation-current",
        10,
    );
    runtime
        .observe_non_transition(
            attempt.attempt_id(),
            DeleteNonTransitionEvent::CatalogRemoved,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeletePending
    );
    assert_eq!(runtime.tombstone_count(), 0);
}

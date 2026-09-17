use super::delete_mutation_observation::{
    DeleteMutationFailureKind, DeleteMutationObservationRuntime, DeleteMutationState,
    DeleteNonTransitionEvent,
};
use crate::shared::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::RemoteHostIdentity;
use serde_json::json;

const THREAD_ID: &str = "0199a8c0-1111-7222-8333-444455556666";
const OTHER_THREAD_ID: &str = "0199a8c0-7777-7888-8999-aaaabbbbcccc";

fn host() -> RemoteHostIdentity {
    RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap()
}

fn key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-fixture", THREAD_ID)
}

fn runtime() -> DeleteMutationObservationRuntime {
    DeleteMutationObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-1").unwrap(),
        AppServerConnectionGeneration::new("app-server-generation-1").unwrap(),
    )
}

fn begin(
    runtime: &DeleteMutationObservationRuntime,
) -> super::delete_mutation_observation::DeleteAttemptId {
    runtime
        .begin_delete(
            host(),
            key(),
            THREAD_ID,
            "workspace-generation-1",
            "app-server-generation-1",
            10,
        )
        .unwrap()
}

#[test]
fn exact_thread_key_is_required_for_delete() {
    let runtime = runtime();
    assert!(runtime
        .begin_delete(
            host(),
            CodexThreadKey::new("", THREAD_ID),
            THREAD_ID,
            "workspace-generation-1",
            "app-server-generation-1",
            10,
        )
        .is_err());
}

#[test]
fn fuzzy_thread_match_is_rejected() {
    let runtime = runtime();
    for candidate in ["0199a8c0", "fixture title", "C:\\fixture", "sidebar-row-1"] {
        assert!(runtime
            .begin_delete(
                host(),
                CodexThreadKey::new("codex-home-fixture", candidate),
                candidate,
                "workspace-generation-1",
                "app-server-generation-1",
                10,
            )
            .is_err());
    }
    assert_eq!(runtime.dispatch_count(), 0);
}

#[test]
fn delete_attempt_has_unique_attempt_id() {
    let runtime = runtime();
    let first = begin(&runtime);
    let second = begin(&runtime);
    assert_ne!(first, second);
}

#[test]
fn delete_attempt_binds_workspace_session_generation() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    assert_eq!(
        runtime
            .snapshot(&attempt)
            .unwrap()
            .workspace_session_generation,
        "workspace-generation-1"
    );
}

#[test]
fn delete_attempt_binds_app_server_connection_generation() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    assert_eq!(
        runtime
            .snapshot(&attempt)
            .unwrap()
            .app_server_connection_generation,
        "app-server-generation-1"
    );
}

#[test]
fn delete_attempt_binds_remote_host_identity() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().remote_host_identity,
        host()
    );
}

#[test]
fn stale_workspace_generation_rejects_delete() {
    let runtime = runtime();
    assert!(runtime
        .begin_delete(
            host(),
            key(),
            THREAD_ID,
            "workspace-generation-old",
            "app-server-generation-1",
            10,
        )
        .is_err());
    assert_eq!(runtime.dispatch_count(), 0);
}

#[test]
fn stale_app_server_generation_rejects_delete() {
    let runtime = runtime();
    assert!(runtime
        .begin_delete(
            host(),
            key(),
            THREAD_ID,
            "workspace-generation-1",
            "app-server-generation-old",
            10,
        )
        .is_err());
    assert_eq!(runtime.dispatch_count(), 0);
}

#[test]
fn thread_delete_success_confirms_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(&attempt, &json!({"result": {}}), 12)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn exact_thread_deleted_event_confirms_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_thread_deleted(
            &host(),
            "workspace-generation-1",
            "app-server-generation-1",
            THREAD_ID,
            12,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn delete_confirmation_is_idempotent() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(&attempt, &json!({"result": {}}), 12)
        .unwrap();
    runtime
        .record_thread_deleted(
            &host(),
            "workspace-generation-1",
            "app-server-generation-1",
            THREAD_ID,
            13,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
    assert_eq!(runtime.tombstone_count(), 1);
}

#[test]
fn active_writer_rejection_records_delete_rejected() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(
            &attempt,
            &json!({"error":{"code":-32600,"message":"already has an active writer"}}),
            12,
        )
        .unwrap();
    let snapshot = runtime.snapshot(&attempt).unwrap();
    assert_eq!(snapshot.state, DeleteMutationState::DeleteRejected);
    assert_eq!(
        snapshot.failure_kind,
        Some(DeleteMutationFailureKind::BlockedByActiveWriter)
    );
}

#[test]
fn missing_rollout_without_upstream_success_does_not_confirm_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime
        .observe_non_transition(&attempt, DeleteNonTransitionEvent::RolloutMissing)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeletePending
    );
}

#[test]
fn dispatched_response_loss_records_delete_outcome_unknown() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeleteOutcomeUnknown
    );
}

#[test]
fn session_end_after_unknown_records_session_ended_outcome_unknown() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    runtime.record_session_ended(13);
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::SessionEndedOutcomeUnknown
    );
}

#[test]
fn remote_disconnect_alone_does_not_end_delete_session() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime
        .observe_non_transition(&attempt, DeleteNonTransitionEvent::RemoteTcpDisconnected)
        .unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeletePending
    );
}

#[test]
fn delete_pending_does_not_create_tombstone() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    assert!(runtime.confirmed_tombstone(&attempt).is_none());
}

#[test]
fn delete_outcome_unknown_does_not_create_tombstone() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    assert!(runtime.confirmed_tombstone(&attempt).is_none());
}

#[test]
fn confirmed_delete_creates_exact_tombstone() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_response(&attempt, &json!({"result": {}}), 12)
        .unwrap();
    let tombstone = runtime.confirmed_tombstone(&attempt).unwrap();
    assert_eq!(tombstone.thread_key, key());
    assert_eq!(tombstone.attempt_id, attempt);
}

#[test]
fn desktop_and_remote_confirmed_delete_use_same_reconciliation() {
    let runtime = runtime();
    let local = begin(&runtime);
    runtime.record_dispatched(&local, 11).unwrap();
    runtime
        .record_response(&local, &json!({"result": {}}), 12)
        .unwrap();
    let remote = begin(&runtime);
    runtime.record_dispatched(&remote, 13).unwrap();
    runtime
        .record_thread_deleted(
            &host(),
            "workspace-generation-1",
            "app-server-generation-1",
            THREAD_ID,
            14,
        )
        .unwrap();
    assert_eq!(
        runtime.confirmed_tombstone(&local).unwrap().thread_key,
        runtime.confirmed_tombstone(&remote).unwrap().thread_key
    );
}

#[test]
fn thread_closed_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::ThreadClosed);
}

#[test]
fn not_loaded_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::ThreadNotLoaded);
}

#[test]
fn ui_projection_removal_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::UiProjectionRemoved);
}

#[test]
fn sidebar_removal_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::SidebarRemoved);
}

#[test]
fn catalog_removal_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::CatalogRemoved);
}

#[test]
fn remote_client_close_does_not_confirm_delete() {
    non_transition_does_not_confirm(DeleteNonTransitionEvent::RemoteClientClosed);
}

fn non_transition_does_not_confirm(event: DeleteNonTransitionEvent) {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.observe_non_transition(&attempt, event).unwrap();
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeletePending
    );
    assert!(runtime.confirmed_tombstone(&attempt).is_none());
}

#[test]
fn automatic_delete_retry_is_zero() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    assert_eq!(runtime.snapshot(&attempt).unwrap().retry_count, 0);
}

#[test]
fn automatic_delete_replay_is_zero() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    runtime
        .record_outcome_unknown(&attempt, DeleteMutationFailureKind::ResponseLost, 12)
        .unwrap();
    assert_eq!(runtime.dispatch_count(), 1);
}

#[test]
fn delete_observation_contains_no_client_owner() {
    forbidden_serialized_fields_are_absent();
}

#[test]
fn delete_observation_contains_no_delete_owner() {
    forbidden_serialized_fields_are_absent();
}

#[test]
fn delete_observation_contains_no_lease() {
    forbidden_serialized_fields_are_absent();
}

fn forbidden_serialized_fields_are_absent() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    let serialized = serde_json::to_string(&runtime.snapshot(&attempt).unwrap()).unwrap();
    for forbidden in [
        "clientOwner",
        "remoteClient",
        "deleteOwner",
        "owner",
        "lease",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "forbidden field {forbidden}"
        );
    }
}

#[test]
fn mismatched_thread_deleted_event_cannot_confirm_delete() {
    let runtime = runtime();
    let attempt = begin(&runtime);
    runtime.record_dispatched(&attempt, 11).unwrap();
    assert!(runtime
        .record_thread_deleted(
            &host(),
            "workspace-generation-1",
            "app-server-generation-1",
            OTHER_THREAD_ID,
            12,
        )
        .is_err());
    assert_eq!(
        runtime.snapshot(&attempt).unwrap().state,
        DeleteMutationState::DeletePending
    );
}

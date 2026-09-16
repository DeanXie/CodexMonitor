use super::approval_decision_provenance::{
    validate_approval_decision_response, ApprovalDecisionFailureKind,
    ApprovalDecisionRemoteProvenance, ApprovalDecisionResponseKind, ApprovalDecisionState,
};
use super::approval_observation::{
    ApprovalObservationRuntime, ApprovalRequestKind, ApprovalSessionEndEvidenceKind,
};
use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

fn runtime() -> ApprovalObservationRuntime {
    ApprovalObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-1").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-1").unwrap(),
    )
}

fn request(method: &str, request_id: Value) -> Value {
    json!({
        "id": request_id,
        "method": method,
        "params": {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "item-1",
            "approvalId": "approval-1"
        }
    })
}

fn remote(name: &str, request_id: u64) -> ApprovalDecisionRemoteProvenance {
    ApprovalDecisionRemoteProvenance::new(name, request_id).unwrap()
}

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("app-server")
        .join("approval-decision-provenance")
        .join(name);
    serde_json::from_slice(
        &std::fs::read(&path)
            .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse fixture {}: {error}", path.display()))
}

fn observe_command(runtime: &ApprovalObservationRuntime) {
    runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7)),
            10,
        )
        .unwrap();
}

#[test]
fn pending_current_request_allows_one_decision_attempt() {
    let runtime = runtime();
    observe_command(&runtime);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    assert_eq!(attempt.state, ApprovalDecisionState::DecisionPending);
    assert_eq!(attempt.identity.as_ref().unwrap().thread_id, "thread-1");
    assert_eq!(
        attempt.response_kind,
        Some(ApprovalDecisionResponseKind::CommandAccept)
    );
    assert_eq!(
        attempt.workspace_session_generation,
        "workspace-generation-1"
    );
    assert_eq!(
        attempt.app_server_connection_generation,
        "connection-generation-1"
    );
}

#[test]
fn resolved_and_session_ended_requests_reject_late_decisions() {
    let resolved_runtime = runtime();
    observe_command(&resolved_runtime);
    assert!(resolved_runtime.record_server_request_resolved(
        resolved_runtime.workspace_session_generation(),
        resolved_runtime.app_server_connection_generation(),
        &json!({"params":{"requestId":7,"threadId":"thread-1"}}),
        11,
    ));
    assert!(resolved_runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            12
        )
        .is_err());
    assert_eq!(
        resolved_runtime.decision_attempts()[0].state,
        ApprovalDecisionState::DecisionStaleRejected
    );

    let ended_runtime = runtime();
    observe_command(&ended_runtime);
    ended_runtime.record_session_ended(
        ApprovalSessionEndEvidenceKind::AppServerProcessExited,
        "fixture exit",
        11,
    );
    assert!(ended_runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"decline"}),
            remote("t-b", 2),
            12
        )
        .is_err());
    assert_eq!(
        ended_runtime.decision_attempts()[0].failure_kind,
        Some(ApprovalDecisionFailureKind::ApprovalNotPending)
    );
}

#[test]
fn replacement_generations_do_not_accept_old_approval_identity() {
    let old = runtime();
    observe_command(&old);
    let replacement = ApprovalObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-2").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-2").unwrap(),
    );
    assert!(replacement
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-new", 1),
            20
        )
        .is_err());
    assert_eq!(
        replacement.decision_attempts()[0].state,
        ApprovalDecisionState::DecisionStaleRejected
    );
}

#[test]
fn simultaneous_remote_decisions_dispatch_at_most_once() {
    let runtime = Arc::new(runtime());
    observe_command(&runtime);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for (transport, request_id) in [("t-a", 1), ("t-b", 1)] {
        let runtime = Arc::clone(&runtime);
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            runtime.begin_remote_decision(
                &json!(7),
                &json!({"decision":"accept"}),
                remote(transport, request_id),
                11,
            )
        }));
    }
    barrier.wait();
    let admitted = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(Result::is_ok)
        .count();
    assert_eq!(admitted, 1);
    let attempts = runtime.decision_attempts();
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts
            .iter()
            .filter(|attempt| attempt.state == ApprovalDecisionState::DecisionPending)
            .count(),
        1
    );
}

#[test]
fn second_decision_attempt_fails_closed() {
    let runtime = runtime();
    observe_command(&runtime);
    runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    assert!(runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"decline"}),
            remote("t-b", 1),
            12
        )
        .is_err());
    assert_eq!(
        runtime.decision_attempts()[1].failure_kind,
        Some(ApprovalDecisionFailureKind::DuplicateDecisionAttempt)
    );
}

#[test]
fn disconnect_boundary_states_are_explicit_and_never_retry() {
    let before_runtime = runtime();
    observe_command(&before_runtime);
    let before = before_runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    before_runtime
        .record_decision_not_dispatched(
            &before.attempt_id,
            ApprovalDecisionFailureKind::TransportLostBeforeDispatch,
            12,
        )
        .unwrap();
    assert_eq!(
        before_runtime.decision_attempts()[0].state,
        ApprovalDecisionState::DecisionNotDispatched
    );

    let after_runtime = runtime();
    observe_command(&after_runtime);
    let after = after_runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-b", 1),
            11,
        )
        .unwrap();
    after_runtime
        .record_decision_dispatched(&after.attempt_id, 12)
        .unwrap();
    after_runtime
        .record_decision_outcome_unknown(
            &after.attempt_id,
            ApprovalDecisionFailureKind::RemoteResponseUnobserved,
            13,
        )
        .unwrap();
    let attempts = after_runtime.decision_attempts();
    let snapshot = &attempts[0];
    assert_eq!(
        snapshot.state,
        ApprovalDecisionState::DecisionOutcomeUnknown
    );
    assert_eq!(snapshot.dispatch_count, 1);
    assert_eq!(snapshot.retry_count, 0);
}

#[test]
fn resolved_and_item_completed_do_not_claim_a_specific_decision_won() {
    let runtime = runtime();
    observe_command(&runtime);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    runtime
        .record_decision_dispatched(&attempt.attempt_id, 12)
        .unwrap();
    assert!(runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &json!({"params":{"requestId":7,"threadId":"thread-1"}}),
        13,
    ));
    let serialized = serde_json::to_string(&runtime.decision_attempts()[0]).unwrap();
    assert!(serialized.contains("serverRequest/resolved"));
    for forbidden in ["won", "approver", "owner", "lease"] {
        assert!(!serialized.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn exact_item_completed_only_annotates_the_related_attempt() {
    let runtime = runtime();
    observe_command(&runtime);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    runtime
        .record_decision_dispatched(&attempt.attempt_id, 12)
        .unwrap();
    assert!(runtime.record_item_completed(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &json!({
            "params": {
                "requestId": 7,
                "threadId": "thread-1",
                "turnId": "turn-1",
                "item": {"id":"item-1","status":"completed"}
            }
        }),
        13,
    ));
    let snapshot = runtime.decision_attempts().remove(0);
    assert_eq!(snapshot.state, ApprovalDecisionState::DecisionDispatched);
    assert_eq!(
        snapshot.resolution_method.as_deref(),
        Some("item/completed")
    );
    assert_eq!(snapshot.dispatch_count, 1);
}

#[test]
fn session_end_after_dispatch_records_unknown_without_retry() {
    let runtime = runtime();
    observe_command(&runtime);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    runtime
        .record_decision_dispatched(&attempt.attempt_id, 12)
        .unwrap();
    runtime.record_session_ended(
        ApprovalSessionEndEvidenceKind::AppServerProcessExited,
        "fixture exit",
        13,
    );
    let snapshot = runtime.decision_attempts().remove(0);
    assert_eq!(
        snapshot.state,
        ApprovalDecisionState::DecisionOutcomeUnknown
    );
    assert_eq!(
        snapshot.failure_kind,
        Some(ApprovalDecisionFailureKind::SessionEndedAfterDispatch)
    );
    assert_eq!(snapshot.dispatch_count, 1);
    assert_eq!(snapshot.retry_count, 0);
}

#[test]
fn auto_review_resolution_rejects_late_human_decision() {
    let runtime = runtime();
    observe_command(&runtime);
    assert!(runtime.record_auto_review(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &json!({"method":"item/autoApprovalReview/completed","params":{"threadId":"thread-1","turnId":"turn-1","targetItemId":"item-1","review":{"status":"approved"}}}),
        11,
    ));
    assert!(runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &json!({"params":{"requestId":7,"threadId":"thread-1"}}),
        12,
    ));
    assert!(runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            13
        )
        .is_err());
}

#[test]
fn bundled_command_response_schema_is_typed() {
    let cases = [
        (
            json!({"decision":"accept"}),
            ApprovalDecisionResponseKind::CommandAccept,
        ),
        (
            json!({"decision":"acceptForSession"}),
            ApprovalDecisionResponseKind::CommandAcceptForSession,
        ),
        (
            json!({"decision":"decline"}),
            ApprovalDecisionResponseKind::CommandDecline,
        ),
        (
            json!({"decision":"cancel"}),
            ApprovalDecisionResponseKind::CommandCancel,
        ),
        (
            json!({"decision":{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["git","status"]}}}),
            ApprovalDecisionResponseKind::CommandExecPolicyAmendment,
        ),
        (
            json!({"decision":{"applyNetworkPolicyAmendment":{"network_policy_amendment":{"host":"example.com","action":"allow"}}}}),
            ApprovalDecisionResponseKind::CommandNetworkPolicyAmendment,
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(
            validate_approval_decision_response(ApprovalRequestKind::CommandExecution, &value)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn bundled_file_response_schema_is_typed() {
    for (decision, expected) in [
        ("accept", ApprovalDecisionResponseKind::FileAccept),
        (
            "acceptForSession",
            ApprovalDecisionResponseKind::FileAcceptForSession,
        ),
        ("decline", ApprovalDecisionResponseKind::FileDecline),
        ("cancel", ApprovalDecisionResponseKind::FileCancel),
    ] {
        assert_eq!(
            validate_approval_decision_response(
                ApprovalRequestKind::FileChange,
                &json!({"decision":decision}),
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn bundled_permissions_response_schema_is_typed() {
    let result = json!({
        "permissions": {
            "network": {"enabled": true},
            "fileSystem": {"read":["C:/repo"],"write":null}
        },
        "scope": "turn",
        "strictAutoReview": true
    });
    assert_eq!(
        validate_approval_decision_response(ApprovalRequestKind::Permissions, &result).unwrap(),
        ApprovalDecisionResponseKind::PermissionsGrant
    );
}

#[test]
fn bundled_response_fixtures_drive_the_typed_validator() {
    for response in fixture("command-responses.json")["responses"]
        .as_array()
        .expect("command responses")
    {
        validate_approval_decision_response(ApprovalRequestKind::CommandExecution, response)
            .expect("valid command response fixture");
    }
    for response in fixture("file-responses.json")["responses"]
        .as_array()
        .expect("file responses")
    {
        validate_approval_decision_response(ApprovalRequestKind::FileChange, response)
            .expect("valid file response fixture");
    }
    validate_approval_decision_response(
        ApprovalRequestKind::Permissions,
        &fixture("permissions-response.json")["response"],
    )
    .expect("valid permissions response fixture");
    assert!(validate_approval_decision_response(
        ApprovalRequestKind::Permissions,
        &fixture("wrong-kind-response.json")["response"],
    )
    .is_err());
}

#[test]
fn provenance_fixtures_freeze_zero_retry_and_forbidden_semantics() {
    let dispatch = fixture("dispatch-boundaries.json");
    assert_eq!(dispatch["beforeWrite"]["dispatchCount"], 0);
    assert_eq!(dispatch["beforeWrite"]["retryCount"], 0);
    assert_eq!(
        dispatch["afterWriteWithoutCallerResponse"]["dispatchCount"],
        1
    );
    assert_eq!(dispatch["afterWriteWithoutCallerResponse"]["retryCount"], 0);
    let generations = fixture("generation-and-duplicate-boundaries.json");
    assert_eq!(generations["oneAdmittedAttemptPerIdentity"], true);
    assert_eq!(generations["automaticRetry"], false);
    assert_eq!(generations["automaticReplay"], false);
    let serialized = format!("{dispatch}{generations}").to_ascii_lowercase();
    for forbidden in ["remoteclientidentity\":{", "approvalowner", "leaseid"] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn wrong_response_schema_fails_closed() {
    for (kind, value) in [
        (
            ApprovalRequestKind::Permissions,
            json!({"decision":"accept"}),
        ),
        (
            ApprovalRequestKind::FileChange,
            json!({"permissions":{},"scope":"turn"}),
        ),
        (
            ApprovalRequestKind::CommandExecution,
            json!({"decision":"acceptAlways"}),
        ),
        (
            ApprovalRequestKind::Permissions,
            json!({"permissions":{},"scope":"forever"}),
        ),
        (
            ApprovalRequestKind::Permissions,
            json!({"permissions":{},"scope":"session","strictAutoReview":true}),
        ),
        (
            ApprovalRequestKind::Permissions,
            json!({"permissions":{"fileSystem":{"globScanMaxDepth":0}},"scope":"turn"}),
        ),
    ] {
        assert!(validate_approval_decision_response(kind, &value).is_err());
    }
}

#[test]
fn reconnect_transport_provenance_is_distinct_from_approval_identity() {
    let runtime = runtime();
    observe_command(&runtime);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("transport-generation-new", 1),
            11,
        )
        .unwrap();
    assert_eq!(
        attempt.remote_transport_generation,
        "transport-generation-new"
    );
    assert_eq!(attempt.transport_request_id, 1);
    assert_eq!(
        attempt
            .identity
            .as_ref()
            .unwrap()
            .workspace_session_generation,
        "workspace-generation-1"
    );
}

#[test]
fn decision_provenance_contains_no_client_ownership_or_lease_semantics() {
    let runtime = runtime();
    observe_command(&runtime);
    runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote("t-a", 1),
            11,
        )
        .unwrap();
    let serialized = serde_json::to_string(&runtime.decision_attempts()).unwrap();
    for forbidden in ["remoteClient", "approverOwner", "approvalOwner", "lease"] {
        assert!(!serialized
            .to_ascii_lowercase()
            .contains(&forbidden.to_ascii_lowercase()));
    }
}

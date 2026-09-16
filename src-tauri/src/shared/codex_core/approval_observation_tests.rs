use super::approval_observation::{
    reconcile_approval_observation_message, ApprovalObservationRuntime, ApprovalObservationState,
    ApprovalRequestKind, ApprovalSessionEndEvidenceKind,
};
use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use serde_json::json;
use std::path::PathBuf;

fn runtime() -> ApprovalObservationRuntime {
    ApprovalObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-1").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-1").unwrap(),
    )
}

fn request(method: &str, request_id: serde_json::Value, item_id: &str) -> serde_json::Value {
    json!({
        "id": request_id,
        "method": method,
        "params": {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": item_id,
            "approvalId": "approval-1",
            "availableDecisions": ["accept", "decline"],
            "command": ["secret", "argument"],
            "cwd": "C:/secret"
        }
    })
}

fn fixture(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("app-server")
        .join("approval-request-observation")
        .join(name);
    serde_json::from_slice(
        &std::fs::read(&path)
            .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse fixture {}: {error}", path.display()))
}

#[test]
fn observes_three_approval_request_kinds() {
    let runtime = runtime();
    let cases = [
        (
            "item/commandExecution/requestApproval",
            ApprovalRequestKind::CommandExecution,
        ),
        (
            "item/fileChange/requestApproval",
            ApprovalRequestKind::FileChange,
        ),
        (
            "item/permissions/requestApproval",
            ApprovalRequestKind::Permissions,
        ),
    ];
    for (index, (method, kind)) in cases.into_iter().enumerate() {
        let snapshot = runtime
            .observe_request(
                &request(method, json!(index + 1), &format!("item-{index}")),
                10 + index as i64,
            )
            .unwrap();
        assert_eq!(snapshot.kind, kind);
        assert_eq!(snapshot.state, ApprovalObservationState::Pending);
    }
    assert_eq!(runtime.current_pending().len(), 3);
}

#[test]
fn mcp_elicitation_is_not_an_approval_request() {
    let runtime = runtime();
    let error = runtime
        .observe_request(
            &request("mcpServer/elicitation/request", json!(1), "item-1"),
            10,
        )
        .unwrap_err();
    assert!(error.contains("unsupported approval request method"));
    assert!(runtime.current_pending().is_empty());
}

#[test]
fn approval_identity_is_generation_scoped() {
    let runtime = runtime();
    let snapshot = runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    assert_eq!(
        snapshot.identity.workspace_session_generation,
        "workspace-generation-1"
    );
    assert_eq!(
        snapshot.identity.app_server_connection_generation,
        "connection-generation-1"
    );
    assert_eq!(snapshot.identity.thread_id, "thread-1");
    assert_eq!(snapshot.identity.turn_id, "turn-1");
    assert_eq!(snapshot.identity.item_id, "item-1");
}

#[test]
fn server_request_resolved_requires_exact_current_identity() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    let wrong_thread = json!({"params":{"requestId":7,"threadId":"thread-other"}});
    assert!(!runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &wrong_thread,
        11,
    ));
    let resolved = json!({"params":{"requestId":7,"threadId":"thread-1"}});
    assert!(runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &resolved,
        12,
    ));
    assert!(runtime.current_pending().is_empty());
    assert_eq!(
        runtime.history().last().unwrap().state,
        ApprovalObservationState::ResolvedOrCleared
    );
}

#[test]
fn stale_generation_cannot_resolve_current_request() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/fileChange/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    let stale_workspace = WorkspaceSessionGeneration::new("workspace-generation-old").unwrap();
    let stale_connection = AppServerConnectionGeneration::new("connection-generation-old").unwrap();
    let resolved = json!({"params":{"requestId":7,"threadId":"thread-1"}});
    assert!(!runtime.record_server_request_resolved(
        &stale_workspace,
        runtime.app_server_connection_generation(),
        &resolved,
        11
    ));
    assert!(!runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        &stale_connection,
        &resolved,
        12
    ));
    assert_eq!(runtime.current_pending().len(), 1);
}

#[test]
fn item_completed_requires_request_thread_turn_and_item_exact_match() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/permissions/requestApproval", json!("req-1"), "item-1"),
            10,
        )
        .unwrap();
    let incomplete = json!({"params":{"threadId":"thread-1","turnId":"turn-1","item":{"id":"item-1","status":"completed"}}});
    assert!(!runtime.record_item_completed(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &incomplete,
        11
    ));
    for mismatched in [
        json!({"params":{"requestId":"req-other","threadId":"thread-1","turnId":"turn-1","item":{"id":"item-1","status":"completed"}}}),
        json!({"params":{"requestId":"req-1","threadId":"thread-other","turnId":"turn-1","item":{"id":"item-1","status":"completed"}}}),
        json!({"params":{"requestId":"req-1","threadId":"thread-1","turnId":"turn-other","item":{"id":"item-1","status":"completed"}}}),
        json!({"params":{"requestId":"req-1","threadId":"thread-1","turnId":"turn-1","item":{"id":"item-other","status":"completed"}}}),
    ] {
        assert!(!runtime.record_item_completed(
            runtime.workspace_session_generation(),
            runtime.app_server_connection_generation(),
            &mismatched,
            11
        ));
    }
    let stale_connection = AppServerConnectionGeneration::new("connection-generation-old").unwrap();
    let exact = json!({"params":{"requestId":"req-1","threadId":"thread-1","turnId":"turn-1","item":{"id":"item-1","status":"completed"}}});
    assert!(!runtime.record_item_completed(
        runtime.workspace_session_generation(),
        &stale_connection,
        &exact,
        11
    ));
    assert!(runtime.record_item_completed(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &exact,
        12
    ));
    assert!(runtime.current_pending().is_empty());
}

#[test]
fn auto_review_is_annotation_not_human_decision() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    let event = json!({"method":"item/autoApprovalReview/started","params":{"threadId":"thread-1","turnId":"turn-1","targetItemId":"item-1","reviewId":"review-1","review":{"status":"inProgress"}}});
    assert!(runtime.record_auto_review(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &event,
        11
    ));
    let current = runtime.current_pending();
    assert_eq!(current[0].state, ApprovalObservationState::Pending);
    assert_eq!(current[0].auto_review.as_ref().unwrap().phase, "started");
}

#[test]
fn local_allowlist_is_not_upstream_auto_review() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    runtime.record_local_allowlist_non_transition();
    assert!(runtime.current_pending()[0].auto_review.is_none());
}

#[test]
fn real_session_end_marks_pending_unresolved() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    assert_eq!(
        runtime.record_session_ended(
            ApprovalSessionEndEvidenceKind::AppServerProcessExited,
            "process exited",
            11
        ),
        1
    );
    assert!(runtime.current_pending().is_empty());
    assert_eq!(
        runtime.history().last().unwrap().state,
        ApprovalObservationState::SessionEndedUnresolved
    );
}

#[test]
fn remote_transport_disconnect_is_a_non_transition() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/fileChange/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    runtime.record_remote_transport_non_transition();
    assert_eq!(
        runtime.current_pending()[0].state,
        ApprovalObservationState::Pending
    );
}

#[test]
fn new_generation_does_not_inherit_pending_requests() {
    let runtime = runtime();
    runtime
        .observe_request(
            &request("item/fileChange/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    let replacement = ApprovalObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-2").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-2").unwrap(),
    );
    assert!(replacement.current_pending().is_empty());
    let replacement_snapshot = replacement
        .observe_request(
            &request("item/fileChange/requestApproval", json!(7), "item-2"),
            11,
        )
        .unwrap();
    assert_eq!(
        replacement_snapshot.identity.workspace_session_generation,
        "workspace-generation-2"
    );
    assert_eq!(replacement_snapshot.identity.item_id, "item-2");
    assert_eq!(runtime.current_pending()[0].identity.item_id, "item-1");
}

#[test]
fn shared_remote_clients_observe_one_session_registry() {
    let runtime = std::sync::Arc::new(runtime());
    let client_a = runtime.clone();
    let client_b = runtime.clone();
    client_a
        .observe_request(
            &request("item/permissions/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    assert_eq!(client_b.current_pending(), client_a.current_pending());
}

#[test]
fn serialized_snapshot_excludes_secrets_and_forbidden_semantics() {
    let runtime = runtime();
    let snapshot = runtime
        .observe_request(
            &request("item/commandExecution/requestApproval", json!(7), "item-1"),
            10,
        )
        .unwrap();
    let serialized = serde_json::to_string(&snapshot).unwrap();
    for forbidden in [
        "secret",
        "C:/secret",
        "free",
        "available",
        "released",
        "owner",
        "lease",
        "accepted",
        "declined",
    ] {
        assert!(
            !serialized
                .to_ascii_lowercase()
                .contains(&forbidden.to_ascii_lowercase()),
            "forbidden value leaked: {forbidden}"
        );
    }
}

#[test]
fn sanitized_protocol_fixtures_cover_request_and_resolution_lifecycle() {
    let runtime = runtime();
    for name in [
        "command-execution-request.json",
        "file-change-request.json",
        "permissions-request.json",
    ] {
        runtime.observe_request(&fixture(name), 10).unwrap();
    }
    assert_eq!(runtime.current_pending().len(), 3);
    assert!(runtime.record_auto_review(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &fixture("auto-review.json"),
        11,
    ));
    assert!(runtime.record_server_request_resolved(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &fixture("server-request-resolved.json"),
        12,
    ));
    assert!(runtime.record_item_completed(
        runtime.workspace_session_generation(),
        runtime.app_server_connection_generation(),
        &fixture("item-completed-exact.json"),
        13,
    ));
    assert_eq!(runtime.current_pending().len(), 1);
}

#[test]
fn production_reconciler_ingests_before_projection_without_decision_mutation() {
    let runtime = runtime();
    reconcile_approval_observation_message(
        &runtime,
        &fixture("command-execution-request.json"),
        10,
    );
    assert_eq!(runtime.current_pending().len(), 1);
    reconcile_approval_observation_message(&runtime, &fixture("server-request-resolved.json"), 11);
    assert!(runtime.current_pending().is_empty());
    assert_eq!(
        runtime.history().last().unwrap().state,
        ApprovalObservationState::ResolvedOrCleared
    );
}

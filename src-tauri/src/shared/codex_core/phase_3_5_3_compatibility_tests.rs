use super::approval_decision_provenance::{
    ApprovalDecisionRemoteProvenance, ApprovalDecisionState,
};
use super::approval_observation::{ApprovalObservationRuntime, ApprovalObservationState};
use super::delete_mutation_observation::{
    DeleteMutationFailureKind, DeleteMutationObservation, DeleteMutationObservationRuntime,
    DeleteMutationRejectionReason, DeleteMutationRejectionSource, DeleteMutationState,
    DeleteNonTransitionEvent,
};
use super::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::{DaemonProcessGeneration, RemoteHostIdentity};
use crate::shared::remote_request_provenance::RemoteTransportGeneration;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

const FIXTURE_DIRECTORY: &str = "phase-3-5-3-compatibility";
const THREAD_A: &str = "0199a8c0-1111-7222-8333-444455556666";
const THREAD_B: &str = "0199a8c0-1111-7222-8333-444455556667";
const WORKSPACE_GENERATION: &str = "workspace-generation-compatibility";
const APP_SERVER_GENERATION: &str = "app-server-generation-compatibility";
const REQUIRED_FIXTURE_FAMILIES: &[&str] = &[
    "approval request command",
    "approval request file",
    "approval request permissions",
    "approval resolved",
    "approval item completed",
    "approval auto review",
    "approval decision dispatch",
    "approval stale decision",
    "approval multi client",
    "delete exact request",
    "delete success",
    "delete upstream rejection",
    "delete thread deleted",
    "delete response loss",
    "delete session ended",
    "delete same thread multi client",
    "delete different thread concurrency",
    "delete stale generation",
    "delete pre dispatch loss",
    "delete post dispatch loss",
    "delete direct evidence after transport loss",
    "delete new intent after unknown",
    "delete stale projection",
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
}

fn fixture_path(name: &str) -> PathBuf {
    fixture_root()
        .join("app-server")
        .join(FIXTURE_DIRECTORY)
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let path = fixture_path(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read compatibility fixture {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse compatibility fixture {}: {error}", path.display()))
}

fn authority() -> Value {
    let fixture = load_fixture("authority-contract.json");
    assert_eq!(fixture["schemaVersion"], 1);
    fixture
}

fn workspace_generation(value: &str) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(value).expect("WorkspaceSession generation")
}

fn app_server_generation(value: &str) -> AppServerConnectionGeneration {
    AppServerConnectionGeneration::new(value).expect("app-server generation")
}

fn approval_runtime(workspace: &str, app_server: &str) -> ApprovalObservationRuntime {
    ApprovalObservationRuntime::new(
        workspace_generation(workspace),
        app_server_generation(app_server),
    )
}

fn approval_request(request_id: i64) -> Value {
    json!({
        "id": request_id,
        "method": "item/commandExecution/requestApproval",
        "params": {
            "threadId": THREAD_A,
            "turnId": "turn-compatibility",
            "itemId": "item-compatibility",
            "approvalId": "approval-compatibility",
            "availableDecisions": ["accept", "decline", "cancel"]
        }
    })
}

fn observe_approval(runtime: &ApprovalObservationRuntime, request_id: i64) {
    runtime
        .observe_request(&approval_request(request_id), 10)
        .expect("observe approval request");
}

fn remote_provenance(transport: &str, request_id: u64) -> ApprovalDecisionRemoteProvenance {
    ApprovalDecisionRemoteProvenance::new(transport, request_id)
        .expect("Remote approval provenance")
}

fn host() -> RemoteHostIdentity {
    RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").expect("RemoteHostIdentity")
}

fn thread_key(thread_id: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home-compatibility", thread_id)
}

fn delete_runtime() -> DeleteMutationObservationRuntime {
    DeleteMutationObservationRuntime::new(
        workspace_generation(WORKSPACE_GENERATION),
        app_server_generation(APP_SERVER_GENERATION),
    )
}

fn begin_delete(
    runtime: &DeleteMutationObservationRuntime,
    thread_id: &str,
    observed_at: i64,
) -> super::delete_mutation_observation::DeleteAttemptAdmission {
    runtime.begin_delete_attempt(
        host(),
        thread_key(thread_id),
        thread_id,
        WORKSPACE_GENERATION,
        APP_SERVER_GENERATION,
        observed_at,
    )
}

fn normalized_delete_snapshot(snapshot: &DeleteMutationObservation) -> Value {
    let mut value = serde_json::to_value(snapshot).expect("serialize delete observation");
    value.as_object_mut().unwrap().remove("attemptId");
    value
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|entry| entry.as_str().expect("string entry").to_string())
        .collect()
}

fn object_keys(value: &Value, keys: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                keys.push(key.clone());
                object_keys(value, keys);
            }
        }
        Value::Array(values) => {
            for value in values {
                object_keys(value, keys);
            }
        }
        _ => {}
    }
}

#[test]
fn approval_observation_schema_stable() {
    let contract = authority();
    let runtime = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    let snapshot = runtime.observe_request(&approval_request(7), 10).unwrap();
    let serialized = serde_json::to_value(snapshot).unwrap();
    let actual = serialized
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let expected = strings(&contract["approval"]["observationFields"])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(serialized["state"], "pending");
    assert_eq!(
        serde_json::to_value(ApprovalObservationState::NotObserved).unwrap(),
        "not_observed"
    );
}

#[test]
fn approval_decision_provenance_schema_stable() {
    let contract = authority();
    let runtime = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    observe_approval(&runtime, 7);
    let attempt = runtime
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote_provenance("transport-generation-a", 41),
            11,
        )
        .unwrap();
    let serialized = serde_json::to_value(attempt).unwrap();
    let actual = serialized
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let expected = strings(&contract["approval"]["decisionAttemptFields"])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(serialized["state"], "decision_pending");
}

#[test]
fn approval_app_daemon_semantics_match() {
    let app = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    let daemon = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    let message = approval_request(7);
    assert_eq!(
        app.observe_request(&message, 10).unwrap(),
        daemon.observe_request(&message, 10).unwrap()
    );
}

#[test]
fn approval_same_request_new_generation_isolated() {
    let old = approval_runtime("workspace-old", "connection-old");
    observe_approval(&old, 7);
    let current = approval_runtime("workspace-current", "connection-current");
    assert!(current
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"accept"}),
            remote_provenance("transport-current", 7),
            20,
        )
        .is_err());
    assert_eq!(
        current.decision_attempts()[0].state,
        ApprovalDecisionState::DecisionStaleRejected
    );
    assert_eq!(old.current_pending().len(), 1);
}

#[test]
fn approval_multi_client_single_dispatch_contract_stable() {
    let runtime = Arc::new(approval_runtime(
        WORKSPACE_GENERATION,
        APP_SERVER_GENERATION,
    ));
    observe_approval(&runtime, 7);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for transport in ["transport-a", "transport-b"] {
        let runtime = Arc::clone(&runtime);
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            runtime.begin_remote_decision(
                &json!(7),
                &json!({"decision":"accept"}),
                remote_provenance(transport, 1),
                11,
            )
        }));
    }
    barrier.wait();
    assert_eq!(
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .filter(Result::is_ok)
            .count(),
        1
    );
    let attempts = runtime.decision_attempts();
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts
            .iter()
            .filter(|a| a.state == ApprovalDecisionState::DecisionPending)
            .count(),
        1
    );
}

#[test]
fn delete_observation_schema_stable() {
    let contract = authority();
    let runtime = delete_runtime();
    let attempt = begin_delete(&runtime, THREAD_A, 10);
    let serialized = serde_json::to_value(runtime.snapshot(attempt.attempt_id()).unwrap()).unwrap();
    let actual = serialized
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let expected = strings(&contract["delete"]["observationFields"])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(serialized["state"], "delete_pending");
}

#[test]
fn delete_attempt_schema_stable() {
    let runtime = delete_runtime();
    let first = begin_delete(&runtime, THREAD_A, 10);
    let second = begin_delete(&runtime, THREAD_A, 11);
    assert_ne!(first.attempt_id(), second.attempt_id());
    assert!(first.is_admitted());
    assert!(!second.is_admitted());
    assert_eq!(
        runtime
            .snapshot(second.attempt_id())
            .unwrap()
            .rejection_source,
        Some(DeleteMutationRejectionSource::LocalPreDispatchRejection)
    );
}

#[test]
fn delete_app_daemon_semantics_match() {
    let app = delete_runtime();
    let daemon = delete_runtime();
    let app_attempt = begin_delete(&app, THREAD_A, 10);
    let daemon_attempt = begin_delete(&daemon, THREAD_A, 10);
    assert_eq!(
        normalized_delete_snapshot(&app.snapshot(app_attempt.attempt_id()).unwrap()),
        normalized_delete_snapshot(&daemon.snapshot(daemon_attempt.attempt_id()).unwrap())
    );
}

#[test]
fn delete_exact_thread_key_contract_stable() {
    let runtime = delete_runtime();
    let rejected = runtime.begin_delete_attempt(
        host(),
        CodexThreadKey::new("codex-home-compatibility", "fixture title"),
        "fixture title",
        WORKSPACE_GENERATION,
        APP_SERVER_GENERATION,
        10,
    );
    assert!(!rejected.is_admitted());
    assert_eq!(runtime.dispatch_count(), 0);
    assert_eq!(
        runtime
            .snapshot(rejected.attempt_id())
            .unwrap()
            .rejection_reason,
        Some(DeleteMutationRejectionReason::InvalidExactThreadKey)
    );
}

#[test]
fn delete_same_thread_single_dispatch_contract_stable() {
    let runtime = delete_runtime();
    let first = begin_delete(&runtime, THREAD_A, 10);
    let second = begin_delete(&runtime, THREAD_A, 11);
    runtime.record_dispatched(first.attempt_id(), 12).unwrap();
    assert!(first.is_admitted());
    assert!(!second.is_admitted());
    assert_eq!(runtime.dispatch_count(), 1);
}

#[test]
fn delete_different_thread_concurrency_stable() {
    let runtime = delete_runtime();
    let first = begin_delete(&runtime, THREAD_A, 10);
    let second = begin_delete(&runtime, THREAD_B, 11);
    runtime.record_dispatched(first.attempt_id(), 12).unwrap();
    runtime.record_dispatched(second.attempt_id(), 13).unwrap();
    assert!(first.is_admitted());
    assert!(second.is_admitted());
    assert_eq!(runtime.dispatch_count(), 2);
}

#[test]
fn delete_rejection_source_stable() {
    let runtime = delete_runtime();
    let first = begin_delete(&runtime, THREAD_A, 10);
    let local = begin_delete(&runtime, THREAD_A, 11);
    runtime.record_dispatched(first.attempt_id(), 12).unwrap();
    runtime
        .record_response(
            first.attempt_id(),
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
            .snapshot(first.attempt_id())
            .unwrap()
            .rejection_source,
        Some(DeleteMutationRejectionSource::UpstreamRejection)
    );
}

#[test]
fn delete_direct_evidence_precedence_stable() {
    let runtime = delete_runtime();
    let attempt = begin_delete(&runtime, THREAD_A, 10);
    runtime.record_dispatched(attempt.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    runtime
        .record_response(attempt.attempt_id(), &json!({"result":{}}), 13)
        .unwrap();
    runtime
        .record_outcome_unknown(
            attempt.attempt_id(),
            DeleteMutationFailureKind::DispatchDisconnected,
            14,
        )
        .unwrap();
    assert_eq!(
        runtime.snapshot(attempt.attempt_id()).unwrap().state,
        DeleteMutationState::DeleteConfirmed
    );
}

#[test]
fn delete_tombstone_gate_stable() {
    let runtime = delete_runtime();
    let unknown = begin_delete(&runtime, THREAD_A, 10);
    runtime.record_dispatched(unknown.attempt_id(), 11).unwrap();
    runtime
        .record_outcome_unknown(
            unknown.attempt_id(),
            DeleteMutationFailureKind::ResponseLost,
            12,
        )
        .unwrap();
    assert!(runtime.confirmed_tombstone(unknown.attempt_id()).is_none());

    let confirmed = begin_delete(&runtime, THREAD_B, 13);
    runtime
        .record_dispatched(confirmed.attempt_id(), 14)
        .unwrap();
    runtime
        .record_response(confirmed.attempt_id(), &json!({"result":{}}), 15)
        .unwrap();
    runtime
        .observe_non_transition(
            confirmed.attempt_id(),
            DeleteNonTransitionEvent::UiProjectionRemoved,
        )
        .unwrap();
    assert_eq!(
        runtime
            .confirmed_tombstone(confirmed.attempt_id())
            .unwrap()
            .thread_key,
        thread_key(THREAD_B)
    );
}

#[test]
fn retry_replay_zero_contract_stable() {
    let contract = authority();
    for field in [
        "approvalRetryCount",
        "approvalReplayCount",
        "deleteRetryCount",
        "deleteReplayCount",
    ] {
        assert_eq!(contract["mutationPolicy"][field], 0, "policy {field}");
    }
    let approval = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    observe_approval(&approval, 7);
    let approval_attempt = approval
        .begin_remote_decision(
            &json!(7),
            &json!({"decision":"decline"}),
            remote_provenance("transport-generation-a", 7),
            11,
        )
        .unwrap();
    assert_eq!(approval_attempt.retry_count, 0);
    let delete = delete_runtime();
    let delete_attempt = begin_delete(&delete, THREAD_A, 10);
    assert_eq!(
        delete
            .snapshot(delete_attempt.attempt_id())
            .unwrap()
            .retry_count,
        0
    );
}

#[test]
fn generation_hierarchy_contract_stable() {
    assert_eq!(
        strings(&authority()["generationHierarchy"]),
        [
            "RemoteHostIdentity",
            "DaemonProcessGeneration",
            "RemoteTransportGeneration",
            "WorkspaceSessionGeneration",
            "AppServerConnectionGeneration",
        ]
    );
    assert_eq!(
        serde_json::to_value(host()).unwrap(),
        "6ba7b810-9dad-41d1-80b4-00c04fd430c8"
    );
    assert_eq!(
        serde_json::to_value(DaemonProcessGeneration::new("daemon-process-a").unwrap()).unwrap(),
        "daemon-process-a"
    );
    assert_eq!(
        serde_json::to_value(RemoteTransportGeneration::new("transport-a").unwrap()).unwrap(),
        "transport-a"
    );
    assert_eq!(workspace_generation("workspace-a").as_str(), "workspace-a");
    assert_eq!(
        app_server_generation("app-server-a").as_str(),
        "app-server-a"
    );
}

#[test]
fn fixture_families_are_present_and_sanitized() {
    let manifest = load_fixture("fixture-family-manifest.json");
    assert_eq!(manifest["schemaVersion"], 1);
    let families = manifest["families"].as_array().expect("fixture families");
    let actual_names = families
        .iter()
        .map(|family| family["name"].as_str().expect("fixture family name"))
        .collect::<HashSet<_>>();
    let expected_names = REQUIRED_FIXTURE_FAMILIES
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert!(
        expected_names.is_subset(&actual_names),
        "missing required fixture families: {:?}",
        expected_names.difference(&actual_names).collect::<Vec<_>>()
    );
    for family in families {
        let path = fixture_root().join(family["path"].as_str().expect("fixture family path"));
        assert!(path.exists(), "missing fixture family {}", path.display());
        assert!(path.starts_with(fixture_root()));
    }
}

#[test]
fn forbidden_semantics_absent() {
    let approval = approval_runtime(WORKSPACE_GENERATION, APP_SERVER_GENERATION);
    let approval_snapshot = approval.observe_request(&approval_request(7), 10).unwrap();
    let delete = delete_runtime();
    let delete_attempt = begin_delete(&delete, THREAD_A, 10);
    let values = [
        authority(),
        load_fixture("fixture-family-manifest.json"),
        serde_json::to_value(approval_snapshot).unwrap(),
        serde_json::to_value(delete.snapshot(delete_attempt.attempt_id()).unwrap()).unwrap(),
    ];
    let forbidden = [
        "remoteClientIdentity",
        "approvalOwner",
        "approverOwner",
        "deleteOwner",
        "primaryDeleter",
        "writerOwner",
        "subscriptionOwner",
        "leaseId",
        "forceTakeover",
        "writerFree",
        "available",
        "released",
    ];
    for value in values {
        let mut keys = Vec::new();
        object_keys(&value, &mut keys);
        for forbidden in forbidden {
            assert!(
                !keys.iter().any(|key| key.eq_ignore_ascii_case(forbidden)),
                "forbidden schema field {forbidden}"
            );
        }
        let states = value
            .pointer("/approval/observationStates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .chain(
                value
                    .pointer("/delete/states")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten(),
            );
        for state in states {
            assert!(!matches!(
                state.as_str(),
                Some("free" | "available" | "released")
            ));
        }
    }
}

#[test]
fn fixture_paths_remain_inside_repository() {
    for name in ["authority-contract.json", "fixture-family-manifest.json"] {
        assert!(fixture_path(name).starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")));
    }
}

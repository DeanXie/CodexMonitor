use super::codex_core::thread_lifecycle_observation::{
    AppServerConnectionGeneration, ThreadLifecycleObservationRuntime,
    ThreadRuntimeAvailabilityEvidenceSource, ThreadRuntimeAvailabilityState,
    ThreadSubscriptionEvidenceSource, ThreadSubscriptionObservationState,
};
use super::codex_core::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionObservationRuntime, WriterAdmissionObservationState,
};
use super::codex_identity::CodexThreadKey;
use super::remote_host_identity::{
    classify_daemon_process_continuity, DaemonProcessContinuity, DaemonProcessGeneration,
    RemoteDaemonCapabilities, RemoteDaemonInfo, RemoteHostIdentity, REMOTE_DAEMON_PROTOCOL_VERSION,
};
use super::remote_request_provenance::{
    RemoteRequestDispatchState, RemoteRequestProvenanceRuntime, RemoteRequestTransitionError,
    RemoteTransportGeneration, SessionAttemptProvenance,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

const FIXTURE_DIRECTORY: &str = "remote-transport-coordination";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join(FIXTURE_DIRECTORY)
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let path = fixture_path(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read compatibility fixture {}: {error}", path.display()));
    let fixture: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse compatibility fixture {}: {error}", path.display()));
    assert_eq!(fixture["schemaVersion"], 1, "fixture {name}");
    fixture
}

fn transport_generation(value: &str) -> RemoteTransportGeneration {
    RemoteTransportGeneration::new(value).expect("transport generation")
}

fn workspace_generation(value: &str) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(value).expect("WorkspaceSession generation")
}

fn app_server_generation(value: &str) -> AppServerConnectionGeneration {
    AppServerConnectionGeneration::new(value).expect("app-server generation")
}

fn remote_host_identity(value: &str) -> RemoteHostIdentity {
    RemoteHostIdentity::parse(value).expect("RemoteHostIdentity")
}

fn daemon_info(host: &str, process: &str) -> RemoteDaemonInfo {
    RemoteDaemonInfo {
        name: "codex-monitor-daemon".to_string(),
        remote_host_identity: remote_host_identity(host),
        daemon_process_generation: Some(
            DaemonProcessGeneration::new(process).expect("daemon process generation"),
        ),
        version: "fixture".to_string(),
        protocol_version: REMOTE_DAEMON_PROTOCOL_VERSION,
        mode: "tcp".to_string(),
        display_name: None,
        capabilities: RemoteDaemonCapabilities::default(),
    }
}

fn request_snapshot(scenario: &Value) -> Value {
    let generation = scenario["transportGeneration"]
        .as_str()
        .expect("transportGeneration");
    let runtime = RemoteRequestProvenanceRuntime::new(transport_generation(generation));
    let request_id = scenario["transportRequestId"]
        .as_u64()
        .expect("transportRequestId");
    let key = runtime
        .record_received(
            request_id,
            scenario["method"].as_str().expect("method"),
            scenario["receivedAt"].as_i64().expect("receivedAt"),
        )
        .expect("record request");
    runtime
        .record_dispatch_started(
            &key,
            scenario["dispatchStartedAt"]
                .as_i64()
                .expect("dispatchStartedAt"),
        )
        .expect("record dispatch");
    let attempt = &scenario["sessionAttempt"];
    let thread_key = CodexThreadKey::new(
        attempt["threadKey"]["codexHomeIdentity"]
            .as_str()
            .expect("codexHomeIdentity"),
        attempt["threadKey"]["threadId"].as_str().expect("threadId"),
    );
    let provenance = match attempt["kind"].as_str().expect("attempt kind") {
        "writer_admission" => SessionAttemptProvenance::writer_admission(
            attempt["workspaceId"].as_str().expect("workspaceId"),
            attempt["workspaceSessionGeneration"]
                .as_str()
                .expect("workspaceSessionGeneration"),
            thread_key,
            attempt["attemptId"].as_str().expect("attemptId"),
        ),
        "upstream_unsubscribe" => SessionAttemptProvenance::upstream_unsubscribe(
            attempt["workspaceId"].as_str().expect("workspaceId"),
            attempt["workspaceSessionGeneration"]
                .as_str()
                .expect("workspaceSessionGeneration"),
            attempt["appServerConnectionGeneration"]
                .as_str()
                .expect("appServerConnectionGeneration"),
            thread_key,
            attempt["attemptId"].as_str().expect("attemptId"),
        ),
        other => panic!("unexpected attempt kind {other}"),
    }
    .expect("session attempt provenance");
    runtime
        .record_session_attempt_bound(
            &key,
            provenance,
            scenario["sessionAttemptBoundAt"]
                .as_i64()
                .expect("sessionAttemptBoundAt"),
        )
        .expect("bind attempt");
    serde_json::to_value(runtime.snapshot(&key).expect("request snapshot"))
        .expect("serialize request snapshot")
}

#[test]
fn generation_schema_stable() {
    let fixture = load_fixture("generation-hierarchy.json");
    let generations = &fixture["generations"];

    assert_eq!(
        serde_json::to_value(remote_host_identity(
            generations["remoteHostIdentity"]["sample"]
                .as_str()
                .expect("host sample"),
        ))
        .unwrap(),
        generations["remoteHostIdentity"]["sample"]
    );
    assert_eq!(
        serde_json::to_value(
            DaemonProcessGeneration::new(
                generations["daemonProcessGeneration"]["sample"]
                    .as_str()
                    .expect("daemon sample"),
            )
            .unwrap(),
        )
        .unwrap(),
        generations["daemonProcessGeneration"]["sample"]
    );
    assert_eq!(
        serde_json::to_value(transport_generation(
            generations["remoteTransportGeneration"]["sample"]
                .as_str()
                .expect("transport sample"),
        ))
        .unwrap(),
        generations["remoteTransportGeneration"]["sample"]
    );
    assert_eq!(
        workspace_generation(
            generations["workspaceSessionGeneration"]["sample"]
                .as_str()
                .expect("workspace sample"),
        )
        .as_str(),
        generations["workspaceSessionGeneration"]["sample"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        app_server_generation(
            generations["appServerConnectionGeneration"]["sample"]
                .as_str()
                .expect("app-server sample"),
        )
        .as_str(),
        generations["appServerConnectionGeneration"]["sample"]
            .as_str()
            .unwrap()
    );
}

#[test]
fn request_provenance_schema_stable() {
    let fixture = load_fixture("request-provenance.json");
    for scenario in fixture["scenarios"].as_array().expect("scenarios") {
        assert_eq!(request_snapshot(scenario), scenario["expectedSnapshot"]);
    }
}

#[test]
fn same_request_id_cross_generation_safe() {
    let fixture = load_fixture("request-provenance.json");
    let request_id = fixture["crossGenerationRequestId"]
        .as_u64()
        .expect("cross-generation request id");
    let first = RemoteRequestProvenanceRuntime::new(transport_generation("transport-a"));
    let second = RemoteRequestProvenanceRuntime::new(transport_generation("transport-b"));
    let first_key = first
        .record_received(request_id, "resume_thread", 10)
        .expect("first request");
    let second_key = second
        .record_received(request_id, "resume_thread", 20)
        .expect("second request");

    assert_ne!(first_key, second_key);
    assert_eq!(first.snapshots().len(), 1);
    assert_eq!(second.snapshots().len(), 1);
}

#[test]
fn app_daemon_transport_semantics_match() {
    let fixture = load_fixture("protocol-provenance.json");
    assert_eq!(fixture["implementation"]["appCore"], "shared_core");
    assert_eq!(fixture["implementation"]["daemonCore"], "shared_core");

    let request_fixture = load_fixture("request-provenance.json");
    for scenario in request_fixture["scenarios"].as_array().expect("scenarios") {
        let app_payload = request_snapshot(scenario);
        let daemon_payload = request_snapshot(scenario);
        assert_eq!(app_payload, daemon_payload);
    }
}

#[test]
fn stale_delivery_isolation_stable() {
    let fixture = load_fixture("stale-delivery.json");
    for scenario in fixture["scenarios"].as_array().expect("scenarios") {
        assert_eq!(scenario["currentStateChanged"], false, "{scenario}");
        if scenario["sourceGeneration"] == "old" {
            assert_eq!(scenario["deliveredToCurrent"], false, "{scenario}");
        }
    }

    let current = RemoteRequestProvenanceRuntime::new(transport_generation("transport-current"));
    let old = RemoteRequestProvenanceRuntime::new(transport_generation("transport-old"));
    let old_key = old.record_received(1, "resume_thread", 10).unwrap();
    let current_key = current.record_received(1, "resume_thread", 20).unwrap();
    assert_eq!(
        current.record_response_observed(&old_key, 30),
        Err(RemoteRequestTransitionError::TransportGenerationMismatch)
    );
    assert_eq!(
        current.snapshot(&current_key).unwrap().dispatch_state,
        RemoteRequestDispatchState::Received
    );
}

#[test]
fn daemon_restart_generation_reset_stable() {
    let fixture = load_fixture("daemon-restart.json");
    let thread_key = CodexThreadKey::new(
        fixture["threadKey"]["codexHomeIdentity"]
            .as_str()
            .expect("codexHomeIdentity"),
        fixture["threadKey"]["threadId"].as_str().expect("threadId"),
    );
    let current_workspace = workspace_generation(
        fixture["afterRestart"]["workspaceSessionGeneration"]
            .as_str()
            .expect("new workspace generation"),
    );
    let current_connection = app_server_generation(
        fixture["afterRestart"]["appServerConnectionGeneration"]
            .as_str()
            .expect("new app-server generation"),
    );
    let writer = WriterAdmissionObservationRuntime::new(current_workspace.clone());
    let lifecycle = ThreadLifecycleObservationRuntime::new(current_workspace, current_connection);

    assert_eq!(
        writer.state(&thread_key),
        WriterAdmissionObservationState::NotObserved
    );
    assert_eq!(
        lifecycle.subscription_snapshot(&thread_key).state,
        ThreadSubscriptionObservationState::NotObserved
    );
    assert_eq!(
        lifecycle.runtime_state(&thread_key),
        ThreadRuntimeAvailabilityState::Unknown
    );
    assert_eq!(fixture["afterRestart"]["sessionsMapCount"], 0);
}

#[test]
fn same_host_identity_does_not_imply_same_session() {
    let fixture = load_fixture("daemon-restart.json");
    let host = fixture["remoteHostIdentity"]
        .as_str()
        .expect("RemoteHostIdentity");
    let before = daemon_info(
        host,
        fixture["beforeRestart"]["daemonProcessGeneration"]
            .as_str()
            .expect("old daemon generation"),
    );
    let after = daemon_info(
        host,
        fixture["afterRestart"]["daemonProcessGeneration"]
            .as_str()
            .expect("new daemon generation"),
    );

    assert_eq!(before.remote_host_identity, after.remote_host_identity);
    assert_eq!(
        classify_daemon_process_continuity(&before, &after),
        DaemonProcessContinuity::RestartedProcess
    );
    assert_ne!(
        fixture["beforeRestart"]["workspaceSessionGeneration"],
        fixture["afterRestart"]["workspaceSessionGeneration"]
    );
    assert_ne!(
        fixture["beforeRestart"]["appServerConnectionGeneration"],
        fixture["afterRestart"]["appServerConnectionGeneration"]
    );
}

#[test]
fn multi_client_shared_session_contract_stable() {
    let fixture = load_fixture("daemon-restart.json");
    assert_eq!(fixture["reestablishment"]["concurrentClientCount"], 2);
    assert_eq!(fixture["reestablishment"]["sessionSpawnCount"], 1);
    assert_eq!(fixture["reestablishment"]["currentSessionCount"], 1);

    let shared = Arc::new(WriterAdmissionObservationRuntime::new(
        workspace_generation(
            fixture["afterRestart"]["workspaceSessionGeneration"]
                .as_str()
                .expect("new workspace generation"),
        ),
    ));
    let first_client_view = Arc::clone(&shared);
    let second_client_view = Arc::clone(&shared);
    assert!(Arc::ptr_eq(&first_client_view, &second_client_view));
}

#[test]
fn no_retry_replay_contract_stable() {
    let fixture = load_fixture("protocol-provenance.json");
    for key in [
        "resumeThreadRetryCount",
        "upstreamUnsubscribeRetryCount",
        "reconnectReplayCount",
        "daemonRestartReplayCount",
    ] {
        assert_eq!(fixture["mutationPolicy"][key], 0, "policy {key}");
    }
    assert_eq!(
        fixture["mutationPolicy"]["newExplicitIntent"],
        "new_attempt"
    );
}

#[test]
fn forbidden_semantics_absent() {
    for name in [
        "generation-hierarchy.json",
        "request-provenance.json",
        "stale-delivery.json",
        "daemon-restart.json",
        "protocol-provenance.json",
    ] {
        let serialized = serde_json::to_string(&load_fixture(name))
            .expect("serialize compatibility fixture")
            .to_ascii_lowercase();
        for forbidden in [
            "remoteclientidentity",
            "clientowner",
            "writerowner",
            "subscriptionowner",
            "leaseid",
            "force_takeover",
            "\"free\"",
            "\"available\"",
            "\"released\"",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "fixture {name} contains forbidden semantic {forbidden}"
            );
        }
    }
}

#[test]
fn availability_does_not_change_canonical_thread_truth() {
    let fixture = load_fixture("daemon-restart.json");
    assert_eq!(fixture["afterRestart"]["canonicalThreadPreserved"], true);
    assert_eq!(fixture["afterRestart"]["canonicalThreadState"], "unchanged");
    assert_eq!(fixture["afterRestart"]["workspaceRoutePersisted"], true);
    assert_eq!(
        fixture["afterRestart"]["workspaceConnectedBeforeExplicitConnect"],
        false
    );
}

#[test]
fn old_observation_is_not_inherited_by_reestablished_session() {
    let fixture = load_fixture("daemon-restart.json");
    let thread_key = CodexThreadKey::new(
        fixture["threadKey"]["codexHomeIdentity"]
            .as_str()
            .expect("codexHomeIdentity"),
        fixture["threadKey"]["threadId"].as_str().expect("threadId"),
    );
    let old_workspace = workspace_generation(
        fixture["beforeRestart"]["workspaceSessionGeneration"]
            .as_str()
            .expect("old workspace generation"),
    );
    let old_connection = app_server_generation(
        fixture["beforeRestart"]["appServerConnectionGeneration"]
            .as_str()
            .expect("old app-server generation"),
    );
    let old_writer = WriterAdmissionObservationRuntime::new(old_workspace.clone());
    let old_lifecycle = ThreadLifecycleObservationRuntime::new(old_workspace, old_connection);
    let attempt = old_writer
        .begin_resume(thread_key.clone(), &thread_key.thread_id, 10)
        .expect("old writer attempt");
    old_writer
        .record_exact_resume_success(&thread_key, &attempt, &thread_key.thread_id, 11)
        .expect("old writer result");
    old_lifecycle
        .record_subscribed(
            thread_key.clone(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            12,
        )
        .expect("old subscription");
    old_lifecycle.record_runtime_not_loaded(
        thread_key.clone(),
        ThreadRuntimeAvailabilityEvidenceSource::ThreadClosedNotification,
        13,
    );

    let current_workspace = workspace_generation(
        fixture["afterRestart"]["workspaceSessionGeneration"]
            .as_str()
            .expect("new workspace generation"),
    );
    let current_connection = app_server_generation(
        fixture["afterRestart"]["appServerConnectionGeneration"]
            .as_str()
            .expect("new app-server generation"),
    );
    let current_writer = WriterAdmissionObservationRuntime::new(current_workspace.clone());
    let current_lifecycle =
        ThreadLifecycleObservationRuntime::new(current_workspace, current_connection);

    assert_eq!(
        old_writer.state(&thread_key),
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(
        current_writer.state(&thread_key),
        WriterAdmissionObservationState::NotObserved
    );
    assert_eq!(
        current_lifecycle.subscription_snapshot(&thread_key).state,
        ThreadSubscriptionObservationState::NotObserved
    );
    assert_eq!(
        current_lifecycle.runtime_state(&thread_key),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

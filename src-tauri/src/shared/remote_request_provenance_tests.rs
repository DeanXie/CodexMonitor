use super::remote_request_provenance::{
    RemoteRequestDispatchState, RemoteRequestProvenanceRuntime, RemoteRequestTransitionError,
    RemoteTransportGeneration, SessionAttemptProvenance,
};

fn generation(value: &str) -> RemoteTransportGeneration {
    RemoteTransportGeneration::new(value).expect("valid generation")
}

#[test]
fn authenticated_transport_gets_unique_generation() {
    let first = RemoteTransportGeneration::generate();
    let second = RemoteTransportGeneration::generate();
    assert_ne!(first, second);
}

#[test]
fn same_transport_keeps_generation() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    assert_eq!(
        runtime.transport_generation(),
        runtime.transport_generation()
    );
}

#[test]
fn reconnect_creates_new_transport_generation() {
    let first = RemoteRequestProvenanceRuntime::new(RemoteTransportGeneration::generate());
    let second = RemoteRequestProvenanceRuntime::new(RemoteTransportGeneration::generate());
    assert_ne!(first.transport_generation(), second.transport_generation());
}

#[test]
fn request_id_is_scoped_by_transport_generation() {
    let first = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let second = RemoteRequestProvenanceRuntime::new(generation("transport-b"));
    let first_key = first.record_received(1, "daemon_info", 10).unwrap();
    let second_key = second.record_received(1, "daemon_info", 10).unwrap();
    assert_ne!(first_key, second_key);
}

#[test]
fn same_request_id_across_generations_does_not_collide() {
    let first = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let second = RemoteRequestProvenanceRuntime::new(generation("transport-b"));
    assert!(first.record_received(1, "list_workspaces", 10).is_ok());
    assert!(second.record_received(1, "list_workspaces", 10).is_ok());
    assert_eq!(first.snapshots().len(), 1);
    assert_eq!(second.snapshots().len(), 1);
}

#[test]
fn provenance_records_method_and_timestamp() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let key = runtime.record_received(7, "resume_thread", 42).unwrap();
    let snapshot = runtime.snapshot(&key).unwrap();
    assert_eq!(snapshot.method, "resume_thread");
    assert_eq!(snapshot.received_at, 42);
    assert_eq!(
        snapshot.dispatch_state,
        RemoteRequestDispatchState::Received
    );
}

#[test]
fn disconnect_before_dispatch_has_no_session_attempt() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let key = runtime.record_received(8, "resume_thread", 10).unwrap();
    runtime.record_transport_lost(20);
    let snapshot = runtime.snapshot(&key).unwrap();
    assert_eq!(
        snapshot.dispatch_state,
        RemoteRequestDispatchState::TransportLost
    );
    assert_eq!(snapshot.dispatch_started_at, None);
    assert_eq!(snapshot.session_attempt, None);
    assert_eq!(snapshot.transport_lost_at, Some(20));
}

#[test]
fn disconnect_after_dispatch_preserves_session_attempt_provenance() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let key = runtime.record_received(9, "resume_thread", 10).unwrap();
    runtime.record_dispatch_started(&key, 11).unwrap();
    runtime
        .record_session_attempt_bound(
            &key,
            SessionAttemptProvenance::new("workspace-a", "session-generation-a", "attempt-a")
                .unwrap(),
            12,
        )
        .unwrap();
    runtime.record_transport_lost(13);
    let snapshot = runtime.snapshot(&key).unwrap();
    assert_eq!(
        snapshot.dispatch_state,
        RemoteRequestDispatchState::TransportLost
    );
    assert_eq!(snapshot.dispatch_started_at, Some(11));
    assert_eq!(
        snapshot
            .session_attempt
            .as_ref()
            .map(|value| value.attempt_id.as_str()),
        Some("attempt-a")
    );
}

#[test]
fn old_generation_response_cannot_complete_new_generation_request() {
    let old = RemoteRequestProvenanceRuntime::new(generation("transport-old"));
    let current = RemoteRequestProvenanceRuntime::new(generation("transport-current"));
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
fn old_generation_provenance_cannot_mutate_new_generation() {
    let old = RemoteRequestProvenanceRuntime::new(generation("transport-old"));
    let current = RemoteRequestProvenanceRuntime::new(generation("transport-current"));
    let old_key = old
        .record_received(1, "thread_upstream_unsubscribe", 10)
        .unwrap();
    assert_eq!(
        current.record_dispatch_started(&old_key, 20),
        Err(RemoteRequestTransitionError::TransportGenerationMismatch)
    );
    assert!(current.snapshots().is_empty());
}

#[test]
fn user_new_intent_creates_new_provenance() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let first = runtime.record_received(1, "resume_thread", 10).unwrap();
    let second = runtime.record_received(2, "resume_thread", 20).unwrap();
    assert_ne!(first, second);
    assert_eq!(runtime.snapshots().len(), 2);
}

#[test]
fn system_replay_is_not_allowed() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    runtime.record_received(1, "resume_thread", 10).unwrap();
    assert_eq!(
        runtime.record_received(1, "resume_thread", 20),
        Err(RemoteRequestTransitionError::DuplicateRequest)
    );
}

#[test]
fn transport_generation_is_not_remote_client_identity() {
    let generation = RemoteTransportGeneration::generate();
    let serialized = serde_json::to_value(&generation).unwrap();
    assert!(serialized.is_string());
    assert!(!serialized
        .to_string()
        .to_ascii_lowercase()
        .contains("client"));
}

#[test]
fn provenance_contains_no_writer_owner_subscription_owner_or_lease() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let key = runtime.record_received(1, "resume_thread", 10).unwrap();
    let serialized = serde_json::to_string(&runtime.snapshot(&key).unwrap()).unwrap();
    let normalized = serialized.to_ascii_lowercase();
    for forbidden in ["writerowner", "subscriptionowner", "leaseid", "clientowner"] {
        assert!(
            !normalized.contains(forbidden),
            "forbidden field {forbidden}"
        );
    }
}

#[test]
fn provenance_serialization_is_transport_only_and_stable() {
    let runtime = RemoteRequestProvenanceRuntime::new(generation("transport-a"));
    let key = runtime.record_received(3, "daemon_info", 10).unwrap();
    let serialized = serde_json::to_value(runtime.snapshot(&key).unwrap()).unwrap();
    assert_eq!(serialized["transportGeneration"], "transport-a");
    assert_eq!(serialized["transportRequestId"], 3);
    assert_eq!(serialized["method"], "daemon_info");
    assert_eq!(serialized["receivedAt"], 10);
    assert_eq!(serialized["dispatchState"], "received");
    for forbidden in ["token", "requestPayload", "clientId", "owner", "lease"] {
        assert!(
            serialized.get(forbidden).is_none(),
            "forbidden field {forbidden}"
        );
    }
}

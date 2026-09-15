use super::thread_lifecycle_observation::{
    AppServerConnectionEndEvidenceKind, AppServerConnectionGeneration,
    ThreadLifecycleObservationRuntime, ThreadRuntimeAvailabilityEvidenceSource,
    ThreadRuntimeAvailabilityState, ThreadSubscriptionEvidenceSource,
    ThreadSubscriptionObservationState, ThreadSubscriptionOutcomeErrorKind,
};
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionObservationRuntime,
    WriterAdmissionObservationSnapshotState, WriterAdmissionObservationState,
};
use crate::backend::app_server::reconcile_thread_lifecycle_message;
use crate::shared::codex_identity::CodexThreadKey;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-c4", THREAD_ID)
}

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/fixtures/app-server/thread-lifecycle-reconciliation")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read lifecycle fixture"))
        .expect("parse lifecycle fixture")
}

fn runtime(workspace: &str, connection: &str) -> ThreadLifecycleObservationRuntime {
    ThreadLifecycleObservationRuntime::new(
        WorkspaceSessionGeneration::new(workspace).unwrap(),
        AppServerConnectionGeneration::new(connection).unwrap(),
    )
}

fn subscribed_runtime(workspace: &str, connection: &str) -> ThreadLifecycleObservationRuntime {
    let runtime = runtime(workspace, connection);
    runtime
        .record_subscribed(
            thread_key(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            10,
        )
        .unwrap();
    runtime
}

fn admitted_writer(workspace: &str) -> WriterAdmissionObservationRuntime {
    let runtime =
        WriterAdmissionObservationRuntime::new(WorkspaceSessionGeneration::new(workspace).unwrap());
    let key = thread_key();
    let attempt = runtime.begin_resume(key.clone(), THREAD_ID, 1).unwrap();
    runtime
        .record_exact_resume_success(&key, &attempt, THREAD_ID, 2)
        .unwrap();
    runtime
}

fn ingest(runtime: &ThreadLifecycleObservationRuntime, fixture_name: &str, observed_at: i64) {
    assert!(reconcile_thread_lifecycle_message(
        runtime,
        "codex-home-c4",
        &fixture(fixture_name),
        observed_at,
    ));
}

#[test]
fn thread_closed_marks_runtime_not_loaded() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");

    ingest(&runtime, "thread-closed.json", 20);

    let observation = runtime.runtime_observation(&thread_key()).unwrap();
    assert_eq!(
        observation.state,
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
    assert_eq!(
        observation.evidence_source,
        ThreadRuntimeAvailabilityEvidenceSource::ThreadClosedNotification
    );
}

#[test]
fn malformed_lifecycle_notification_does_not_create_empty_thread_evidence() {
    let runtime = runtime("workspace-1", "connection-1");
    let message = json!({
        "method": "thread/closed",
        "params": { "threadId": "   " }
    });

    assert!(!reconcile_thread_lifecycle_message(
        &runtime,
        "codex-home-c4",
        &message,
        20,
    ));
    assert_eq!(
        runtime.runtime_state(&CodexThreadKey::new("codex-home-c4", "")),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

#[test]
fn thread_closed_does_not_release_writer() {
    let lifecycle = subscribed_runtime("workspace-1", "connection-1");
    let writer = admitted_writer("workspace-1");

    ingest(&lifecycle, "thread-closed.json", 20);

    assert_eq!(
        writer.snapshot(&thread_key()).unwrap().state,
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn thread_closed_does_not_rewrite_unknown_unsubscribe_outcome() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::Timeout,
            "response not observed",
            30,
        )
        .unwrap();

    ingest(&runtime, "thread-closed.json", 40);

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
}

#[test]
fn delayed_closed_after_unsubscribed_preserves_both_evidence() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &json!({"result":{"status":"unsubscribed"}}), 30)
        .unwrap();

    ingest(&runtime, "thread-closed.json", 40);

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
}

#[test]
fn closed_before_response_preserves_order_independent_evidence() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();

    ingest(&runtime, "thread-closed.json", 30);
    runtime
        .record_response(&attempt, &json!({"result":{"status":"unsubscribed"}}), 40)
        .unwrap();

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
}

#[test]
fn lost_response_plus_closed_does_not_become_unsubscribed() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected,
            "connection ended after dispatch",
            30,
        )
        .unwrap();

    ingest(&runtime, "thread-closed.json", 40);

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
}

#[test]
fn reconnect_creates_new_connection_generation() {
    let previous = subscribed_runtime("workspace-1", "connection-1");

    let next = previous.for_new_app_server_connection_generation(
        AppServerConnectionGeneration::new("connection-2").unwrap(),
    );

    assert_eq!(
        next.app_server_connection_generation().as_str(),
        "connection-2"
    );
    assert_ne!(
        previous.app_server_connection_generation(),
        next.app_server_connection_generation()
    );
}

#[test]
fn reconnect_does_not_inherit_old_subscription_state() {
    let previous = subscribed_runtime("workspace-1", "connection-1");
    let next = previous.for_new_app_server_connection_generation(
        AppServerConnectionGeneration::new("connection-2").unwrap(),
    );

    assert_eq!(
        next.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::NotObserved
    );
}

#[test]
fn reconnect_does_not_retry_unknown_unsubscribe() {
    let previous = subscribed_runtime("workspace-1", "connection-1");
    let attempt = previous
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    previous
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected,
            "connection ended after dispatch",
            30,
        )
        .unwrap();

    let next = previous.for_new_app_server_connection_generation(
        AppServerConnectionGeneration::new("connection-2").unwrap(),
    );

    let snapshot = next.subscription_snapshot(&thread_key());
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::NotObserved
    );
    assert!(snapshot.attempt_id.is_none());
}

#[test]
fn two_subscribers_one_unsubscribed_other_remains() {
    let connection_a = subscribed_runtime("workspace-1", "connection-a");
    let connection_b = subscribed_runtime("workspace-1", "connection-b");
    let attempt = connection_a
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    connection_a
        .record_response(&attempt, &json!({"result":{"status":"unsubscribed"}}), 30)
        .unwrap();

    assert_eq!(
        connection_a.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(
        connection_b.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection
    );
    assert_eq!(
        connection_b.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

#[test]
fn one_unsubscribe_does_not_imply_global_unsubscribed() {
    let connection_a = subscribed_runtime("workspace-1", "connection-a");
    let connection_b = subscribed_runtime("workspace-1", "connection-b");
    let attempt = connection_a
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    connection_a
        .record_response(&attempt, &json!({"result":{"status":"notSubscribed"}}), 30)
        .unwrap();

    assert_eq!(
        connection_b.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection
    );
}

#[test]
fn transport_disconnect_does_not_become_unsubscribe_success() {
    let runtime = subscribed_runtime("workspace-1", "connection-1");
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected,
            "transport disconnected",
            30,
        )
        .unwrap();

    runtime.record_connection_ended(
        AppServerConnectionEndEvidenceKind::TransportDisconnected,
        "stdout transport ended",
        40,
    );

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(runtime.connection_end_history().len(), 1);
}

#[test]
fn not_loaded_does_not_change_writer_observation() {
    let lifecycle = subscribed_runtime("workspace-1", "connection-1");
    let writer = admitted_writer("workspace-1");

    ingest(&lifecycle, "thread-status-not-loaded.json", 20);

    assert_eq!(
        lifecycle.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
    assert_eq!(
        writer.snapshot(&thread_key()).unwrap().state,
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn workspace_generation_change_resets_all_current_models() {
    let previous_lifecycle = subscribed_runtime("workspace-1", "connection-1");
    ingest(&previous_lifecycle, "thread-status-not-loaded.json", 20);
    let previous_writer = admitted_writer("workspace-1");

    let next_lifecycle = previous_lifecycle.for_new_workspace_session_generation(
        WorkspaceSessionGeneration::new("workspace-2").unwrap(),
        AppServerConnectionGeneration::new("connection-2").unwrap(),
    );
    let next_writer = WriterAdmissionObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-2").unwrap(),
    );

    assert_eq!(
        previous_writer.snapshot(&thread_key()).unwrap().state,
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(
        next_lifecycle.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::NotObserved
    );
    assert_eq!(
        next_lifecycle.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::Unknown
    );
    assert_eq!(
        next_writer.current_snapshot(&thread_key()).state,
        WriterAdmissionObservationSnapshotState::NotObserved
    );
}

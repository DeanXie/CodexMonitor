use super::thread_lifecycle_observation::{
    AppServerConnectionGeneration, ThreadLifecycleObservationScope,
    ThreadRuntimeAvailabilityEvidenceSource, ThreadRuntimeAvailabilityState,
    ThreadRuntimeAvailabilityTracker, ThreadSubscriptionAttemptId,
    ThreadSubscriptionEvidenceSource, ThreadSubscriptionObservationState,
    ThreadSubscriptionObservationTracker, ThreadSubscriptionOutcomeErrorKind,
};
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionAttemptId, WriterAdmissionNonTransitionEvent,
    WriterAdmissionObservationState, WriterAdmissionObservationTracker,
};
use crate::shared::codex_identity::CodexThreadKey;

const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-a", THREAD_ID)
}

fn workspace_generation(value: &str) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(value).expect("valid workspace generation")
}

fn connection_generation(value: &str) -> AppServerConnectionGeneration {
    AppServerConnectionGeneration::new(value).expect("valid connection generation")
}

fn scope(workspace: &str, connection: &str) -> ThreadLifecycleObservationScope {
    ThreadLifecycleObservationScope::new(
        workspace_generation(workspace),
        connection_generation(connection),
        thread_key(),
    )
}

fn subscription_tracker() -> ThreadSubscriptionObservationTracker {
    ThreadSubscriptionObservationTracker::new(scope("workspace-1", "connection-1"))
}

fn subscribed_tracker() -> ThreadSubscriptionObservationTracker {
    let mut tracker = subscription_tracker();
    tracker
        .record_subscribed(ThreadSubscriptionEvidenceSource::ThreadResumeResponse, 100)
        .expect("direct subscription evidence");
    tracker
}

fn pending_unsubscribe_tracker() -> ThreadSubscriptionObservationTracker {
    let mut tracker = subscribed_tracker();
    tracker
        .begin_unsubscribe(
            ThreadSubscriptionAttemptId::new("unsubscribe-attempt-1").unwrap(),
            THREAD_ID,
            110,
        )
        .expect("pending unsubscribe");
    tracker
}

fn admitted_writer_tracker() -> WriterAdmissionObservationTracker {
    let mut tracker =
        WriterAdmissionObservationTracker::new(thread_key(), workspace_generation("workspace-1"));
    tracker
        .begin_resume(
            WriterAdmissionAttemptId::new("resume-attempt-1").unwrap(),
            THREAD_ID,
            90,
        )
        .unwrap();
    tracker.record_exact_resume_success(THREAD_ID, 95).unwrap();
    tracker
}

#[test]
fn new_session_generation_starts_subscription_not_observed() {
    let previous = subscribed_tracker();

    let next = previous.for_new_workspace_session_generation(
        workspace_generation("workspace-2"),
        connection_generation("connection-2"),
    );

    assert_eq!(
        next.state(),
        ThreadSubscriptionObservationState::NotObserved
    );
    assert!(next.latest_observation().is_none());
    assert_eq!(
        next.scope().workspace_session_generation().as_str(),
        "workspace-2"
    );
}

#[test]
fn new_connection_generation_does_not_inherit_subscription() {
    let previous = subscribed_tracker();

    let next =
        previous.for_new_app_server_connection_generation(connection_generation("connection-2"));

    assert_eq!(
        next.state(),
        ThreadSubscriptionObservationState::NotObserved
    );
    assert!(next.latest_observation().is_none());
    assert_eq!(
        next.scope().app_server_connection_generation().as_str(),
        "connection-2"
    );
}

#[test]
fn subscribed_is_connection_scoped() {
    let tracker = subscribed_tracker();
    let observation = tracker.latest_observation().expect("subscription evidence");

    assert_eq!(observation.scope.thread_key(), &thread_key());
    assert_eq!(
        observation.scope.workspace_session_generation().as_str(),
        "workspace-1"
    );
    assert_eq!(
        observation
            .scope
            .app_server_connection_generation()
            .as_str(),
        "connection-1"
    );
    assert_eq!(observation.observed_at, 100);
    assert_eq!(
        observation.state,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection
    );
}

#[test]
fn unsubscribe_intent_moves_to_pending() {
    let tracker = pending_unsubscribe_tracker();
    let observation = tracker.latest_observation().expect("unsubscribe evidence");

    assert_eq!(
        observation.state,
        ThreadSubscriptionObservationState::UnsubscribePending
    );
    assert_eq!(observation.observed_at, 110);
    assert_eq!(
        observation
            .attempt_id
            .as_ref()
            .map(|attempt| attempt.as_str()),
        Some("unsubscribe-attempt-1")
    );
    assert_eq!(
        observation.requested_full_thread_id.as_deref(),
        Some(THREAD_ID)
    );
    assert_eq!(
        observation.evidence.request_method,
        Some("thread/unsubscribe")
    );
}

#[test]
fn unsubscribe_success_moves_to_unsubscribed() {
    let mut tracker = pending_unsubscribe_tracker();

    tracker
        .record_unsubscribed(120)
        .expect("unsubscribed response");

    let observation = tracker.latest_observation().unwrap();
    assert_eq!(
        observation.state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(observation.evidence.response_status, Some("unsubscribed"));
}

#[test]
fn not_subscribed_response_moves_to_not_subscribed() {
    let mut tracker = pending_unsubscribe_tracker();

    tracker
        .record_not_subscribed(120)
        .expect("notSubscribed response");

    let observation = tracker.latest_observation().unwrap();
    assert_eq!(
        observation.state,
        ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection
    );
    assert_eq!(observation.evidence.response_status, Some("notSubscribed"));
}

#[test]
fn timeout_moves_to_unsubscribe_outcome_unknown() {
    let mut tracker = pending_unsubscribe_tracker();

    tracker.record_timeout("response timed out", 120).unwrap();

    assert_eq!(
        tracker.state(),
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        tracker.latest_observation().unwrap().evidence.error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::Timeout)
    );
}

#[test]
fn disconnect_moves_to_unsubscribe_outcome_unknown() {
    let mut tracker = pending_unsubscribe_tracker();

    tracker
        .record_disconnect("connection closed after dispatch", 120)
        .unwrap();

    assert_eq!(
        tracker.state(),
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        tracker.latest_observation().unwrap().evidence.error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected)
    );
}

#[test]
fn cancellation_moves_to_unsubscribe_outcome_unknown() {
    let mut tracker = pending_unsubscribe_tracker();

    tracker
        .record_cancellation("request cancelled", 120)
        .unwrap();

    assert_eq!(
        tracker.state(),
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        tracker.latest_observation().unwrap().evidence.error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::Cancellation)
    );
}

#[test]
fn unsubscribe_result_does_not_change_writer_observation() {
    let mut writer = admitted_writer_tracker();

    writer.observe_non_transition(
        WriterAdmissionNonTransitionEvent::ThreadUnsubscribedForAppServerConnection,
    );

    assert_eq!(
        writer.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn thread_closed_does_not_change_writer_observation() {
    let mut writer = admitted_writer_tracker();

    writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadClosed);

    assert_eq!(
        writer.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn not_loaded_does_not_change_writer_observation() {
    let mut writer = admitted_writer_tracker();

    writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadRuntimeNotLoaded);

    assert_eq!(
        writer.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn runtime_new_generation_starts_unknown() {
    let mut previous = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));
    previous.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadReadResponse,
        100,
    );

    let next = previous.for_new_workspace_session_generation(
        workspace_generation("workspace-2"),
        connection_generation("connection-2"),
    );

    assert_eq!(next.state(), ThreadRuntimeAvailabilityState::Unknown);
    assert!(next.latest_observation().is_none());
}

#[test]
fn runtime_new_connection_generation_starts_unknown() {
    let mut previous = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));
    previous.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadReadResponse,
        100,
    );

    let next =
        previous.for_new_app_server_connection_generation(connection_generation("connection-2"));

    assert_eq!(next.state(), ThreadRuntimeAvailabilityState::Unknown);
    assert!(next.latest_observation().is_none());
}

#[test]
fn runtime_loaded_observed() {
    let mut tracker = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));

    tracker.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadReadResponse,
        100,
    );

    let observation = tracker.latest_observation().unwrap();
    assert_eq!(
        observation.state,
        ThreadRuntimeAvailabilityState::LoadedObserved
    );
    assert_eq!(
        observation.evidence_source,
        ThreadRuntimeAvailabilityEvidenceSource::ThreadReadResponse
    );
    assert_eq!(observation.observed_at, 100);
}

#[test]
fn runtime_not_loaded_observed() {
    let mut tracker = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));
    tracker.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadReadResponse,
        100,
    );

    tracker.record_not_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadClosedNotification,
        110,
    );

    let observation = tracker.latest_observation().unwrap();
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
fn not_loaded_can_be_followed_by_direct_loaded_evidence() {
    let mut tracker = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));
    tracker.record_not_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadStatusChangedNotification,
        100,
    );

    tracker.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadResumeResponse,
        110,
    );

    assert_eq!(
        tracker.state(),
        ThreadRuntimeAvailabilityState::LoadedObserved
    );
}

#[test]
fn unsubscribe_does_not_imply_not_loaded() {
    let mut subscription = pending_unsubscribe_tracker();
    let runtime = ThreadRuntimeAvailabilityTracker::new(scope("workspace-1", "connection-1"));

    subscription.record_unsubscribed(120).unwrap();

    assert_eq!(
        subscription.state(),
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(runtime.state(), ThreadRuntimeAvailabilityState::Unknown);
}

#[test]
fn unsubscribed_state_contains_no_remote_client_owner() {
    let mut tracker = pending_unsubscribe_tracker();
    tracker.record_unsubscribed(120).unwrap();

    let value = serde_json::to_value(tracker.snapshot()).unwrap();
    let object = value.as_object().unwrap();

    assert_eq!(
        object
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "appServerConnectionGeneration",
            "attemptId",
            "evidence",
            "observedAt",
            "requestedFullThreadId",
            "state",
            "threadKey",
            "workspaceSessionGeneration",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    );
    assert!(object.get("remoteClientOwner").is_none());
}

#[test]
fn subscription_model_contains_no_writer_owner_or_lease() {
    let mut tracker = pending_unsubscribe_tracker();
    tracker.record_unsubscribed(120).unwrap();

    let value = serde_json::to_value(tracker.snapshot()).unwrap();
    let text = serde_json::to_string(&value).unwrap();

    assert!(!text.contains("writerOwner"));
    assert!(!text.contains("leaseId"));
    assert!(!text.contains("writerFree"));
}

#[test]
fn no_subscription_state_means_writer_released() {
    let states = [
        ThreadSubscriptionObservationState::NotObserved,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::UnsubscribePending,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown,
    ];

    let serialized = states
        .into_iter()
        .map(|state| serde_json::to_value(state).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        serialized,
        vec![
            serde_json::json!("not_observed"),
            serde_json::json!("subscribed_for_app_server_connection"),
            serde_json::json!("unsubscribe_pending"),
            serde_json::json!("unsubscribed_for_app_server_connection"),
            serde_json::json!("not_subscribed_for_app_server_connection"),
            serde_json::json!("unsubscribe_outcome_unknown"),
        ]
    );
}

#[test]
fn runtime_availability_state_serialization_is_independent() {
    assert_eq!(
        serde_json::to_value(ThreadRuntimeAvailabilityState::Unknown).unwrap(),
        serde_json::json!("unknown")
    );
    assert_eq!(
        serde_json::to_value(ThreadRuntimeAvailabilityState::LoadedObserved).unwrap(),
        serde_json::json!("loaded_observed")
    );
    assert_eq!(
        serde_json::to_value(ThreadRuntimeAvailabilityState::NotLoadedObserved).unwrap(),
        serde_json::json!("not_loaded_observed")
    );
}

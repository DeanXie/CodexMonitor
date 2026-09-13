use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionAttemptId, WriterAdmissionErrorKind,
    WriterAdmissionNonTransitionEvent, WriterAdmissionObservationRuntime,
    WriterAdmissionObservationState, WriterAdmissionObservationTracker,
    WriterAdmissionTransitionError,
};
use crate::shared::codex_identity::CodexThreadKey;

const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-a", THREAD_ID)
}

fn generation(value: &str) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(value).expect("valid generation")
}

fn attempt(value: &str) -> WriterAdmissionAttemptId {
    WriterAdmissionAttemptId::new(value).expect("valid attempt")
}

fn tracker() -> WriterAdmissionObservationTracker {
    WriterAdmissionObservationTracker::new(thread_key(), generation("session-generation-1"))
}

fn pending_tracker() -> WriterAdmissionObservationTracker {
    let mut tracker = tracker();
    tracker
        .begin_resume(attempt("resume-attempt-1"), THREAD_ID, 100)
        .expect("pending admission");
    tracker
}

fn admitted_tracker() -> WriterAdmissionObservationTracker {
    let mut tracker = pending_tracker();
    tracker
        .record_exact_resume_success(THREAD_ID, 110)
        .expect("admitted observation");
    tracker
}

#[test]
fn new_generation_starts_not_observed() {
    let tracker = tracker();

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::NotObserved
    );
    assert!(tracker.latest_observation().is_none());
}

#[test]
fn explicit_resume_intent_moves_to_pending() {
    let mut tracker = tracker();

    tracker
        .begin_resume(attempt("resume-attempt-1"), THREAD_ID, 100)
        .expect("pending admission");

    let observation = tracker.latest_observation().expect("resume observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionPending
    );
    assert_eq!(observation.thread_key, thread_key());
    assert_eq!(
        observation.workspace_session_generation,
        generation("session-generation-1")
    );
    assert_eq!(observation.observed_at, 100);
    assert_eq!(observation.attempt_id, attempt("resume-attempt-1"));
    assert_eq!(observation.requested_full_thread_id, THREAD_ID);
    assert_eq!(observation.evidence.request_method, "thread/resume");
}

#[test]
fn exact_resume_success_moves_to_admitted_for_session() {
    let mut tracker = pending_tracker();

    tracker
        .record_exact_resume_success(THREAD_ID, 110)
        .expect("admitted observation");

    let observation = tracker.latest_observation().expect("resume observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(
        observation.evidence.returned_full_thread_id.as_deref(),
        Some(THREAD_ID)
    );
    assert_eq!(observation.observed_at, 110);
}

#[test]
fn active_writer_error_moves_to_blocked() {
    let mut tracker = pending_tracker();

    tracker
        .record_active_writer_blocked(-32600, "already has an active writer", 110)
        .expect("blocked observation");

    let observation = tracker.latest_observation().expect("resume observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::BlockedByActiveWriter
    );
    assert_eq!(observation.evidence.upstream_error_code, Some(-32600));
    assert_eq!(
        observation.evidence.normalized_error_kind,
        Some(WriterAdmissionErrorKind::BlockedByActiveWriter)
    );
}

#[test]
fn non_active_writer_error_cannot_be_recorded_as_blocked() {
    let mut tracker = pending_tracker();

    let result =
        tracker.record_active_writer_blocked(-32601, "different upstream request failure", 110);

    assert_eq!(
        result,
        Err(WriterAdmissionTransitionError::ActiveWriterEvidenceMismatch)
    );
    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmissionPending
    );
}

#[test]
fn timeout_moves_to_outcome_unknown() {
    let mut tracker = pending_tracker();

    tracker
        .record_timeout("resume response timed out", 110)
        .expect("unknown outcome");

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(
        tracker
            .latest_observation()
            .and_then(|observation| observation.evidence.normalized_error_kind),
        Some(WriterAdmissionErrorKind::Timeout)
    );
}

#[test]
fn disconnect_during_pending_moves_to_outcome_unknown() {
    let mut tracker = pending_tracker();

    tracker
        .record_dispatch_disconnect("connection closed after dispatch", 110)
        .expect("unknown outcome");

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(
        tracker
            .latest_observation()
            .and_then(|observation| observation.evidence.normalized_error_kind),
        Some(WriterAdmissionErrorKind::DispatchDisconnected)
    );
}

#[test]
fn session_end_moves_to_release_unobserved() {
    let mut tracker = admitted_tracker();

    tracker
        .record_session_ended(&generation("session-generation-1"), 120)
        .expect("session ended observation");

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::SessionEndedReleaseUnobserved
    );
    assert!(!tracker.global_writer_free_observed());
}

#[test]
fn new_generation_does_not_inherit_prior_admission() {
    let tracker = admitted_tracker();

    let next = tracker.for_new_generation(generation("session-generation-2"));

    assert_eq!(next.thread_key(), &thread_key());
    assert_eq!(
        next.workspace_session_generation(),
        &generation("session-generation-2")
    );
    assert_eq!(next.state(), WriterAdmissionObservationState::NotObserved);
    assert!(next.latest_observation().is_none());
}

#[test]
fn thread_read_does_not_change_observation() {
    let mut tracker = admitted_tracker();
    let before = tracker.clone();

    tracker.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadRead);

    assert_eq!(tracker, before);
}

#[test]
fn turn_completed_does_not_release_writer() {
    let mut tracker = admitted_tracker();

    tracker.observe_non_transition(WriterAdmissionNonTransitionEvent::TurnCompleted);

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn remote_client_disconnect_does_not_release_writer() {
    let mut tracker = admitted_tracker();

    tracker.observe_non_transition(WriterAdmissionNonTransitionEvent::RemoteTcpDisconnected);

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn unsubscribe_does_not_release_writer() {
    let mut tracker = admitted_tracker();

    tracker.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadUnsubscribe);

    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn other_observation_only_events_do_not_change_admission() {
    let events = [
        WriterAdmissionNonTransitionEvent::TurnIdle,
        WriterAdmissionNonTransitionEvent::OrdinaryRefresh,
        WriterAdmissionNonTransitionEvent::Polling,
        WriterAdmissionNonTransitionEvent::RemoteClientClosed,
        WriterAdmissionNonTransitionEvent::MobilePageClosed,
        WriterAdmissionNonTransitionEvent::FocusLost,
    ];

    for event in events {
        let mut tracker = admitted_tracker();
        tracker.observe_non_transition(event);
        assert_eq!(
            tracker.state(),
            WriterAdmissionObservationState::AdmittedForSession
        );
    }
}

#[test]
fn blocked_and_unknown_observations_end_without_claiming_release() {
    let mut blocked = pending_tracker();
    blocked
        .record_active_writer_blocked(-32600, "already has an active writer", 110)
        .expect("blocked observation");
    blocked
        .record_session_ended(&generation("session-generation-1"), 120)
        .expect("blocked session ended");

    let mut unknown = pending_tracker();
    unknown
        .record_timeout("resume response timed out", 110)
        .expect("unknown outcome");
    unknown
        .record_session_ended(&generation("session-generation-1"), 120)
        .expect("unknown session ended");

    assert_eq!(
        blocked.state(),
        WriterAdmissionObservationState::SessionEndedReleaseUnobserved
    );
    assert_eq!(
        unknown.state(),
        WriterAdmissionObservationState::SessionEndedReleaseUnobserved
    );
    assert!(!blocked.global_writer_free_observed());
    assert!(!unknown.global_writer_free_observed());
}

#[test]
fn pending_observation_cannot_skip_to_session_ended() {
    let mut tracker = pending_tracker();

    let result = tracker.record_session_ended(&generation("session-generation-1"), 120);

    assert_eq!(
        result,
        Err(WriterAdmissionTransitionError::InvalidTransition)
    );
    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmissionPending
    );
}

#[test]
fn admitted_state_has_no_owner_identity() {
    let tracker = admitted_tracker();

    assert_eq!(tracker.writer_owner_identity(), None);
}

#[test]
fn admitted_state_has_no_lease_identity() {
    let tracker = admitted_tracker();

    assert_eq!(tracker.writer_lease_identity(), None);
}

#[test]
fn no_state_represents_global_free() {
    let mut snapshots = vec![tracker(), pending_tracker(), admitted_tracker()];

    let mut blocked = pending_tracker();
    blocked
        .record_active_writer_blocked(-32600, "already has an active writer", 110)
        .expect("blocked observation");
    snapshots.push(blocked);

    let mut unknown = pending_tracker();
    unknown
        .record_timeout("resume response timed out", 110)
        .expect("unknown outcome");
    snapshots.push(unknown);

    let mut ended = admitted_tracker();
    ended
        .record_session_ended(&generation("session-generation-1"), 120)
        .expect("session ended observation");
    snapshots.push(ended);

    assert!(snapshots
        .iter()
        .all(|snapshot| !snapshot.global_writer_free_observed()));
}

#[test]
fn mismatched_success_response_cannot_admit_the_session() {
    let mut tracker = pending_tracker();

    let result = tracker.record_exact_resume_success("different-thread", 110);

    assert_eq!(
        result,
        Err(WriterAdmissionTransitionError::ThreadIdentityMismatch)
    );
    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmissionPending
    );
}

#[test]
fn different_generation_cannot_end_existing_observation() {
    let mut tracker = admitted_tracker();

    let result = tracker.record_session_ended(&generation("session-generation-2"), 120);

    assert_eq!(
        result,
        Err(WriterAdmissionTransitionError::SessionGenerationMismatch)
    );
    assert_eq!(
        tracker.state(),
        WriterAdmissionObservationState::AdmittedForSession
    );
}

#[test]
fn resume_intent_records_pending() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));

    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");

    let observation = runtime
        .snapshot(&thread_key())
        .expect("pending observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionPending
    );
    assert_eq!(observation.attempt_id, attempt_id);
    assert_eq!(
        observation.workspace_session_generation,
        generation("session-generation-1")
    );
}

#[test]
fn exact_success_records_admitted_for_session() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");

    runtime
        .record_exact_resume_success(&thread_key(), &attempt_id, THREAD_ID, 210)
        .expect("exact success");

    let observation = runtime
        .snapshot(&thread_key())
        .expect("admitted observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(observation.evidence.exact_id_match, Some(true));
}

#[test]
fn mismatched_success_response_does_not_admit() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");

    runtime
        .record_malformed_response(
            &thread_key(),
            &attempt_id,
            Some("different-thread"),
            "resume response thread id did not match",
            210,
        )
        .expect("unknown outcome");

    let observation = runtime
        .snapshot(&thread_key())
        .expect("unknown observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(observation.evidence.exact_id_match, Some(false));
    assert_eq!(
        observation.evidence.normalized_error_kind,
        Some(WriterAdmissionErrorKind::MalformedResponse)
    );
}

#[test]
fn active_writer_error_records_blocked() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");

    runtime
        .record_active_writer_blocked(
            &thread_key(),
            &attempt_id,
            -32600,
            "already has an active writer",
            210,
        )
        .expect("blocked observation");

    assert_eq!(
        runtime.snapshot(&thread_key()).unwrap().state,
        WriterAdmissionObservationState::BlockedByActiveWriter
    );
}

#[test]
fn timeout_disconnect_and_cancellation_record_outcome_unknown() {
    let cases = [
        WriterAdmissionErrorKind::Timeout,
        WriterAdmissionErrorKind::DispatchDisconnected,
        WriterAdmissionErrorKind::Cancellation,
    ];

    for (index, kind) in cases.into_iter().enumerate() {
        let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
        let attempt_id = runtime
            .begin_resume(thread_key(), THREAD_ID, 200)
            .expect("resume intent");
        runtime
            .record_outcome_unknown(
                &thread_key(),
                &attempt_id,
                kind,
                format!("unknown outcome {index}"),
                210,
            )
            .expect("unknown observation");

        let observation = runtime
            .snapshot(&thread_key())
            .expect("unknown observation");
        assert_eq!(
            observation.state,
            WriterAdmissionObservationState::AdmissionOutcomeUnknown
        );
        assert_eq!(observation.evidence.normalized_error_kind, Some(kind));
    }
}

#[test]
fn new_session_generation_resets_runtime_observation() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");
    runtime
        .record_exact_resume_success(&thread_key(), &attempt_id, THREAD_ID, 210)
        .expect("exact success");

    let next = WriterAdmissionObservationRuntime::new(generation("session-generation-2"));

    assert_eq!(
        next.state(&thread_key()),
        WriterAdmissionObservationState::NotObserved
    );
    assert!(next.snapshot(&thread_key()).is_none());
}

#[test]
fn observation_runtime_is_session_scoped_not_client_owned() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let attempt_id = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("resume intent");
    runtime
        .record_exact_resume_success(&thread_key(), &attempt_id, THREAD_ID, 210)
        .expect("exact success");

    let observation = runtime
        .snapshot(&thread_key())
        .expect("admitted observation");
    assert_eq!(
        observation.workspace_session_generation,
        generation("session-generation-1")
    );
}

#[test]
fn each_resume_intent_receives_a_unique_attempt_id() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));

    let first = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("first resume intent");
    let second = runtime
        .begin_resume(thread_key(), THREAD_ID, 210)
        .expect("second resume intent");

    assert_ne!(first, second);
    assert_eq!(runtime.snapshot(&thread_key()).unwrap().attempt_id, second);
}

#[test]
fn concurrent_session_attempts_are_correlated_without_client_ownership() {
    let runtime = WriterAdmissionObservationRuntime::new(generation("session-generation-1"));
    let first = runtime
        .begin_resume(thread_key(), THREAD_ID, 200)
        .expect("first resume intent");
    let second = runtime
        .begin_resume(thread_key(), THREAD_ID, 210)
        .expect("second resume intent");

    runtime
        .record_exact_resume_success(&thread_key(), &first, THREAD_ID, 220)
        .expect("first attempt remains correlated");
    runtime
        .record_active_writer_blocked(
            &thread_key(),
            &second,
            -32600,
            "already has an active writer",
            230,
        )
        .expect("second attempt remains correlated");

    let latest = runtime.snapshot(&thread_key()).expect("latest observation");
    assert_eq!(latest.attempt_id, second);
    assert_eq!(
        latest.state,
        WriterAdmissionObservationState::BlockedByActiveWriter
    );
}

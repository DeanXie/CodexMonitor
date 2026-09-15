use super::creation_coordination::DispatchBoundary;
use super::thread_lifecycle_observation::{
    AppServerConnectionGeneration, ThreadLifecycleObservationRuntime,
    ThreadRuntimeAvailabilityState, ThreadSubscriptionEvidenceSource,
    ThreadSubscriptionObservationState, ThreadSubscriptionOutcomeErrorKind,
};
use super::thread_upstream_unsubscribe_core;
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionObservationRuntime, WriterAdmissionObservationState,
};
use crate::backend::app_server::{classify_unsubscribe_dispatch_error, WorkspaceSession};
use crate::shared::codex_identity::CodexThreadKey;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/fixtures/app-server/thread-unsubscribe")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read unsubscribe fixture"))
        .expect("parse unsubscribe fixture")
}

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-c3", THREAD_ID)
}

fn runtime() -> ThreadLifecycleObservationRuntime {
    ThreadLifecycleObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-c3").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-c3").unwrap(),
    )
}

fn subscribed_runtime() -> ThreadLifecycleObservationRuntime {
    let runtime = runtime();
    runtime
        .record_subscribed(
            thread_key(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            10,
        )
        .unwrap();
    runtime
}

async fn make_session() -> (Arc<WorkspaceSession>, CodexThreadKey) {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "more"]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", "cat"]);
        command
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().expect("spawn dummy app-server");
    let stdin = child.stdin.take().expect("dummy app-server stdin");
    let workspace_reconciler = crate::backend::app_server::runtime_reconciler_for_home(None);
    let key = CodexThreadKey::new(workspace_reconciler.codex_home_identity(), THREAD_ID);
    let writer_admission_observations = WriterAdmissionObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-c3").unwrap(),
    );
    let thread_lifecycle_observations = ThreadLifecycleObservationRuntime::new(
        writer_admission_observations
            .workspace_session_generation()
            .clone(),
        AppServerConnectionGeneration::new("connection-generation-c3").unwrap(),
    );
    thread_lifecycle_observations
        .record_subscribed(
            key.clone(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            10,
        )
        .unwrap();
    let writer_attempt = writer_admission_observations
        .begin_resume(key.clone(), THREAD_ID, 1)
        .unwrap();
    writer_admission_observations
        .record_exact_resume_success(&key, &writer_attempt, THREAD_ID, 2)
        .unwrap();
    let session = Arc::new(WorkspaceSession {
        codex_args: None,
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        request_context: Mutex::new(HashMap::new()),
        thread_workspace: Mutex::new(HashMap::new()),
        workspace_reconciler: Mutex::new(workspace_reconciler),
        execution_settings_evidence: Default::default(),
        projection_observations: Default::default(),
        writer_admission_observations,
        thread_lifecycle_observations,
        creation_coordinator: Mutex::new(None),
        runtime_observation_keys: Mutex::new(HashSet::new()),
        runtime_observation_clock: AtomicU64::new(0),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: "workspace-c3".to_string(),
        workspace_ids: Mutex::new(HashSet::from(["workspace-c3".to_string()])),
    });
    (session, key)
}

async fn await_pending_request(session: &WorkspaceSession) -> u64 {
    for _ in 0..100 {
        if let Some(id) = session.pending.lock().await.keys().next().copied() {
            return id;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("thread/unsubscribe request was not dispatched");
}

async fn deliver_response(session: &WorkspaceSession, request_id: u64, response: Value) {
    session
        .pending
        .lock()
        .await
        .remove(&request_id)
        .unwrap()
        .send(response)
        .unwrap();
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[test]
fn explicit_upstream_unsubscribe_records_pending() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .expect("unsubscribe intent");
    let snapshot = runtime.subscription_snapshot(&thread_key());

    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribePending
    );
    assert_eq!(
        snapshot.attempt_id.as_deref(),
        Some(attempt.attempt_id().as_str())
    );
    assert_eq!(
        snapshot.requested_full_thread_id.as_deref(),
        Some(THREAD_ID)
    );
    assert_eq!(
        snapshot.evidence.unwrap().request_method.as_deref(),
        Some("thread/unsubscribe")
    );
}

#[test]
fn unsubscribed_response_records_unsubscribed() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();

    runtime
        .record_response(&attempt, &fixture("response-unsubscribed.json"), 30)
        .unwrap();

    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

#[test]
fn not_subscribed_response_records_not_subscribed() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &fixture("response-not-subscribed.json"), 30)
        .unwrap();
    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection
    );
    assert_eq!(
        runtime
            .subscription_snapshot(&thread_key())
            .evidence
            .unwrap()
            .response_status
            .as_deref(),
        Some("notSubscribed")
    );
}

#[test]
fn not_loaded_response_records_not_subscribed() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &fixture("response-not-loaded.json"), 30)
        .unwrap();
    assert_eq!(
        runtime.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection
    );
    assert_eq!(
        runtime
            .subscription_snapshot(&thread_key())
            .evidence
            .unwrap()
            .response_status
            .as_deref(),
        Some("notLoaded")
    );
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
}

#[test]
fn not_loaded_response_updates_runtime_not_loaded() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &fixture("response-not-loaded.json"), 30)
        .unwrap();
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::NotLoadedObserved
    );
}

#[test]
fn unsubscribed_does_not_mark_runtime_not_loaded() {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &fixture("response-unsubscribed.json"), 30)
        .unwrap();
    assert_eq!(
        runtime.runtime_state(&thread_key()),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

#[test]
fn unsubscribe_does_not_change_writer_observation() {
    let writer = WriterAdmissionObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-c3").unwrap(),
    );
    let key = thread_key();
    let writer_attempt = writer.begin_resume(key.clone(), THREAD_ID, 1).unwrap();
    writer
        .record_exact_resume_success(&key, &writer_attempt, THREAD_ID, 2)
        .unwrap();
    let before = writer.current_snapshot(&key);

    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(key.clone(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_response(&attempt, &fixture("response-not-loaded.json"), 30)
        .unwrap();

    assert_eq!(writer.current_snapshot(&key), before);
}

fn assert_unknown(kind: ThreadSubscriptionOutcomeErrorKind) {
    let runtime = subscribed_runtime();
    let attempt = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();
    runtime
        .record_outcome_unknown(&attempt, kind, "diagnostic", 30)
        .unwrap();
    let snapshot = runtime.subscription_snapshot(&thread_key());
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(snapshot.evidence.unwrap().error_kind, Some(kind));
}

#[test]
fn unsubscribe_timeout_records_outcome_unknown() {
    assert_unknown(ThreadSubscriptionOutcomeErrorKind::Timeout);
}

#[test]
fn cancellation_records_outcome_unknown() {
    assert_unknown(ThreadSubscriptionOutcomeErrorKind::Cancellation);
}

#[test]
fn malformed_response_records_outcome_unknown() {
    assert_unknown(ThreadSubscriptionOutcomeErrorKind::MalformedResponse);
}

#[test]
fn stale_connection_generation_cannot_update_current_observation() {
    let old = subscribed_runtime();
    let stale_attempt = old.begin_unsubscribe(thread_key(), THREAD_ID, 20).unwrap();
    let current = ThreadLifecycleObservationRuntime::new(
        WorkspaceSessionGeneration::new("workspace-generation-c3").unwrap(),
        AppServerConnectionGeneration::new("connection-generation-next").unwrap(),
    );

    assert!(current
        .record_response(
            &stale_attempt,
            &json!({"result":{"status":"unsubscribed"}}),
            30,
        )
        .is_err());
    assert_eq!(
        current.subscription_snapshot(&thread_key()).state,
        ThreadSubscriptionObservationState::NotObserved
    );
}

#[test]
fn concurrent_attempts_are_correlated_or_fail_closed() {
    let runtime = subscribed_runtime();
    let first = runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 20)
        .unwrap();

    assert!(runtime
        .begin_unsubscribe(thread_key(), THREAD_ID, 21)
        .is_err());
    assert_eq!(
        runtime
            .subscription_snapshot(&thread_key())
            .attempt_id
            .as_deref(),
        Some(first.attempt_id().as_str())
    );
}

#[tokio::test]
async fn upstream_boundary_dispatches_once_and_maps_exact_response() {
    let (session, key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        "workspace-c3".to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            thread_upstream_unsubscribe_core(
                &sessions,
                "workspace-c3".to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    let context = session
        .request_context
        .lock()
        .await
        .get(&request_id)
        .cloned()
        .unwrap();
    let request_fixture = fixture("request.json");
    assert_eq!(context.method, request_fixture["method"]);
    assert_eq!(context.params, request_fixture["params"]);
    assert_eq!(
        session
            .thread_lifecycle_observations
            .subscription_snapshot(&key)
            .state,
        ThreadSubscriptionObservationState::UnsubscribePending
    );
    deliver_response(
        &session,
        request_id,
        json!({"id":request_id,"result":{"status":"unsubscribed"}}),
    )
    .await;
    let response = task.await.unwrap().unwrap();
    assert_eq!(
        response.pointer("/result/status"),
        Some(&json!("unsubscribed"))
    );
    assert_eq!(
        session
            .thread_lifecycle_observations
            .subscription_snapshot(&key)
            .state,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection
    );
    assert_eq!(
        session.writer_admission_observations.state(&key),
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(session.next_id.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(session.child.lock().await.try_wait().unwrap().is_none());
    stop_session(&session).await;
}

#[tokio::test]
async fn unsubscribe_does_not_end_workspace_session() {
    let (session, _) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        "workspace-c3".to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            thread_upstream_unsubscribe_core(
                &sessions,
                "workspace-c3".to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id":request_id,"result":{"status":"unsubscribed"}}),
    )
    .await;
    task.await.unwrap().unwrap();
    assert!(session.child.lock().await.try_wait().unwrap().is_none());
    assert_eq!(session.workspace_ids.lock().await.len(), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn disconnect_after_dispatch_records_outcome_unknown() {
    let (session, key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        "workspace-c3".to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            thread_upstream_unsubscribe_core(
                &sessions,
                "workspace-c3".to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    drop(session.pending.lock().await.remove(&request_id));
    assert!(task.await.unwrap().is_err());
    let snapshot = session
        .thread_lifecycle_observations
        .subscription_snapshot(&key);
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        snapshot.evidence.unwrap().error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn cancellation_after_dispatch_records_outcome_unknown() {
    let (session, key) = make_session().await;
    let boundary = DispatchBoundary::default();
    let task = {
        let session = Arc::clone(&session);
        let boundary = boundary.clone();
        tokio::spawn(async move {
            session
                .send_upstream_unsubscribe_request_for_workspace_with_boundary(
                    "workspace-c3",
                    THREAD_ID,
                    boundary,
                )
                .await
        })
    };
    for _ in 0..100 {
        if boundary.crossed() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(boundary.crossed(), "request must cross dispatch boundary");
    task.abort();
    let _ = task.await;
    let snapshot = session
        .thread_lifecycle_observations
        .subscription_snapshot(&key);
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        snapshot.evidence.unwrap().error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::Cancellation)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn cancellation_before_dispatch_restores_subscribed_observation() {
    let (session, key) = make_session().await;
    let stdin_guard = session.stdin.lock().await;
    let boundary = DispatchBoundary::default();
    let task = {
        let session = Arc::clone(&session);
        let boundary = boundary.clone();
        tokio::spawn(async move {
            session
                .send_upstream_unsubscribe_request_for_workspace_with_boundary(
                    "workspace-c3",
                    THREAD_ID,
                    boundary,
                )
                .await
        })
    };
    await_pending_request(&session).await;
    assert!(
        !boundary.crossed(),
        "stdin lock keeps request before dispatch"
    );
    task.abort();
    let _ = task.await;
    drop(stdin_guard);
    let snapshot = session
        .thread_lifecycle_observations
        .subscription_snapshot(&key);
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection
    );
    assert!(snapshot.attempt_id.is_none());
    stop_session(&session).await;
}

#[tokio::test]
async fn malformed_response_records_unknown_without_retry() {
    let (session, key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        "workspace-c3".to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            thread_upstream_unsubscribe_core(
                &sessions,
                "workspace-c3".to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(&session, request_id, json!({"id":request_id,"result":{}})).await;
    assert!(task.await.unwrap().is_err());
    let snapshot = session
        .thread_lifecycle_observations
        .subscription_snapshot(&key);
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        snapshot.evidence.unwrap().error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::MalformedResponse)
    );
    assert_eq!(session.next_id.load(std::sync::atomic::Ordering::SeqCst), 2);
    stop_session(&session).await;
}

#[tokio::test]
async fn response_with_error_and_recognized_status_is_malformed() {
    let (session, key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        "workspace-c3".to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            thread_upstream_unsubscribe_core(
                &sessions,
                "workspace-c3".to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({
            "id": request_id,
            "result": {"status": "unsubscribed"},
            "error": {"code": -32603, "message": "contradictory response"}
        }),
    )
    .await;
    assert!(task.await.unwrap().is_err());
    let snapshot = session
        .thread_lifecycle_observations
        .subscription_snapshot(&key);
    assert_eq!(
        snapshot.state,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown
    );
    assert_eq!(
        snapshot.evidence.unwrap().error_kind,
        Some(ThreadSubscriptionOutcomeErrorKind::MalformedResponse)
    );
    assert_eq!(session.next_id.load(std::sync::atomic::Ordering::SeqCst), 2);
    stop_session(&session).await;
}

#[test]
fn timeout_is_typed_without_retry_semantics() {
    assert_eq!(
        classify_unsubscribe_dispatch_error("request timed out after 300 seconds"),
        ThreadSubscriptionOutcomeErrorKind::Timeout
    );
    assert_eq!(
        classify_unsubscribe_dispatch_error("request canceled"),
        ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected
    );
}

use super::thread_lifecycle_observation::{
    AppServerConnectionGeneration, ThreadLifecycleObservationScope,
    ThreadRuntimeAvailabilityEvidenceSource, ThreadRuntimeAvailabilityState,
    ThreadRuntimeAvailabilityTracker, ThreadSubscriptionEvidenceSource,
    ThreadSubscriptionObservationState, ThreadSubscriptionObservationTracker,
};
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionObservationRuntime, WriterAdmissionObservationState,
};
use super::{thread_live_unsubscribe_core, SyntheticLiveDetachOutcome};
use crate::backend::app_server::WorkspaceSession;
use crate::shared::codex_identity::CodexThreadKey;
use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const WORKSPACE_ID: &str = "synthetic-detach-workspace";
const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn compatibility_fixture() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("app-server")
        .join("thread-lifecycle-observation")
        .join("synthetic-live-detach.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse fixture {}: {error}", path.display()))
}

fn workspace_entry() -> WorkspaceEntry {
    WorkspaceEntry {
        id: WORKSPACE_ID.to_string(),
        name: "Synthetic detach workspace".to_string(),
        path: "C:\\synthetic-detach-workspace".to_string(),
        kind: WorkspaceKind::Main,
        parent_id: None,
        worktree: None,
        settings: WorkspaceSettings::default(),
    }
}

fn workspace_generation() -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new("synthetic-detach-session-generation")
        .expect("workspace generation")
}

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-synthetic-detach", THREAD_ID)
}

fn subscribed_observation() -> ThreadSubscriptionObservationTracker {
    let scope = ThreadLifecycleObservationScope::new(
        workspace_generation(),
        AppServerConnectionGeneration::new("app-server-connection-generation")
            .expect("connection generation"),
        thread_key(),
    );
    let mut tracker = ThreadSubscriptionObservationTracker::new(scope);
    tracker
        .record_subscribed(ThreadSubscriptionEvidenceSource::ThreadResumeResponse, 10)
        .expect("direct subscription evidence");
    tracker
}

fn loaded_runtime_observation() -> ThreadRuntimeAvailabilityTracker {
    let scope = ThreadLifecycleObservationScope::new(
        workspace_generation(),
        AppServerConnectionGeneration::new("app-server-connection-generation")
            .expect("connection generation"),
        thread_key(),
    );
    let mut tracker = ThreadRuntimeAvailabilityTracker::new(scope);
    tracker.record_loaded(
        ThreadRuntimeAvailabilityEvidenceSource::ThreadResumeResponse,
        10,
    );
    tracker
}

async fn make_session() -> Arc<WorkspaceSession> {
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
    let key = thread_key();
    let writer_admission_observations =
        WriterAdmissionObservationRuntime::new(workspace_generation());
    let attempt_id = writer_admission_observations
        .begin_resume(key.clone(), THREAD_ID, 1)
        .expect("pending admission");
    writer_admission_observations
        .record_exact_resume_success(&key, &attempt_id, THREAD_ID, 2)
        .expect("admitted session evidence");

    Arc::new(WorkspaceSession {
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
        thread_lifecycle_observations: Default::default(),
        approval_observations: Default::default(),
        creation_coordinator: Mutex::new(None),
        runtime_observation_keys: Mutex::new(HashSet::new()),
        runtime_observation_clock: AtomicU64::new(0),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: WORKSPACE_ID.to_string(),
        workspace_ids: Mutex::new(HashSet::from([
            WORKSPACE_ID.to_string(),
            "shared-route-b".to_string(),
        ])),
    })
}

async fn fixture() -> (
    Mutex<HashMap<String, WorkspaceEntry>>,
    Mutex<HashMap<String, Arc<WorkspaceSession>>>,
    Arc<WorkspaceSession>,
) {
    let session = make_session().await;
    (
        Mutex::new(HashMap::from([(
            WORKSPACE_ID.to_string(),
            workspace_entry(),
        )])),
        Mutex::new(HashMap::from([(
            WORKSPACE_ID.to_string(),
            Arc::clone(&session),
        )])),
        session,
    )
}

async fn detach(
    workspaces: &Mutex<HashMap<String, WorkspaceEntry>>,
    sessions: &Mutex<HashMap<String, Arc<WorkspaceSession>>>,
) -> SyntheticLiveDetachOutcome {
    thread_live_unsubscribe_core(
        workspaces,
        sessions,
        WORKSPACE_ID.to_string(),
        THREAD_ID.to_string(),
    )
    .await
    .expect("synthetic live detach")
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[tokio::test]
async fn synthetic_detach_still_dispatches_zero_upstream_unsubscribe() {
    let (workspaces, sessions, session) = fixture().await;
    let expected = compatibility_fixture();

    let outcome = detach(&workspaces, &sessions).await;

    assert_eq!(outcome.event_method(), expected["localEvent"]);
    assert_eq!(expected["operation"], "thread_live_unsubscribe");
    assert_eq!(expected["upstreamOperation"], "thread_upstream_unsubscribe");
    assert_eq!(expected["upstreamMethod"], "thread/unsubscribe");
    assert_eq!(expected["upstreamDispatchCount"], 0);
    assert!(session.pending.lock().await.is_empty());
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_resume_thread() {
    let (workspaces, sessions, session) = fixture().await;
    detach(&workspaces, &sessions).await;
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_start_thread() {
    let (workspaces, sessions, session) = fixture().await;
    detach(&workspaces, &sessions).await;
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_start_turn() {
    let (workspaces, sessions, session) = fixture().await;
    detach(&workspaces, &sessions).await;
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_connect_workspace() {
    let (workspaces, sessions, session) = fixture().await;
    let before = sessions.lock().await.get(WORKSPACE_ID).cloned().unwrap();

    detach(&workspaces, &sessions).await;

    let after = sessions.lock().await.get(WORKSPACE_ID).cloned().unwrap();
    assert!(Arc::ptr_eq(&before, &after));
    assert_eq!(sessions.lock().await.len(), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_end_workspace_session() {
    let (workspaces, sessions, session) = fixture().await;

    detach(&workspaces, &sessions).await;

    assert!(sessions.lock().await.contains_key(WORKSPACE_ID));
    assert!(session.child.lock().await.try_wait().unwrap().is_none());
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_change_subscription_observation() {
    let (workspaces, sessions, session) = fixture().await;
    let subscription = subscribed_observation();
    let before = subscription.snapshot();

    detach(&workspaces, &sessions).await;

    assert_eq!(subscription.snapshot(), before);
    assert_eq!(
        subscription.state(),
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_change_runtime_availability() {
    let (workspaces, sessions, session) = fixture().await;
    let runtime = loaded_runtime_observation();

    detach(&workspaces, &sessions).await;

    assert_eq!(
        runtime.state(),
        ThreadRuntimeAvailabilityState::LoadedObserved
    );
    assert_eq!(runtime.latest_observation().unwrap().observed_at, 10);
    stop_session(&session).await;
}

#[tokio::test]
async fn synthetic_detach_does_not_change_writer_observation() {
    let (workspaces, sessions, session) = fixture().await;
    let before = session
        .writer_admission_observations
        .snapshot(&thread_key())
        .expect("writer admission evidence");

    detach(&workspaces, &sessions).await;

    assert_eq!(
        session
            .writer_admission_observations
            .snapshot(&thread_key()),
        Some(before)
    );
    assert_eq!(
        session.writer_admission_observations.state(&thread_key()),
        WriterAdmissionObservationState::AdmittedForSession
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn app_and_daemon_have_same_synthetic_detach_contract() {
    let (workspaces, sessions, session) = fixture().await;
    let expected = compatibility_fixture();

    let outcome = detach(&workspaces, &sessions).await;

    assert_eq!(outcome.event_method(), expected["localEvent"]);
    assert_eq!(
        outcome.event_params(),
        json!({
            "workspaceId": WORKSPACE_ID,
            "threadId": THREAD_ID,
            "reason": "manual",
        })
    );
    assert_eq!(outcome.response(), json!({ "ok": true }));
    stop_session(&session).await;
}

#[tokio::test]
async fn shared_clients_do_not_gain_client_ownership_from_detach() {
    let (workspaces, sessions, session) = fixture().await;

    let outcome = detach(&workspaces, &sessions).await;

    assert_eq!(session.workspace_ids.lock().await.len(), 2);
    let event = outcome.event_params();
    assert!(event.get("remoteClientOwner").is_none());
    assert!(event.get("subscriptionOwner").is_none());
    stop_session(&session).await;
}

#[tokio::test]
async fn missing_workspace_is_not_unsubscribed() {
    let workspaces = Mutex::new(HashMap::new());
    let sessions = Mutex::new(HashMap::new());

    let error = thread_live_unsubscribe_core(
        &workspaces,
        &sessions,
        WORKSPACE_ID.to_string(),
        THREAD_ID.to_string(),
    )
    .await
    .expect_err("missing workspace must fail before local detach");

    assert_eq!(error, "workspace not found");
}

#[tokio::test]
async fn unavailable_session_is_not_unsubscribed() {
    let workspaces = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        workspace_entry(),
    )]));
    let sessions = Mutex::new(HashMap::new());

    let error = thread_live_unsubscribe_core(
        &workspaces,
        &sessions,
        WORKSPACE_ID.to_string(),
        THREAD_ID.to_string(),
    )
    .await
    .expect_err("unavailable session must fail before local detach");

    assert_eq!(error, "workspace session unavailable");
}

#[tokio::test]
async fn duplicate_synthetic_detach_repeats_the_local_event_contract() {
    let (workspaces, sessions, session) = fixture().await;

    let first = detach(&workspaces, &sessions).await;
    let second = detach(&workspaces, &sessions).await;

    assert_eq!(first, second);
    assert_eq!(first.event_method(), "thread/live_detached");
    assert_eq!(second.event_method(), "thread/live_detached");
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

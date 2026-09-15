use super::creation_coordination::CreationCoordinator;
use super::writer_admission_observation::{
    WriterAdmissionErrorKind, WriterAdmissionObservationState,
};
use super::{read_thread_core, resume_thread_core, thread_live_unsubscribe_core};
use crate::backend::app_server::{classify_resume_dispatch_error, WorkspaceSession};
use crate::shared::codex_identity::CodexThreadKey;
use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const WORKSPACE_ID: &str = "writer-observation-workspace";
const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

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
    let thread_key = CodexThreadKey::new(workspace_reconciler.codex_home_identity(), THREAD_ID);

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
        writer_admission_observations: Default::default(),
        thread_lifecycle_observations: Default::default(),
        creation_coordinator: Mutex::new(None),
        runtime_observation_keys: Mutex::new(HashSet::new()),
        runtime_observation_clock: AtomicU64::new(0),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: WORKSPACE_ID.to_string(),
        workspace_ids: Mutex::new(HashSet::from([WORKSPACE_ID.to_string()])),
    });
    (session, thread_key)
}

async fn await_pending_request(session: &WorkspaceSession) -> u64 {
    for _ in 0..100 {
        if let Some(id) = session.pending.lock().await.keys().next().copied() {
            return id;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("resume request was not dispatched");
}

async fn deliver_response(session: &WorkspaceSession, request_id: u64, response: Value) {
    let sender = session
        .pending
        .lock()
        .await
        .remove(&request_id)
        .expect("pending request sender");
    sender.send(response).expect("deliver app-server response");
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[tokio::test]
async fn resume_boundary_records_pending_then_exact_admission() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
            )
            .await
        })
    };

    let request_id = await_pending_request(&session).await;
    assert_eq!(
        session.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::AdmissionPending
    );
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"thread": {"id": THREAD_ID}}}),
    )
    .await;

    let response = task.await.unwrap().expect("exact resume succeeds");
    assert_eq!(
        response
            .pointer("/result/thread/id")
            .and_then(Value::as_str),
        Some(THREAD_ID)
    );
    let observation = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .expect("admission observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmittedForSession
    );
    assert_eq!(observation.evidence.exact_id_match, Some(true));
    stop_session(&session).await;
}

#[tokio::test]
async fn resume_boundary_records_active_writer_block_without_absence() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
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
            "error": {
                "code": -32600,
                "message": format!("thread {THREAD_ID} already has an active writer")
            }
        }),
    )
    .await;

    let response = task.await.unwrap().expect("typed upstream error response");
    assert_eq!(
        response.pointer("/error/data/codexMonitorKind"),
        Some(&json!("BLOCKED_BY_ACTIVE_WRITER"))
    );
    assert_eq!(
        session.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::BlockedByActiveWriter
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn mismatched_resume_success_records_unknown_and_never_admits() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"thread": {"id": "different-thread"}}}),
    )
    .await;

    assert!(task.await.unwrap().is_err());
    assert_eq!(
        session.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn incomplete_resume_success_records_unknown_and_never_admits() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {}}),
    )
    .await;

    assert!(task.await.unwrap().is_err());
    let observation = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .expect("incomplete response observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(
        observation.evidence.normalized_error_kind,
        Some(WriterAdmissionErrorKind::MalformedResponse)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn thread_read_boundary_does_not_touch_writer_observation() {
    let (session, thread_key) = make_session().await;
    let attempt_id = session
        .writer_admission_observations
        .begin_resume(thread_key.clone(), THREAD_ID, 10)
        .unwrap();
    session
        .writer_admission_observations
        .record_exact_resume_success(&thread_key, &attempt_id, THREAD_ID, 20)
        .unwrap();
    let before = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .unwrap();
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            read_thread_core(&sessions, WORKSPACE_ID.to_string(), THREAD_ID.to_string()).await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"thread": {"id": THREAD_ID}}}),
    )
    .await;
    task.await.unwrap().expect("exact read succeeds");

    assert_eq!(
        session.writer_admission_observations.snapshot(&thread_key),
        Some(before)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn thread_unsubscribe_boundary_does_not_end_session_generation() {
    let (session, thread_key) = make_session().await;
    let attempt_id = session
        .writer_admission_observations
        .begin_resume(thread_key.clone(), THREAD_ID, 10)
        .unwrap();
    session
        .writer_admission_observations
        .record_exact_resume_success(&thread_key, &attempt_id, THREAD_ID, 20)
        .unwrap();
    let before = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .unwrap();
    let sessions = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )]));
    let workspaces = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        WorkspaceEntry {
            id: WORKSPACE_ID.to_string(),
            name: "Writer observation workspace".to_string(),
            path: "C:\\writer-observation-workspace".to_string(),
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings: WorkspaceSettings::default(),
        },
    )]));

    thread_live_unsubscribe_core(
        &workspaces,
        &sessions,
        WORKSPACE_ID.to_string(),
        THREAD_ID.to_string(),
    )
    .await
    .expect("synthetic live detach succeeds");

    assert_eq!(
        session.writer_admission_observations.snapshot(&thread_key),
        Some(before)
    );
    assert!(session.pending.lock().await.is_empty());
    assert_eq!(session.next_id.load(Ordering::SeqCst), 1);
    stop_session(&session).await;
}

#[tokio::test]
async fn cancellation_after_dispatch_records_outcome_unknown() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
            )
            .await
        })
    };
    await_pending_request(&session).await;

    task.abort();
    let _ = task.await;

    let observation = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .expect("cancellation observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(
        observation.evidence.normalized_error_kind,
        Some(WriterAdmissionErrorKind::Cancellation)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn response_channel_disconnect_after_dispatch_records_outcome_unknown() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let coordinator = Arc::new(CreationCoordinator::default());
    let task = {
        let sessions = Arc::clone(&sessions);
        let coordinator = Arc::clone(&coordinator);
        tokio::spawn(async move {
            resume_thread_core(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                &coordinator,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    drop(session.pending.lock().await.remove(&request_id));

    assert!(task.await.unwrap().is_err());
    let observation = session
        .writer_admission_observations
        .snapshot(&thread_key)
        .expect("disconnect observation");
    assert_eq!(
        observation.state,
        WriterAdmissionObservationState::AdmissionOutcomeUnknown
    );
    assert_eq!(
        observation.evidence.normalized_error_kind,
        Some(WriterAdmissionErrorKind::DispatchDisconnected)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn separate_workspace_sessions_receive_distinct_generations() {
    let (first, _) = make_session().await;
    let (second, _) = make_session().await;

    assert_ne!(
        first
            .writer_admission_observations
            .workspace_session_generation(),
        second
            .writer_admission_observations
            .workspace_session_generation()
    );
    stop_session(&first).await;
    stop_session(&second).await;
}

#[test]
fn timeout_and_disconnect_errors_keep_distinct_unknown_evidence() {
    assert_eq!(
        classify_resume_dispatch_error("request timed out after 300 seconds"),
        WriterAdmissionErrorKind::Timeout
    );
    assert_eq!(
        classify_resume_dispatch_error("request canceled"),
        WriterAdmissionErrorKind::DispatchDisconnected
    );
    assert_eq!(
        classify_resume_dispatch_error("broken pipe"),
        WriterAdmissionErrorKind::DispatchDisconnected
    );
}

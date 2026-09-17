use super::delete_mutation_observation::{
    DeleteMutationFailureKind, DeleteMutationObservationRuntime, DeleteMutationState,
};
use super::delete_thread_core_with_remote_context;
use super::thread_lifecycle_observation::{
    AppServerConnectionGeneration, ThreadLifecycleObservationRuntime,
};
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionObservationRuntime,
};
use crate::backend::app_server::WorkspaceSession;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::RemoteHostIdentity;
use crate::shared::remote_request_provenance::{
    RemoteRequestDispatchContext, RemoteRequestProvenanceRuntime, SessionAttemptKind,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const WORKSPACE_ID: &str = "delete-fixture-workspace";
const THREAD_ID: &str = "0199a8c0-1111-7222-8333-444455556666";

fn host() -> RemoteHostIdentity {
    RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap()
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
    let mut child = command.spawn().expect("spawn fake app-server transport");
    let stdin = child.stdin.take().expect("fake app-server stdin");
    let workspace_reconciler = crate::backend::app_server::runtime_reconciler_for_home(None);
    let thread_key = CodexThreadKey::new(workspace_reconciler.codex_home_identity(), THREAD_ID);
    let workspace_generation =
        WorkspaceSessionGeneration::new("delete-workspace-generation").unwrap();
    let app_server_generation =
        AppServerConnectionGeneration::new("delete-app-server-generation").unwrap();
    let writer_admission_observations =
        WriterAdmissionObservationRuntime::new(workspace_generation.clone());
    let thread_lifecycle_observations = ThreadLifecycleObservationRuntime::new(
        workspace_generation.clone(),
        app_server_generation.clone(),
    );
    let delete_mutation_observations =
        DeleteMutationObservationRuntime::new(workspace_generation, app_server_generation);
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
        approval_observations: Default::default(),
        delete_mutation_observations,
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
    panic!("thread/delete was not dispatched to the fake app-server");
}

async fn deliver_response(session: &WorkspaceSession, request_id: u64, response: Value) {
    session
        .pending
        .lock()
        .await
        .remove(&request_id)
        .expect("pending fake request")
        .send(response)
        .expect("deliver fake response");
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn latest(
    session: &WorkspaceSession,
    key: &CodexThreadKey,
) -> super::delete_mutation_observation::DeleteMutationObservation {
    session
        .delete_mutation_observations
        .latest_for_thread(key)
        .expect("delete observation")
}

#[tokio::test]
async fn exact_delete_dispatch_and_empty_success_confirm_once() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            delete_thread_core_with_remote_context(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                host(),
                None,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    let request = session
        .request_context
        .lock()
        .await
        .get(&request_id)
        .cloned()
        .unwrap();
    assert_eq!(request.method, "thread/delete");
    assert_eq!(request.params, json!({"threadId": THREAD_ID}));
    assert_eq!(
        latest(&session, &thread_key).state,
        DeleteMutationState::DeletePending
    );
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {}}),
    )
    .await;
    assert_eq!(task.await.unwrap().unwrap()["result"], json!({}));
    let observation = latest(&session, &thread_key);
    assert_eq!(observation.state, DeleteMutationState::DeleteConfirmed);
    assert_eq!(observation.dispatch_count, 1);
    assert_eq!(observation.retry_count, 0);
    stop_session(&session).await;
}

#[tokio::test]
async fn active_writer_delete_response_is_rejected_without_retry() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            delete_thread_core_with_remote_context(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                host(),
                None,
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
            "error": {"code": -32600, "message": "thread already has an active writer"}
        }),
    )
    .await;
    assert_eq!(task.await.unwrap().unwrap()["error"]["code"], -32600);
    let observation = latest(&session, &thread_key);
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(
        observation.failure_kind,
        Some(DeleteMutationFailureKind::BlockedByActiveWriter)
    );
    assert_eq!(observation.dispatch_count, 1);
    assert_eq!(observation.retry_count, 0);
    stop_session(&session).await;
}

#[tokio::test]
async fn malformed_delete_response_is_unknown_and_never_tombstones() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            delete_thread_core_with_remote_context(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                host(),
                None,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"ok": true}}),
    )
    .await;
    assert!(task.await.unwrap().is_err());
    let observation = latest(&session, &thread_key);
    assert_eq!(observation.state, DeleteMutationState::DeleteOutcomeUnknown);
    assert!(session
        .delete_mutation_observations
        .confirmed_tombstone(&observation.attempt_id)
        .is_none());
    stop_session(&session).await;
}

#[tokio::test]
async fn remote_transport_loss_after_dispatch_records_unknown_and_correlation() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let provenance = Arc::new(RemoteRequestProvenanceRuntime::new_authenticated_transport());
    let request_key = provenance.record_received(41, "delete_thread", 1).unwrap();
    provenance.record_dispatch_started(&request_key, 2).unwrap();
    let remote_context =
        RemoteRequestDispatchContext::new(Arc::clone(&provenance), request_key.clone());
    let task = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            delete_thread_core_with_remote_context(
                &sessions,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
                host(),
                Some(&remote_context),
            )
            .await
        })
    };
    let _request_id = await_pending_request(&session).await;
    let correlation = provenance
        .snapshot(&request_key)
        .unwrap()
        .session_attempt
        .unwrap();
    assert_eq!(correlation.kind, SessionAttemptKind::DeleteMutation);
    assert_eq!(correlation.thread_key, thread_key);
    provenance.record_transport_lost(20);
    task.abort();
    let _ = task.await;
    let observation = latest(&session, &thread_key);
    assert_eq!(observation.state, DeleteMutationState::DeleteOutcomeUnknown);
    assert_eq!(
        observation.failure_kind,
        Some(DeleteMutationFailureKind::ResponseLost)
    );
    assert_eq!(observation.dispatch_count, 1);
    assert_eq!(observation.retry_count, 0);
    stop_session(&session).await;
}

#[tokio::test]
async fn fuzzy_delete_fails_before_fake_app_server_dispatch() {
    let (session, _) = make_session().await;
    let sessions = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )]));
    let result = delete_thread_core_with_remote_context(
        &sessions,
        WORKSPACE_ID.to_string(),
        "0199a8c0".to_string(),
        host(),
        None,
    )
    .await;
    assert!(result.is_err());
    assert!(session.pending.lock().await.is_empty());
    assert_eq!(session.delete_mutation_observations.dispatch_count(), 0);
    stop_session(&session).await;
}

#[tokio::test]
async fn invalid_remote_correlation_rejects_delete_before_app_server_dispatch() {
    let (session, thread_key) = make_session().await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let provenance = Arc::new(RemoteRequestProvenanceRuntime::new_authenticated_transport());
    let request_key = provenance.record_received(42, "delete_thread", 1).unwrap();
    let remote_context = RemoteRequestDispatchContext::new(Arc::clone(&provenance), request_key);

    let result = delete_thread_core_with_remote_context(
        &sessions,
        WORKSPACE_ID.to_string(),
        THREAD_ID.to_string(),
        host(),
        Some(&remote_context),
    )
    .await;

    let pending_is_empty = session.pending.lock().await.is_empty();
    let observation = latest(&session, &thread_key);
    stop_session(&session).await;

    assert!(result.is_err());
    assert!(pending_is_empty);
    assert_eq!(observation.state, DeleteMutationState::DeleteRejected);
    assert_eq!(
        observation.failure_kind,
        Some(DeleteMutationFailureKind::DispatchDisconnected)
    );
    assert_eq!(observation.dispatch_count, 0);
    assert_eq!(observation.retry_count, 0);
}

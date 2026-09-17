use super::{
    get_authoritative_observation_snapshot_with_freshness_core, get_projection_freshness_core,
    get_writer_admission_observation_with_freshness_core, list_threads_with_freshness_core,
    read_thread_with_freshness_core,
};
use crate::backend::app_server::WorkspaceSession;
use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::projection_freshness::{
    ProjectionFreshnessCoverage, ProjectionFreshnessKey, ProjectionFreshnessQueryError,
    ProjectionFreshnessRuntime, ProjectionFreshnessStatus,
};
use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const WORKSPACE_ID: &str = "freshness-query-workspace";
const THREAD_ID: &str = "thread-fixture";

async fn session(generation: &str) -> Arc<WorkspaceSession> {
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
    let writer = super::writer_admission_observation::WriterAdmissionObservationRuntime::new(
        WorkspaceSessionGeneration::new(generation).unwrap(),
    );
    let lifecycle = super::thread_lifecycle_observation::ThreadLifecycleObservationRuntime::new(
        writer.workspace_session_generation().clone(),
        super::thread_lifecycle_observation::AppServerConnectionGeneration::new(format!(
            "{generation}-connection"
        ))
        .unwrap(),
    );
    Arc::new(WorkspaceSession {
        codex_args: None,
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        request_context: Mutex::new(HashMap::new()),
        thread_workspace: Mutex::new(HashMap::new()),
        workspace_reconciler: Mutex::new(crate::backend::app_server::runtime_reconciler_for_home(
            None,
        )),
        execution_settings_evidence: Default::default(),
        projection_observations: Default::default(),
        writer_admission_observations: writer,
        thread_lifecycle_observations: lifecycle,
        approval_observations: Default::default(),
        delete_mutation_observations: Default::default(),
        creation_coordinator: Mutex::new(None),
        runtime_observation_keys: Mutex::new(HashSet::new()),
        runtime_observation_clock: AtomicU64::new(0),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(0),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: WORKSPACE_ID.to_string(),
        workspace_ids: Mutex::new(HashSet::from([WORKSPACE_ID.to_string()])),
    })
}

async fn await_pending_request(session: &WorkspaceSession) -> u64 {
    for _ in 0..100 {
        if let Some(id) = session.pending.lock().await.keys().next().copied() {
            return id;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("request was not dispatched");
}

async fn deliver_response(session: &WorkspaceSession, request_id: u64, response: Value) {
    session
        .pending
        .lock()
        .await
        .remove(&request_id)
        .expect("pending request sender")
        .send(response)
        .expect("deliver app-server response");
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn workspace() -> WorkspaceEntry {
    WorkspaceEntry {
        id: WORKSPACE_ID.to_string(),
        name: "Freshness query".to_string(),
        path: "C:/sanitized".to_string(),
        kind: WorkspaceKind::Main,
        parent_id: None,
        worktree: None,
        settings: WorkspaceSettings::default(),
    }
}

#[tokio::test]
async fn projection_freshness_query_uses_current_session_generations_without_mutation() {
    let runtime = ProjectionFreshnessRuntime::default();
    let session = session("session-current").await;
    let workspaces = Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), workspace())]));
    let sessions = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )]));
    let next_id_before = session.next_id.load(Ordering::SeqCst);
    let evidence_before = runtime.evidence_count();

    let snapshot = get_projection_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await
    .expect("freshness query");

    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::NotHydrated)
    );
    assert_eq!(runtime.evidence_count(), evidence_before);
    assert_eq!(session.next_id.load(Ordering::SeqCst), next_id_before);
    let detail = snapshot
        .coverages
        .iter()
        .find(|coverage| coverage.coverage == ProjectionFreshnessCoverage::ThreadDetail)
        .unwrap();
    assert_eq!(
        detail
            .generations
            .workspace_session_generation
            .as_ref()
            .unwrap()
            .as_str(),
        "session-current"
    );
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[tokio::test]
async fn unavailable_session_is_distinct_from_not_hydrated() {
    let runtime = ProjectionFreshnessRuntime::default();
    let workspaces = Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), workspace())]));
    let sessions = Mutex::new(HashMap::new());

    let snapshot = get_projection_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await
    .expect("unavailable snapshot");

    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::Unavailable)
    );
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ObservationSnapshot),
        Some(ProjectionFreshnessStatus::Unavailable)
    );
}

#[tokio::test]
async fn missing_workspace_is_not_reported_as_not_hydrated() {
    let result = get_projection_freshness_core(
        &Mutex::new(HashMap::new()),
        &Mutex::new(HashMap::new()),
        &ProjectionFreshnessRuntime::default(),
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await;
    assert_eq!(
        result,
        Err(ProjectionFreshnessQueryError::WorkspaceNotFound)
    );
}

#[tokio::test]
async fn exact_thread_read_instruments_thread_detail_at_the_real_dispatch_boundary() {
    let runtime = Arc::new(ProjectionFreshnessRuntime::default());
    let session = session("session-current").await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let runtime = Arc::clone(&runtime);
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            read_thread_with_freshness_core(
                &sessions,
                &runtime,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"thread": {"id": THREAD_ID}}}),
    )
    .await;
    task.await.unwrap().expect("thread/read succeeds");

    let generations = runtime.generations_for_session(
        session
            .thread_lifecycle_observations
            .workspace_session_generation()
            .clone(),
        session
            .thread_lifecycle_observations
            .app_server_connection_generation()
            .clone(),
    );
    let key = ProjectionFreshnessKey::thread_detail(
        WORKSPACE_ID,
        crate::shared::codex_identity::CodexThreadKey::new(
            session
                .workspace_reconciler
                .lock()
                .await
                .codex_home_identity(),
            THREAD_ID,
        ),
    );
    assert_eq!(
        runtime.snapshot(&key, &generations).status,
        ProjectionFreshnessStatus::Current
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn thread_list_instruments_only_thread_catalog_at_the_real_dispatch_boundary() {
    let runtime = Arc::new(ProjectionFreshnessRuntime::default());
    let session = session("session-current").await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )])));
    let task = {
        let runtime = Arc::clone(&runtime);
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            list_threads_with_freshness_core(
                &sessions,
                &runtime,
                WORKSPACE_ID.to_string(),
                None,
                Some(20),
                None,
            )
            .await
        })
    };
    let request_id = await_pending_request(&session).await;
    deliver_response(
        &session,
        request_id,
        json!({"id": request_id, "result": {"data": []}}),
    )
    .await;
    task.await.unwrap().expect("thread/list succeeds");

    let generations = runtime.generations_for_session(
        session
            .thread_lifecycle_observations
            .workspace_session_generation()
            .clone(),
        session
            .thread_lifecycle_observations
            .app_server_connection_generation()
            .clone(),
    );
    assert_eq!(
        runtime
            .snapshot(
                &ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID),
                &generations,
            )
            .status,
        ProjectionFreshnessStatus::Current
    );
    assert_eq!(
        runtime
            .query(
                WORKSPACE_ID,
                Some(crate::shared::codex_identity::CodexThreadKey::new(
                    "home-sanitized",
                    THREAD_ID,
                )),
                &generations,
            )
            .status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::NotHydrated)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn observation_query_instruments_observation_coverage_without_dispatch() {
    let runtime = ProjectionFreshnessRuntime::default();
    let session = session("session-current").await;
    let workspaces = Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), workspace())]));
    let sessions = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )]));
    let next_id_before = session.next_id.load(Ordering::SeqCst);

    get_writer_admission_observation_with_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        THREAD_ID,
    )
    .await
    .expect("observation query succeeds");
    let snapshot = get_projection_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await
    .expect("freshness query");
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ObservationSnapshot),
        Some(ProjectionFreshnessStatus::Current)
    );
    assert_eq!(session.next_id.load(Ordering::SeqCst), next_id_before);
    stop_session(&session).await;
}

#[tokio::test]
async fn authoritative_observation_snapshot_preserves_separate_models_without_dispatch() {
    let runtime = ProjectionFreshnessRuntime::default();
    let session = session("session-authoritative-snapshot").await;
    let workspaces = Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), workspace())]));
    let sessions = Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&session),
    )]));
    let next_id_before = session.next_id.load(Ordering::SeqCst);

    let observation = get_authoritative_observation_snapshot_with_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        THREAD_ID,
    )
    .await
    .expect("authoritative observation query succeeds");

    assert_eq!(observation.workspace_id, WORKSPACE_ID);
    assert_eq!(observation.thread_key.thread_id, THREAD_ID);
    assert_eq!(
        observation.workspace_session_generation,
        "session-authoritative-snapshot"
    );
    assert_eq!(
        observation.app_server_connection_generation,
        "session-authoritative-snapshot-connection"
    );
    assert_eq!(
        observation.writer.state,
        super::writer_admission_observation::WriterAdmissionObservationSnapshotState::NotObserved
    );
    assert_eq!(
        observation.subscription.state,
        super::thread_lifecycle_observation::ThreadSubscriptionObservationState::NotObserved
    );
    assert_eq!(
        observation.runtime.state,
        super::thread_lifecycle_observation::ThreadRuntimeAvailabilityState::Unknown
    );
    assert!(observation.pending_approvals.is_empty());
    assert!(observation.approval_history.is_empty());
    assert!(observation.approval_decision_attempts.is_empty());
    assert!(observation.delete_observation.is_none());
    assert_eq!(session.next_id.load(Ordering::SeqCst), next_id_before);

    let freshness = get_projection_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await
    .expect("freshness query");
    assert_eq!(
        freshness.status(ProjectionFreshnessCoverage::ObservationSnapshot),
        Some(ProjectionFreshnessStatus::Current)
    );
    assert_eq!(
        freshness.status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::NotHydrated)
    );
    stop_session(&session).await;
}

#[tokio::test]
async fn stale_thread_read_response_cannot_be_current_for_replacement_generation() {
    let runtime = Arc::new(ProjectionFreshnessRuntime::default());
    let old_session = session("session-old").await;
    let replacement_session = session("session-new").await;
    let sessions = Arc::new(Mutex::new(HashMap::from([(
        WORKSPACE_ID.to_string(),
        Arc::clone(&old_session),
    )])));
    let task = {
        let runtime = Arc::clone(&runtime);
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            read_thread_with_freshness_core(
                &sessions,
                &runtime,
                WORKSPACE_ID.to_string(),
                THREAD_ID.to_string(),
            )
            .await
        })
    };
    let request_id = await_pending_request(&old_session).await;
    sessions
        .lock()
        .await
        .insert(WORKSPACE_ID.to_string(), Arc::clone(&replacement_session));
    deliver_response(
        &old_session,
        request_id,
        json!({"id": request_id, "result": {"thread": {"id": THREAD_ID}}}),
    )
    .await;
    task.await.unwrap().expect("old read completed");

    let workspaces = Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), workspace())]));
    let snapshot = get_projection_freshness_core(
        &workspaces,
        &sessions,
        &runtime,
        WORKSPACE_ID,
        Some(THREAD_ID),
    )
    .await
    .expect("replacement freshness query");
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::Stale)
    );
    stop_session(&old_session).await;
    stop_session(&replacement_session).await;
}

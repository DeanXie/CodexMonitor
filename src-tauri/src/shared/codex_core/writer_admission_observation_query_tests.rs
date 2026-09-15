use super::get_writer_admission_observation_core;
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionErrorKind, WriterAdmissionObservationQueryError,
    WriterAdmissionObservationRuntime, WriterAdmissionObservationSnapshotState,
};
use crate::backend::app_server::WorkspaceSession;
use crate::shared::codex_identity::CodexThreadKey;
use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

const WORKSPACE_ID: &str = "writer-observation-query-workspace";
const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

struct QueryFixture {
    workspaces: Mutex<HashMap<String, WorkspaceEntry>>,
    sessions: Mutex<HashMap<String, Arc<WorkspaceSession>>>,
    session: Arc<WorkspaceSession>,
    thread_key: CodexThreadKey,
}

impl QueryFixture {
    async fn new(generation: &str) -> Self {
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
            writer_admission_observations: WriterAdmissionObservationRuntime::new(
                WorkspaceSessionGeneration::new(generation).unwrap(),
            ),
            thread_lifecycle_observations: Default::default(),
            creation_coordinator: Mutex::new(None),
            runtime_observation_keys: Mutex::new(HashSet::new()),
            runtime_observation_clock: AtomicU64::new(0),
            hidden_thread_ids: Mutex::new(HashSet::new()),
            next_id: AtomicU64::new(0),
            background_thread_callbacks: Mutex::new(HashMap::new()),
            owner_workspace_id: WORKSPACE_ID.to_string(),
            workspace_ids: Mutex::new(HashSet::from([WORKSPACE_ID.to_string()])),
        });
        let entry = WorkspaceEntry {
            id: WORKSPACE_ID.to_string(),
            name: "Writer observation query".to_string(),
            path: "C:/synthetic".to_string(),
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings: WorkspaceSettings::default(),
        };
        Self {
            workspaces: Mutex::new(HashMap::from([(WORKSPACE_ID.to_string(), entry)])),
            sessions: Mutex::new(HashMap::from([(
                WORKSPACE_ID.to_string(),
                Arc::clone(&session),
            )])),
            session,
            thread_key,
        }
    }

    async fn query(
        &self,
    ) -> super::writer_admission_observation::WriterAdmissionObservationSnapshot {
        get_writer_admission_observation_core(
            &self.workspaces,
            &self.sessions,
            WORKSPACE_ID,
            THREAD_ID,
        )
        .await
        .expect("query succeeds")
    }

    fn begin(&self) -> super::writer_admission_observation::WriterAdmissionAttemptId {
        self.session
            .writer_admission_observations
            .begin_resume(self.thread_key.clone(), THREAD_ID, 10)
            .unwrap()
    }

    async fn stop(&self) {
        let mut child = self.session.child.lock().await;
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

#[tokio::test]
async fn app_and_daemon_return_same_admitted_snapshot() {
    let fixture = QueryFixture::new("generation-admitted").await;
    let attempt = fixture.begin();
    fixture
        .session
        .writer_admission_observations
        .record_exact_resume_success(&fixture.thread_key, &attempt, THREAD_ID, 20)
        .unwrap();

    let app_snapshot = fixture.query().await;
    let daemon_snapshot = fixture.query().await;
    assert_eq!(app_snapshot, daemon_snapshot);
    assert_eq!(
        app_snapshot.state,
        WriterAdmissionObservationSnapshotState::AdmittedForSession
    );
    assert_eq!(app_snapshot.thread_key, fixture.thread_key);
    assert_eq!(
        app_snapshot.workspace_session_generation,
        "generation-admitted"
    );
    assert_eq!(app_snapshot.attempt_id.as_deref(), Some(attempt.as_str()));
    fixture.stop().await;
}

#[tokio::test]
async fn app_and_daemon_return_same_blocked_snapshot() {
    let fixture = QueryFixture::new("generation-blocked").await;
    let attempt = fixture.begin();
    fixture
        .session
        .writer_admission_observations
        .record_active_writer_blocked(
            &fixture.thread_key,
            &attempt,
            -32600,
            "already has an active writer",
            20,
        )
        .unwrap();

    let app_snapshot = fixture.query().await;
    let daemon_snapshot = fixture.query().await;
    assert_eq!(app_snapshot, daemon_snapshot);
    assert_eq!(
        app_snapshot.state,
        WriterAdmissionObservationSnapshotState::BlockedByActiveWriter
    );
    assert_eq!(
        app_snapshot.evidence.as_ref().unwrap().upstream_error_code,
        Some(-32600)
    );
    fixture.stop().await;
}

#[tokio::test]
async fn app_and_daemon_return_same_unknown_snapshot() {
    let fixture = QueryFixture::new("generation-unknown").await;
    let attempt = fixture.begin();
    fixture
        .session
        .writer_admission_observations
        .record_outcome_unknown(
            &fixture.thread_key,
            &attempt,
            WriterAdmissionErrorKind::Timeout,
            "response timed out",
            20,
        )
        .unwrap();

    let app_snapshot = fixture.query().await;
    let daemon_snapshot = fixture.query().await;
    assert_eq!(app_snapshot, daemon_snapshot);
    assert_eq!(
        app_snapshot.state,
        WriterAdmissionObservationSnapshotState::AdmissionOutcomeUnknown
    );
    fixture.stop().await;
}

#[tokio::test]
async fn current_generation_does_not_return_old_generation_state() {
    let old = QueryFixture::new("generation-old").await;
    let attempt = old.begin();
    old.session
        .writer_admission_observations
        .record_exact_resume_success(&old.thread_key, &attempt, THREAD_ID, 20)
        .unwrap();
    let old_snapshot = old.query().await;
    assert_eq!(
        old_snapshot.state,
        WriterAdmissionObservationSnapshotState::AdmittedForSession
    );

    let replacement = QueryFixture::new("generation-new").await;
    let snapshot = replacement.query().await;
    assert_eq!(
        snapshot.state,
        WriterAdmissionObservationSnapshotState::NotObserved
    );
    assert_eq!(snapshot.workspace_session_generation, "generation-new");
    assert!(snapshot.attempt_id.is_none());
    old.stop().await;
    replacement.stop().await;
}

#[tokio::test]
async fn session_ended_history_not_reported_as_current_admitted() {
    let fixture = QueryFixture::new("generation-ended").await;
    let attempt = fixture.begin();
    fixture
        .session
        .writer_admission_observations
        .record_exact_resume_success(&fixture.thread_key, &attempt, THREAD_ID, 20)
        .unwrap();
    fixture.session.record_app_server_generation_ended(
        super::writer_admission_observation::WriterAdmissionSessionEndEvidenceKind::AppServerProcessExited,
        "fixture process exit",
    );

    let snapshot = fixture.query().await;
    assert_eq!(
        snapshot.state,
        WriterAdmissionObservationSnapshotState::SessionEndedReleaseUnobserved
    );
    assert_eq!(
        snapshot
            .session_end_evidence
            .as_ref()
            .unwrap()
            .previous_state,
        WriterAdmissionObservationSnapshotState::AdmittedForSession
    );
    fixture.stop().await;
}

#[tokio::test]
async fn not_observed_is_distinct_from_workspace_unavailable() {
    let fixture = QueryFixture::new("generation-not-observed").await;
    assert_eq!(
        fixture.query().await.state,
        WriterAdmissionObservationSnapshotState::NotObserved
    );

    fixture.sessions.lock().await.clear();
    let unavailable = get_writer_admission_observation_core(
        &fixture.workspaces,
        &fixture.sessions,
        WORKSPACE_ID,
        THREAD_ID,
    )
    .await;
    assert_eq!(
        unavailable,
        Err(WriterAdmissionObservationQueryError::WorkspaceSessionUnavailable)
    );

    let missing = get_writer_admission_observation_core(
        &Mutex::new(HashMap::new()),
        &fixture.sessions,
        WORKSPACE_ID,
        THREAD_ID,
    )
    .await;
    assert_eq!(
        missing,
        Err(WriterAdmissionObservationQueryError::WorkspaceNotFound)
    );
    fixture.stop().await;
}

#[tokio::test]
async fn shared_remote_clients_read_same_session_observation() {
    let fixture = QueryFixture::new("generation-shared-clients").await;
    let attempt = fixture.begin();
    fixture
        .session
        .writer_admission_observations
        .record_exact_resume_success(&fixture.thread_key, &attempt, THREAD_ID, 20)
        .unwrap();

    let client_a = fixture.query().await;
    let client_b = fixture.query().await;
    assert_eq!(client_a, client_b);
    let json = serde_json::to_value(client_a).unwrap();
    assert!(json.get("remoteClientOwner").is_none());
    fixture.stop().await;
}

#[tokio::test]
async fn observation_query_does_not_connect_resume_or_start_any_codex_operation() {
    let fixture = QueryFixture::new("generation-read-only").await;
    let sessions_before = fixture.sessions.lock().await.len();
    let next_id_before = fixture.session.next_id.load(Ordering::SeqCst);
    let pending_before = fixture.session.pending.lock().await.len();

    let snapshot = fixture.query().await;

    assert_eq!(
        snapshot.state,
        WriterAdmissionObservationSnapshotState::NotObserved
    );
    assert_eq!(fixture.sessions.lock().await.len(), sessions_before);
    assert_eq!(
        fixture.session.next_id.load(Ordering::SeqCst),
        next_id_before
    );
    assert_eq!(fixture.session.pending.lock().await.len(), pending_before);
    fixture.stop().await;
}

#[tokio::test]
async fn serialization_contains_no_free_released_owner_or_lease_fields() {
    let fixture = QueryFixture::new("generation-schema").await;
    let value = serde_json::to_value(fixture.query().await).unwrap();
    let object = value.as_object().expect("snapshot object");
    for forbidden in [
        "writerFree",
        "available",
        "released",
        "writerOwner",
        "leaseId",
        "remoteClientOwner",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "forbidden field {forbidden}"
        );
    }
    assert_eq!(value["state"], "not_observed");
    fixture.stop().await;
}

use super::*;
use crate::shared::codex_core::thread_lifecycle_observation::{
    ThreadRuntimeAvailabilityState, ThreadSubscriptionObservationState,
};
use crate::shared::codex_core::writer_admission_observation::WriterAdmissionObservationState;
use crate::shared::codex_identity::CodexThreadKey;
use crate::shared::remote_host_identity::{
    classify_daemon_process_continuity, DaemonProcessContinuity, RemoteDaemonInfo,
};
use crate::shared::workspaces_core;
use std::sync::atomic::{AtomicUsize, Ordering};

const THREAD_ID: &str = "01a08c05-7880-75a2-976c-2a5895b58723";

fn daemon_config(data_dir: &std::path::Path) -> DaemonConfig {
    DaemonConfig {
        listen: "127.0.0.1:0".parse().expect("listen address"),
        token: Some("fixture-token".to_string()),
        data_dir: data_dir.to_path_buf(),
    }
}

fn load_state(data_dir: &std::path::Path) -> DaemonState {
    let (events, _) = broadcast::channel(16);
    DaemonState::load(&daemon_config(data_dir), events)
        .expect("load daemon state")
}

fn parsed_daemon_info(state: &DaemonState) -> RemoteDaemonInfo {
    serde_json::from_value(state.daemon_info()).expect("typed daemon info")
}

#[test]
fn same_host_identity_does_not_imply_same_daemon_process() {
    let tmp = make_temp_dir("daemon-process-continuity");
    let first = load_state(&tmp);
    let restarted = load_state(&tmp);
    let first_info = parsed_daemon_info(&first);
    let restarted_info = parsed_daemon_info(&restarted);

    assert_eq!(
        first_info.remote_host_identity,
        restarted_info.remote_host_identity
    );
    assert_ne!(
        first_info.daemon_process_generation,
        restarted_info.daemon_process_generation
    );
    assert_eq!(
        classify_daemon_process_continuity(&first_info, &restarted_info),
        DaemonProcessContinuity::RestartedProcess
    );
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn daemon_restart_starts_with_empty_session_runtime() {
    run_async_test(async {
        let tmp = make_temp_dir("daemon-empty-restart-sessions");
        let first = load_state(&tmp);
        let entry = make_workspace_entry("restart-workspace", tmp.to_string_lossy().as_ref());
        let old_session = make_session(entry.clone());
        first
            .sessions
            .lock()
            .await
            .insert(entry.id.clone(), Arc::clone(&old_session));

        let restarted = load_state(&tmp);

        assert!(restarted.sessions.lock().await.is_empty());
        workspaces_core::kill_session_by_id(&first.sessions, &entry.id).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn reconnect_after_restart_requires_workspace_reestablishment() {
    run_async_test(async {
        let tmp = make_temp_dir("daemon-workspace-reestablishment");
        let entry = make_workspace_entry("restart-workspace", tmp.to_string_lossy().as_ref());
        write_workspaces(
            &tmp.join("workspaces.json"),
            std::slice::from_ref(&entry),
        )
        .expect("persist workspace route");
        let restarted = load_state(&tmp);

        let before = restarted.list_workspaces().await;
        assert_eq!(before.len(), 1);
        assert!(!before[0].connected);

        workspaces_core::connect_workspace_core(
            entry.id.clone(),
            &restarted.workspaces,
            &restarted.sessions,
            &restarted.app_settings,
            {
                let entry = entry.clone();
                move |_entry, _bin, _args, _home| {
                    let entry = entry.clone();
                    async move { Ok(make_session(entry)) }
                }
            },
        )
        .await
        .expect("re-establish workspace session");

        assert!(restarted.list_workspaces().await[0].connected);
        workspaces_core::kill_session_by_id(&restarted.sessions, &entry.id).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

fn old_and_reestablished_sessions() -> (Arc<WorkspaceSession>, Arc<WorkspaceSession>, CodexThreadKey) {
    let entry = make_workspace_entry("restart-workspace", "/tmp");
    let old = make_session(entry.clone());
    let current = make_session(entry);
    let thread_key = CodexThreadKey::new("codex-home-restart", THREAD_ID);
    (old, current, thread_key)
}

#[test]
fn new_workspace_session_generation_does_not_inherit_writer_observation() {
    let (old, current, thread_key) = old_and_reestablished_sessions();
    let attempt = old
        .writer_admission_observations
        .begin_resume(thread_key.clone(), THREAD_ID, 10)
        .expect("old resume attempt");
    old.writer_admission_observations
        .record_exact_resume_success(&thread_key, &attempt, THREAD_ID, 11)
        .expect("old admission");

    assert_ne!(
        old.writer_admission_observations.workspace_session_generation(),
        current
            .writer_admission_observations
            .workspace_session_generation()
    );
    assert_eq!(
        current.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::NotObserved
    );
}

#[test]
fn new_workspace_session_generation_does_not_inherit_subscription_observation() {
    let (old, current, thread_key) = old_and_reestablished_sessions();
    old.thread_lifecycle_observations
        .record_subscribed(
            thread_key.clone(),
            crate::shared::codex_core::thread_lifecycle_observation::ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            10,
        )
        .expect("old subscription evidence");

    assert_ne!(
        old.thread_lifecycle_observations.workspace_session_generation(),
        current
            .thread_lifecycle_observations
            .workspace_session_generation()
    );
    assert_eq!(
        current
            .thread_lifecycle_observations
            .subscription_snapshot(&thread_key)
            .state,
        ThreadSubscriptionObservationState::NotObserved
    );
}

#[test]
fn new_workspace_session_generation_starts_runtime_unknown() {
    let (old, current, thread_key) = old_and_reestablished_sessions();
    old.thread_lifecycle_observations.record_runtime_not_loaded(
        thread_key.clone(),
        crate::shared::codex_core::thread_lifecycle_observation::ThreadRuntimeAvailabilityEvidenceSource::ThreadUnsubscribeNotLoadedResponse,
        10,
    );

    assert_ne!(
        old.thread_lifecycle_observations.app_server_connection_generation(),
        current
            .thread_lifecycle_observations
            .app_server_connection_generation()
    );
    assert_eq!(
        current
            .thread_lifecycle_observations
            .runtime_state(&thread_key),
        ThreadRuntimeAvailabilityState::Unknown
    );
}

#[test]
fn daemon_restart_does_not_delete_canonical_thread() {
    let tmp = make_temp_dir("daemon-canonical-preserved");
    let canonical = tmp.join("canonical-thread.jsonl");
    std::fs::write(&canonical, b"{\"type\":\"session_meta\"}\n").expect("write canonical marker");
    let _first = load_state(&tmp);
    let _restarted = load_state(&tmp);

    assert_eq!(
        std::fs::read(&canonical).expect("canonical marker remains"),
        b"{\"type\":\"session_meta\"}\n"
    );
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn daemon_restart_does_not_mark_writer_free() {
    let (_old, current, thread_key) = old_and_reestablished_sessions();
    assert_eq!(
        current.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::NotObserved
    );
}

#[test]
fn daemon_restart_does_not_mark_subscription_released() {
    let (_old, current, thread_key) = old_and_reestablished_sessions();
    assert_eq!(
        current
            .thread_lifecycle_observations
            .subscription_snapshot(&thread_key)
            .state,
        ThreadSubscriptionObservationState::NotObserved
    );
}

#[test]
fn in_flight_resume_is_not_replayed_after_restart() {
    run_async_test(async {
        let tmp = make_temp_dir("daemon-resume-no-replay");
        let old = load_state(&tmp);
        let restarted = load_state(&tmp);
        let entry = make_workspace_entry("restart-workspace", tmp.to_string_lossy().as_ref());
        let old_session = make_session(entry.clone());
        old_session.next_id.store(41, Ordering::SeqCst);
        old.sessions
            .lock()
            .await
            .insert(entry.id.clone(), old_session);

        assert!(restarted.sessions.lock().await.is_empty());
        assert_eq!(restarted.sessions.lock().await.len(), 0);
        workspaces_core::kill_session_by_id(&old.sessions, &entry.id).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn in_flight_unsubscribe_is_not_replayed_after_restart() {
    run_async_test(async {
        let tmp = make_temp_dir("daemon-unsubscribe-no-replay");
        let old = load_state(&tmp);
        let restarted = load_state(&tmp);
        let entry = make_workspace_entry("restart-workspace", tmp.to_string_lossy().as_ref());
        let old_session = make_session(entry.clone());
        old_session.next_id.store(7, Ordering::SeqCst);
        old.sessions
            .lock()
            .await
            .insert(entry.id.clone(), old_session);

        assert!(restarted.sessions.lock().await.is_empty());
        workspaces_core::kill_session_by_id(&old.sessions, &entry.id).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn old_daemon_evidence_cannot_mutate_new_daemon_state() {
    let (old, current, thread_key) = old_and_reestablished_sessions();
    old.record_app_server_transport_disconnected("old daemon transport ended");

    assert!(current
        .thread_lifecycle_observations
        .connection_end_history()
        .is_empty());
    assert_eq!(
        current.writer_admission_observations.state(&thread_key),
        WriterAdmissionObservationState::NotObserved
    );
}

#[test]
fn same_persisted_host_identity_still_revalidates_daemon_info() {
    let tmp = make_temp_dir("daemon-revalidate-info");
    let first = parsed_daemon_info(&load_state(&tmp));
    let restarted = parsed_daemon_info(&load_state(&tmp));

    assert!(crate::shared::remote_host_identity::validate_daemon_info(&first).is_ok());
    assert!(crate::shared::remote_host_identity::validate_daemon_info(&restarted).is_ok());
    assert_eq!(first.remote_host_identity, restarted.remote_host_identity);
    assert_ne!(
        first.daemon_process_generation,
        restarted.daemon_process_generation
    );
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn host_identity_rotation_remains_fail_closed() {
    let tmp_a = make_temp_dir("daemon-host-a");
    let tmp_b = make_temp_dir("daemon-host-b");
    let first = parsed_daemon_info(&load_state(&tmp_a));
    let different_host = parsed_daemon_info(&load_state(&tmp_b));

    assert_ne!(first.remote_host_identity, different_host.remote_host_identity);
    assert_eq!(
        classify_daemon_process_continuity(&first, &different_host),
        DaemonProcessContinuity::HostIdentityMismatch
    );
    let _ = std::fs::remove_dir_all(tmp_a);
    let _ = std::fs::remove_dir_all(tmp_b);
}

#[test]
fn persisted_route_does_not_equal_connected_session() {
    run_async_test(async {
        let tmp = make_temp_dir("daemon-persisted-route");
        let entry = make_workspace_entry("persisted-workspace", tmp.to_string_lossy().as_ref());
        write_workspaces(
            &tmp.join("workspaces.json"),
            std::slice::from_ref(&entry),
        )
        .expect("persist workspace route");
        let restarted = load_state(&tmp);

        let workspaces = restarted.list_workspaces().await;
        assert_eq!(workspaces.len(), 1);
        assert!(!workspaces[0].connected);
        assert!(restarted.sessions.lock().await.is_empty());
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn concurrent_clients_after_restart_share_reestablished_session() {
    run_async_test(async {
        let entry = make_workspace_entry("restart-workspace", "/tmp");
        let workspaces = Arc::new(Mutex::new(HashMap::from([(
            entry.id.clone(),
            entry.clone(),
        )])));
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let settings = Arc::new(Mutex::new(AppSettings::default()));
        let spawn_count = Arc::new(AtomicUsize::new(0));

        let connect = |workspaces: Arc<Mutex<HashMap<String, WorkspaceEntry>>>,
                       sessions: Arc<Mutex<HashMap<String, Arc<WorkspaceSession>>>>,
                       settings: Arc<Mutex<AppSettings>>,
                       spawn_count: Arc<AtomicUsize>,
                       entry: WorkspaceEntry| async move {
            workspaces_core::connect_workspace_core(
                entry.id.clone(),
                &workspaces,
                &sessions,
                &settings,
                move |_entry, _bin, _args, _home| {
                    let spawn_count = Arc::clone(&spawn_count);
                    let entry = entry.clone();
                    async move {
                        spawn_count.fetch_add(1, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                        Ok(make_session(entry))
                    }
                },
            )
            .await
        };

        let first = tokio::spawn(connect(
            Arc::clone(&workspaces),
            Arc::clone(&sessions),
            Arc::clone(&settings),
            Arc::clone(&spawn_count),
            entry.clone(),
        ));
        let second = tokio::spawn(connect(
            Arc::clone(&workspaces),
            Arc::clone(&sessions),
            Arc::clone(&settings),
            Arc::clone(&spawn_count),
            entry.clone(),
        ));

        first.await.expect("first join").expect("first connect");
        second.await.expect("second join").expect("second connect");
        assert_eq!(spawn_count.load(Ordering::SeqCst), 1);
        assert_eq!(sessions.lock().await.len(), 1);
        workspaces_core::kill_session_by_id(&sessions, &entry.id).await;
    });
}

use super::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use super::codex_identity::CodexThreadKey;
use super::projection_freshness::{
    record_authoritative_read_outcome, ProjectionFreshnessCoverage,
    ProjectionFreshnessGenerationVector, ProjectionFreshnessKey, ProjectionFreshnessRuntime,
    ProjectionFreshnessSource, ProjectionFreshnessStatus,
};
use super::remote_host_identity::DaemonProcessGeneration;
use super::remote_request_provenance::RemoteTransportGeneration;

const WORKSPACE_ID: &str = "workspace-fixture";
const THREAD_ID: &str = "thread-fixture";

fn thread_key() -> CodexThreadKey {
    CodexThreadKey::new("codex-home-fixture", THREAD_ID)
}

fn generations(
    daemon: &str,
    transport: Option<&str>,
    workspace: &str,
    app_server: &str,
) -> ProjectionFreshnessGenerationVector {
    ProjectionFreshnessGenerationVector {
        daemon_process_generation: Some(DaemonProcessGeneration::new(daemon).unwrap()),
        remote_transport_generation: transport
            .map(|value| RemoteTransportGeneration::new(value).unwrap()),
        workspace_session_generation: Some(WorkspaceSessionGeneration::new(workspace).unwrap()),
        app_server_connection_generation: Some(
            AppServerConnectionGeneration::new(app_server).unwrap(),
        ),
    }
}

fn session_generations(workspace: &str, app_server: &str) -> ProjectionFreshnessGenerationVector {
    generations("daemon-a", None, workspace, app_server)
}

fn assert_no_absence_claim(snapshot: &impl serde::Serialize) {
    let json = serde_json::to_value(snapshot).unwrap();
    let rendered = serde_json::to_string(&json).unwrap().to_ascii_lowercase();
    assert!(!rendered.contains("absent"));
    assert!(!rendered.contains("deleted"));
}

#[test]
fn projection_freshness_initially_not_hydrated() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    let snapshot = runtime.snapshot(&key, &session_generations("session-a", "connection-a"));

    assert_eq!(snapshot.status, ProjectionFreshnessStatus::NotHydrated);
    assert_eq!(
        snapshot.coverage,
        ProjectionFreshnessCoverage::ThreadCatalog
    );
    assert!(snapshot.source.is_none());
    assert!(snapshot.hydrated_at.is_none());
}

#[test]
fn hydrating_is_not_current() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_hydrating(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadList,
            10,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Hydrating);
    assert_ne!(snapshot.status, ProjectionFreshnessStatus::Current);
}

#[test]
fn authoritative_thread_list_can_mark_thread_catalog_current() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_current(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadList,
            10,
            11,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Current);
    assert_eq!(snapshot.source, Some(ProjectionFreshnessSource::ThreadList));
}

#[test]
fn thread_list_current_does_not_mark_thread_detail_current() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_current(
            ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID),
            current.clone(),
            ProjectionFreshnessSource::ThreadList,
            10,
            10,
        )
        .unwrap();

    let detail = runtime.snapshot(
        &ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key()),
        &current,
    );
    assert_eq!(detail.status, ProjectionFreshnessStatus::NotHydrated);
}

#[test]
fn authoritative_thread_read_can_mark_exact_thread_detail_current() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    runtime
        .record_current(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadRead,
            20,
            20,
        )
        .unwrap();

    assert_eq!(
        runtime.snapshot(&key, &current).status,
        ProjectionFreshnessStatus::Current
    );
}

#[test]
fn observation_query_can_mark_observation_coverage_current() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let key = ProjectionFreshnessKey::observation_snapshot(WORKSPACE_ID, thread_key());
    runtime
        .record_current(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ObservationQuery,
            30,
            30,
        )
        .unwrap();

    assert_eq!(
        runtime.snapshot(&key, &current).status,
        ProjectionFreshnessStatus::Current
    );
}

#[test]
fn workspace_generation_change_invalidates_session_scoped_current_projection() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    let old = session_generations("session-old", "connection-a");
    runtime
        .record_current(
            key.clone(),
            old,
            ProjectionFreshnessSource::ThreadList,
            10,
            10,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &session_generations("session-new", "connection-b"));
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Stale);
}

#[test]
fn app_server_generation_change_invalidates_connection_scoped_projection() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    runtime
        .record_current(
            key.clone(),
            session_generations("session-a", "connection-old"),
            ProjectionFreshnessSource::ThreadRead,
            10,
            10,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &session_generations("session-a", "connection-new"));
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Stale);
}

#[test]
fn transport_reconnect_does_not_imply_thread_absence() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    let direct_session_evidence = generations("daemon-a", None, "session-a", "connection-a");
    runtime
        .record_current(
            key.clone(),
            direct_session_evidence,
            ProjectionFreshnessSource::ThreadRead,
            10,
            10,
        )
        .unwrap();
    let after_reconnect = generations(
        "daemon-a",
        Some("transport-new"),
        "session-a",
        "connection-a",
    );

    let snapshot = runtime.snapshot(&key, &after_reconnect);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Current);
    assert_no_absence_claim(&snapshot);
}

#[test]
fn daemon_process_change_does_not_inherit_current_freshness() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::workspace_catalog();
    let old = generations("daemon-old", None, "session-a", "connection-a");
    runtime
        .record_current(
            key.clone(),
            old,
            ProjectionFreshnessSource::WorkspaceList,
            10,
            10,
        )
        .unwrap();

    let snapshot = runtime.snapshot(
        &key,
        &generations("daemon-new", None, "session-a", "connection-a"),
    );
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Stale);
}

#[test]
fn stale_snapshot_retains_original_generation() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    let old = session_generations("session-old", "connection-old");
    runtime
        .record_current(
            key.clone(),
            old.clone(),
            ProjectionFreshnessSource::ThreadRead,
            10,
            10,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &session_generations("session-new", "connection-new"));
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Stale);
    assert_eq!(snapshot.generations, old);
}

#[test]
fn historical_projection_cannot_become_current_without_new_authoritative_evidence() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    runtime
        .record_current(
            key.clone(),
            session_generations("session-old", "connection-old"),
            ProjectionFreshnessSource::ThreadRead,
            10,
            10,
        )
        .unwrap();
    let current = session_generations("session-new", "connection-new");

    assert_eq!(
        runtime.snapshot(&key, &current).status,
        ProjectionFreshnessStatus::Stale
    );
    assert_eq!(
        runtime.snapshot(&key, &current).status,
        ProjectionFreshnessStatus::Stale
    );
}

#[test]
fn unavailable_does_not_mark_thread_absent() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_unavailable(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadRead,
            10,
        )
        .unwrap();
    let snapshot = runtime.snapshot(&key, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Unavailable);
    assert_no_absence_claim(&snapshot);
}

#[test]
fn unknown_does_not_mark_thread_absent() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_unknown(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadRead,
            10,
        )
        .unwrap();
    let snapshot = runtime.snapshot(&key, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Unknown);
    assert_no_absence_claim(&snapshot);
}

#[test]
fn partial_coverage_is_supported() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    runtime
        .record_current(
            ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID),
            current.clone(),
            ProjectionFreshnessSource::ThreadList,
            10,
            10,
        )
        .unwrap();

    let snapshot = runtime.query(WORKSPACE_ID, Some(thread_key()), &current);
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ThreadCatalog),
        Some(ProjectionFreshnessStatus::Current)
    );
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ThreadDetail),
        Some(ProjectionFreshnessStatus::NotHydrated)
    );
    assert_eq!(
        snapshot.status(ProjectionFreshnessCoverage::ObservationSnapshot),
        Some(ProjectionFreshnessStatus::NotHydrated)
    );
}

#[test]
fn current_status_is_coverage_scoped() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let catalog = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    let detail = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    runtime
        .record_current(
            catalog.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadList,
            10,
            10,
        )
        .unwrap();

    assert_eq!(
        runtime.snapshot(&catalog, &current).status,
        ProjectionFreshnessStatus::Current
    );
    assert_eq!(
        runtime.snapshot(&detail, &current).status,
        ProjectionFreshnessStatus::NotHydrated
    );
}

#[test]
fn timestamp_order_does_not_override_generation_authority() {
    let runtime = ProjectionFreshnessRuntime::default();
    let key = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    let current = session_generations("session-current", "connection-current");
    runtime
        .record_current(
            key.clone(),
            current.clone(),
            ProjectionFreshnessSource::ThreadRead,
            10,
            10,
        )
        .unwrap();
    runtime
        .record_current(
            key.clone(),
            session_generations("session-old", "connection-old"),
            ProjectionFreshnessSource::ThreadRead,
            999,
            999,
        )
        .unwrap();

    let snapshot = runtime.snapshot(&key, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Current);
    assert_eq!(snapshot.observed_at, Some(10));
}

#[test]
fn app_and_daemon_freshness_schema_match() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let app = runtime.query(WORKSPACE_ID, Some(thread_key()), &current);
    let daemon = runtime.query(WORKSPACE_ID, Some(thread_key()), &current);
    assert_eq!(
        serde_json::to_value(app).unwrap(),
        serde_json::to_value(daemon).unwrap()
    );
}

#[test]
fn freshness_query_is_read_only() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let before = runtime.evidence_count();
    let _ = runtime.query(WORKSPACE_ID, Some(thread_key()), &current);
    assert_eq!(runtime.evidence_count(), before);
}

#[test]
fn freshness_model_contains_no_recovery_generation() {
    let runtime = ProjectionFreshnessRuntime::default();
    let json = serde_json::to_string(&runtime.query(
        WORKSPACE_ID,
        Some(thread_key()),
        &session_generations("session-a", "connection-a"),
    ))
    .unwrap();
    assert!(!json.contains("recoveryGeneration"));
    assert!(!json.contains("projectionGeneration"));
    assert!(!json.contains("clientGeneration"));
}

#[test]
fn freshness_model_contains_no_remote_client_identity() {
    let runtime = ProjectionFreshnessRuntime::default();
    let json = serde_json::to_string(&runtime.query(
        WORKSPACE_ID,
        Some(thread_key()),
        &session_generations("session-a", "connection-a"),
    ))
    .unwrap();
    assert!(!json.contains("remoteClient"));
    assert!(!json.contains("clientIdentity"));
}

#[test]
fn freshness_model_contains_no_owner_or_lease() {
    let runtime = ProjectionFreshnessRuntime::default();
    let json = serde_json::to_string(&runtime.query(
        WORKSPACE_ID,
        Some(thread_key()),
        &session_generations("session-a", "connection-a"),
    ))
    .unwrap()
    .to_ascii_lowercase();
    assert!(!json.contains("owner"));
    assert!(!json.contains("lease"));
    assert!(!json.contains("available"));
    assert!(!json.contains("released"));
    assert!(!json.contains("free"));
}

#[test]
fn authoritative_read_outcome_instruments_only_requested_coverage() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let catalog = ProjectionFreshnessKey::thread_catalog(WORKSPACE_ID);
    record_authoritative_read_outcome(
        &runtime,
        catalog.clone(),
        current.clone(),
        ProjectionFreshnessSource::ThreadList,
        &Ok(()),
        40,
    );

    assert_eq!(
        runtime.snapshot(&catalog, &current).status,
        ProjectionFreshnessStatus::Current
    );
    assert_eq!(
        runtime
            .snapshot(
                &ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key()),
                &current,
            )
            .status,
        ProjectionFreshnessStatus::NotHydrated
    );
}

#[test]
fn failed_authoritative_read_records_unavailable_without_absence() {
    let runtime = ProjectionFreshnessRuntime::default();
    let current = session_generations("session-a", "connection-a");
    let detail = ProjectionFreshnessKey::thread_detail(WORKSPACE_ID, thread_key());
    record_authoritative_read_outcome(
        &runtime,
        detail.clone(),
        current.clone(),
        ProjectionFreshnessSource::ThreadRead,
        &Err("transport unavailable".to_string()),
        50,
    );

    let snapshot = runtime.snapshot(&detail, &current);
    assert_eq!(snapshot.status, ProjectionFreshnessStatus::Unavailable);
    assert_no_absence_claim(&snapshot);
}

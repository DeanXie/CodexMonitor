use super::deletion_tombstone::{
    DeletionTombstone, DeletionTombstoneDocument, DeletionTombstoneStore,
};
use super::desktop_metadata::{
    DesktopCatalogEntry, DesktopMetadataPaths, DesktopMetadataReader, DesktopMetadataSnapshot,
};
use super::desktop_projection::{
    assess_desktop_projection, classify_producer_surface, resolve_workspace_assignment,
    AuthorityPresence, DesktopProjectionEvidence, DesktopProjectionState, ProducerSurface,
    ProducerSurfaceInput, ThreadReadStatus, WorkspaceAssignmentState, WorkspaceMappingInput,
    WorkspaceRoot,
};
use super::rollout_discovery::CodexHomeSource;
use super::rollout_identity::CodexThreadKey;
use super::rollout_watcher::{RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy};
use super::source_envelope::CodexHomeIdentity;
use super::source_envelope::{EvidenceConfidence, SourceKind};
use super::source_registry::{ExternalLifecycle, SourceAuthorityRegistry, SourceLaneUpdate};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn key(thread_id: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home:fixture", thread_id)
}

fn near_live_update(thread_id: &str) -> SourceLaneUpdate {
    SourceLaneUpdate {
        observation_id: format!("rollout:{thread_id}"),
        thread_key: key(thread_id),
        turn_key: None,
        source_kind: SourceKind::CodexCliRollout,
        temporal_class: super::source_envelope::SourceTemporalClass::NearLive,
        source_instance_id: "rollout:fixture".to_string(),
        source_generation: "generation:1".to_string(),
        source_timestamp_ms: Some(1_000),
        observed_timestamp_ms: 1_010,
        freshness: super::source_envelope::FreshnessEvidence {
            state: super::source_envelope::FreshnessState::Fresh,
            last_complete_record_observed_at_ms: Some(1_010),
            reason: "fixture".to_string(),
        },
        lifecycle: Some(ExternalLifecycle::Running),
        observed_model: None,
        token_snapshot: None,
    }
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("codex-monitor-slice2-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).expect("temp root");
    root
}

fn watcher_config(root: &Path) -> RolloutWatcherConfig {
    RolloutWatcherConfig {
        homes: vec![CodexHomeSource {
            codex_home: CodexHomeIdentity {
                normalized_path: root.to_string_lossy().to_string(),
                identity: "codex-home:fixture".to_string(),
            },
            root: root.to_path_buf(),
        }],
        checkpoint_path: root.join("checkpoint.json"),
        deletion_tombstones_path: root.join("deletion-tombstones.json"),
        retry: WatcherRetryPolicy {
            max_attempts: 1,
            initial_backoff_ms: 0,
        },
        fresh_window_ms: 5_000,
        settled_after_ms: 10_000,
        reconciliation_interval_ms: 500,
    }
}

fn write_rollout(root: &Path, name: &str, lines: &[serde_json::Value]) {
    let directory = root.join("sessions").join("2026").join("08");
    fs::create_dir_all(&directory).expect("sessions");
    let body = lines
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        directory.join(format!("rollout-{name}.jsonl")),
        format!("{body}\n"),
    )
    .expect("rollout fixture");
}

fn meta(thread_id: &str, source: serde_json::Value, cwd: &str) -> serde_json::Value {
    json!({
        "timestamp": "2026-08-31T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "id": thread_id,
            "cwd": cwd,
            "cli_version": "0.150.0",
            "source": source,
            "originator": "Codex Desktop"
        }
    })
}

fn create_desktop_databases(root: &Path, catalog_ids: &[&str], persisted_ids: &[&str]) {
    let sqlite = root.join("sqlite");
    fs::create_dir_all(&sqlite).expect("sqlite root");
    let catalog_path = sqlite.join("codex-dev.db");
    let connection = rusqlite::Connection::open(&catalog_path).expect("catalog fixture");
    connection
        .execute_batch(
            "CREATE TABLE local_thread_catalog (
                host_id TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                display_title TEXT NOT NULL,
                cwd TEXT,
                source_kind TEXT NOT NULL,
                source_detail TEXT,
                observation_sequence INTEGER NOT NULL,
                project_id TEXT,
                conversation_origin TEXT
            );",
        )
        .expect("catalog schema");
    for (sequence, thread_id) in catalog_ids.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO local_thread_catalog (
                    host_id, thread_id, display_title, cwd, source_kind,
                    observation_sequence, project_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    "local",
                    thread_id,
                    "same fixture title",
                    r"C:\Dev\Fixture",
                    "local",
                    sequence as i64,
                    "project-a"
                ],
            )
            .expect("catalog row");
    }
    drop(connection);

    let state_path = sqlite.join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&state_path).expect("state fixture");
    connection
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT,
                cwd TEXT,
                source TEXT,
                model TEXT,
                agent_path TEXT,
                project_id TEXT
            );",
        )
        .expect("state schema");
    for thread_id in persisted_ids {
        connection
            .execute(
                "INSERT INTO threads (id, rollout_path, cwd, source, model, project_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    thread_id,
                    format!("rollout-{thread_id}.jsonl"),
                    r"C:\Dev\Fixture",
                    "vscode",
                    "gpt-fixture",
                    "project-a"
                ],
            )
            .expect("state row");
    }
}

fn write_global_state(root: &Path, thread_ids: &[&str]) {
    let assignments = thread_ids
        .iter()
        .map(|thread_id| {
            (
                (*thread_id).to_string(),
                json!({ "projectKind": "local", "projectId": "project-a" }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fs::write(
        root.join(".codex-global-state.json"),
        serde_json::to_vec(&json!({
            "thread-project-assignments": assignments,
            "local-projects": {
                "project-a": {
                    "id": "project-a",
                    "name": "Fixture",
                    "rootPaths": [r"C:\Dev\Fixture"]
                }
            }
        }))
        .expect("global state"),
    )
    .expect("write global state");
}

#[test]
fn desktop_projection_producer_surface_keeps_transport_separate_and_requires_corroboration() {
    let monitor = classify_producer_surface(&ProducerSurfaceInput::monitor_live());
    assert_eq!(monitor.surface, ProducerSurface::Monitor);
    assert_eq!(monitor.confidence, EvidenceConfidence::Confirmed);

    let desktop = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("vscode"),
        None,
        true,
        None,
    ));
    assert_eq!(desktop.surface, ProducerSurface::Desktop);
    assert_eq!(desktop.confidence, EvidenceConfidence::Confirmed);

    let vscode_only = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("vscode"),
        None,
        false,
        None,
    ));
    assert_eq!(vscode_only.surface, ProducerSurface::Ambiguous);

    let misleading_originator = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("exec"),
        Some("Codex Desktop"),
        false,
        None,
    ));
    assert_eq!(misleading_originator.surface, ProducerSurface::Cli);

    let ide = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("ide"),
        None,
        false,
        None,
    ));
    assert_eq!(ide.surface, ProducerSurface::Ide);

    let conflict = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("cli"),
        None,
        true,
        None,
    ));
    assert_eq!(conflict.surface, ProducerSurface::Ambiguous);

    assert!(!desktop.evidence.is_empty());
    assert!(!desktop.provenance.is_empty());
}

#[test]
fn desktop_projection_monitor_live_ingest_is_canonical_monitor_surface() {
    let mut update = near_live_update("monitor-thread");
    update.source_kind = SourceKind::MonitorAppServer;
    update.temporal_class = super::source_envelope::SourceTemporalClass::Live;
    update.source_instance_id = "monitor:fixture".to_string();
    let mut registry = SourceAuthorityRegistry::default();

    registry.ingest(update).expect("live ingest");

    assert_eq!(
        registry.snapshot().threads[0].producer_surface.surface,
        ProducerSurface::Monitor
    );
}

#[test]
fn desktop_projection_confirmed_desktop_parent_can_classify_direct_child() {
    let child = classify_producer_surface(&ProducerSurfaceInput::rollout(
        None,
        None,
        false,
        Some(ProducerSurface::Desktop),
    ));

    assert_eq!(child.surface, ProducerSurface::Desktop);
    assert_eq!(child.confidence, EvidenceConfidence::Inferred);
    assert!(child
        .evidence
        .iter()
        .any(|evidence| evidence.contains("confirmed parent edge")));

    let conflict = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("cli"),
        None,
        false,
        Some(ProducerSurface::Desktop),
    ));
    assert_eq!(conflict.surface, ProducerSurface::Ambiguous);
}

#[test]
fn desktop_projection_stale_orphan_requires_high_authority_evidence() {
    let tombstoned = assess_desktop_projection(&DesktopProjectionEvidence {
        exact_catalog_match: true,
        monitor_tombstone: true,
        confirmed_rollout_identity: false,
        authoritative_persisted_thread: AuthorityPresence::Present,
        thread_read: ThreadReadStatus::Exists,
    });
    assert_eq!(tombstoned.state, DesktopProjectionState::DesktopStaleOrphan);
    assert!(!tombstoned.canonical_ingest_allowed);

    let confirmed_absent = assess_desktop_projection(&DesktopProjectionEvidence {
        exact_catalog_match: true,
        monitor_tombstone: false,
        confirmed_rollout_identity: false,
        authoritative_persisted_thread: AuthorityPresence::Absent,
        thread_read: ThreadReadStatus::NotFound,
    });
    assert_eq!(
        confirmed_absent.state,
        DesktopProjectionState::DesktopStaleOrphan
    );
    assert!(!confirmed_absent.canonical_ingest_allowed);

    let catalog_only = assess_desktop_projection(&DesktopProjectionEvidence {
        exact_catalog_match: true,
        monitor_tombstone: false,
        confirmed_rollout_identity: false,
        authoritative_persisted_thread: AuthorityPresence::Unknown,
        thread_read: ThreadReadStatus::Unavailable,
    });
    assert_eq!(catalog_only.state, DesktopProjectionState::Ambiguous);
    assert!(!catalog_only.canonical_ingest_allowed);

    let legitimate = assess_desktop_projection(&DesktopProjectionEvidence {
        exact_catalog_match: true,
        monitor_tombstone: false,
        confirmed_rollout_identity: true,
        authoritative_persisted_thread: AuthorityPresence::Present,
        thread_read: ThreadReadStatus::Exists,
    });
    assert_eq!(
        legitimate.state,
        DesktopProjectionState::CanonicalSupplement
    );
    assert!(legitimate.canonical_ingest_allowed);
}

#[test]
fn desktop_projection_metadata_cannot_create_or_resurrect_registry_thread() {
    let mut registry = SourceAuthorityRegistry::default();
    let surface = classify_producer_surface(&ProducerSurfaceInput::rollout(
        Some("vscode"),
        None,
        true,
        None,
    ));

    assert!(!registry.supplement_desktop_projection(&key("catalog-only"), surface.clone(), None));
    assert_eq!(registry.thread_count(), 0);

    registry
        .ingest(near_live_update("deleted"))
        .expect("ingest");
    registry.retire_threads([key("deleted")]);
    assert!(!registry.supplement_desktop_projection(&key("deleted"), surface, None));
    assert!(registry.is_tombstoned(&key("deleted")));
    assert_eq!(registry.thread_count(), 0);
}

#[test]
fn desktop_projection_same_title_different_full_id_is_independent() {
    let snapshot = DesktopMetadataSnapshot {
        catalog_entries: vec![DesktopCatalogEntry {
            host_id: "local".to_string(),
            thread_id: "thread-a".to_string(),
            display_title: Some("same title".to_string()),
            cwd: None,
            source_kind: None,
            source_detail: None,
            observation_sequence: None,
            project_id: None,
        }],
        ..DesktopMetadataSnapshot::default()
    };

    assert!(snapshot.contains_catalog_thread("thread-a"));
    assert!(!snapshot.contains_catalog_thread("thread-b"));
}

#[test]
fn desktop_projection_workspace_mapping_prefers_rollout_longest_root_and_records_provenance() {
    let roots = vec![
        WorkspaceRoot::new("parent", r"C:\Dev"),
        WorkspaceRoot::new("child", r"C:\Dev\CodexMonitor"),
    ];
    let assignment = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: Some(r"c:/dev/codexmonitor/subdir"),
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: None,
    });

    assert_eq!(assignment.state, WorkspaceAssignmentState::Assigned);
    assert_eq!(assignment.workspace_id.as_deref(), Some("child"));
    assert_eq!(assignment.provenance, "rollout-cwd-longest-root");
    assert_eq!(assignment.candidate_workspace_ids, vec!["child", "parent"]);
}

#[test]
fn desktop_projection_workspace_mapping_requires_confirmed_parent_and_exposes_ties() {
    let roots = vec![
        WorkspaceRoot::new("a", r"C:\Same"),
        WorkspaceRoot::new("b", r"c:/same"),
    ];
    let ambiguous = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: Some(r"C:\Same\child"),
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: None,
    });
    assert_eq!(ambiguous.state, WorkspaceAssignmentState::Ambiguous);
    assert_eq!(ambiguous.candidate_workspace_ids, vec!["a", "b"]);

    let inherited = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: None,
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: Some("parent-workspace"),
    });
    assert_eq!(inherited.state, WorkspaceAssignmentState::Assigned);
    assert_eq!(inherited.workspace_id.as_deref(), Some("parent-workspace"));
    assert_eq!(inherited.provenance, "confirmed-parent-edge");

    let unassigned = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: None,
        configured_roots: &roots,
        desktop_project_roots: &[],
        confirmed_parent_workspace_id: None,
    });
    assert_eq!(unassigned.state, WorkspaceAssignmentState::Unassigned);
}

#[test]
fn desktop_projection_workspace_mapping_uses_desktop_project_only_as_supplement() {
    let roots = vec![WorkspaceRoot::new("workspace", r"C:\Dev\Fixture")];
    let desktop_roots = vec![r"C:\Dev\Fixture".to_string()];
    let assignment = resolve_workspace_assignment(&WorkspaceMappingInput {
        rollout_cwd: None,
        configured_roots: &roots,
        desktop_project_roots: &desktop_roots,
        confirmed_parent_workspace_id: None,
    });

    assert_eq!(assignment.state, WorkspaceAssignmentState::Assigned);
    assert_eq!(assignment.workspace_id.as_deref(), Some("workspace"));
    assert_eq!(assignment.provenance, "desktop-project-assignment");
}

#[test]
fn desktop_metadata_global_state_parsing_is_supplemental_and_schema_drift_is_graceful() {
    let snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &json!({
            "thread-project-assignments": {
                "thread-a": { "projectKind": "local", "projectId": "project-a" }
            },
            "local-projects": {
                "project-a": {
                    "id": "project-a",
                    "name": "Fixture",
                    "rootPaths": ["C:\\Dev\\Fixture"]
                }
            },
            "thread-writable-roots": {
                "thread-a": ["C:\\Dev\\Fixture"]
            }
        }),
    );
    assert_eq!(
        snapshot
            .project_assignments
            .get("thread-a")
            .map(String::as_str),
        Some("project-a")
    );
    assert_eq!(
        snapshot.projects["project-a"].root_paths,
        vec![r"C:\Dev\Fixture"]
    );

    let drifted = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &json!({ "thread-project-assignments": ["unexpected"] }),
    );
    assert!(drifted.project_assignments.is_empty());
    assert!(!drifted.diagnostics.is_empty());
}

#[test]
fn desktop_metadata_missing_and_malformed_files_do_not_block_rollout() {
    let root = std::env::temp_dir().join(format!("codex-monitor-slice2-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).expect("temp root");
    let malformed = root.join(".codex-global-state.json");
    fs::write(&malformed, "{not-json").expect("fixture");
    let paths = DesktopMetadataPaths {
        codex_home_identity: "codex-home:fixture".to_string(),
        global_state_path: malformed,
        catalog_db_path: root.join("missing-codex-dev.db"),
        persisted_state_db_path: root.join("missing-state.sqlite"),
    };

    let snapshot = DesktopMetadataReader::read(&paths);

    assert!(snapshot.catalog_entries.is_empty());
    assert!(snapshot.persisted_threads.is_empty());
    assert!(!snapshot.diagnostics.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn desktop_metadata_sqlite_reader_is_read_only_and_reads_supplemental_fields() {
    let root = temp_root();
    create_desktop_databases(&root, &["thread-a"], &["thread-a"]);
    write_global_state(&root, &["thread-a"]);
    let paths = DesktopMetadataPaths::for_codex_home("codex-home:fixture", &root);
    let catalog_before = fs::metadata(&paths.catalog_db_path)
        .expect("catalog metadata")
        .len();
    let state_before = fs::metadata(&paths.persisted_state_db_path)
        .expect("state metadata")
        .len();

    let snapshot = DesktopMetadataReader::read(&paths);

    assert!(snapshot.catalog_available);
    assert!(snapshot.persisted_state_available);
    assert!(snapshot.contains_catalog_thread("thread-a"));
    assert_eq!(
        snapshot.catalog_entries[0].project_id.as_deref(),
        Some("project-a")
    );
    assert_eq!(
        snapshot.persisted_threads["thread-a"].model.as_deref(),
        Some("gpt-fixture")
    );
    assert_eq!(
        fs::metadata(&paths.catalog_db_path)
            .expect("catalog after")
            .len(),
        catalog_before
    );
    assert_eq!(
        fs::metadata(&paths.persisted_state_db_path)
            .expect("state after")
            .len(),
        state_before
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn desktop_metadata_private_sqlite_schema_drift_degrades_without_evidence() {
    let root = temp_root();
    fs::write(root.join(".codex-global-state.json"), "{}").expect("global state");
    let sqlite = root.join("sqlite");
    fs::create_dir_all(&sqlite).expect("sqlite root");
    rusqlite::Connection::open(sqlite.join("codex-dev.db"))
        .expect("catalog")
        .execute("CREATE TABLE unrelated (value TEXT)", [])
        .expect("catalog drift");
    rusqlite::Connection::open(sqlite.join("state_5.sqlite"))
        .expect("state")
        .execute("CREATE TABLE threads (unexpected TEXT)", [])
        .expect("state drift");

    let snapshot = DesktopMetadataReader::read(&DesktopMetadataPaths::for_codex_home(
        "codex-home:fixture",
        &root,
    ));

    assert!(snapshot.catalog_entries.is_empty());
    assert!(snapshot.persisted_threads.is_empty());
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "private-schema-drift"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn desktop_projection_watcher_classifies_desktop_cli_ambiguous_and_child_without_lifecycle_pollution(
) {
    let root = temp_root();
    create_desktop_databases(
        &root,
        &["desktop-root", "desktop-child"],
        &["desktop-root", "desktop-child"],
    );
    write_global_state(&root, &["desktop-root", "desktop-child"]);
    write_rollout(
        &root,
        "desktop-root",
        &[
            meta("desktop-root", json!("vscode"), r"C:\Dev\Fixture"),
            json!({
                "timestamp": "2026-08-31T00:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "task_started", "turn_id": "turn-root", "started_at": 1788134401}
            }),
        ],
    );
    write_rollout(
        &root,
        "desktop-child",
        &[
            meta(
                "desktop-child",
                json!({"subagent": {"thread_spawn": {
                    "parent_thread_id": "desktop-root",
                    "agent_path": "/root/child"
                }}}),
                r"C:\Dev\Fixture",
            ),
            json!({
                "timestamp": "2026-08-31T00:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "task_started", "turn_id": "turn-child", "started_at": 1788134401}
            }),
        ],
    );
    write_rollout(
        &root,
        "cli",
        &[meta("cli-thread", json!("exec"), r"C:\Dev\Fixture")],
    );
    write_rollout(
        &root,
        "ide-ambiguous",
        &[meta("vscode-only", json!("vscode"), r"C:\Dev\Fixture")],
    );
    let mut watcher = RolloutTailWatcher::new(watcher_config(&root)).with_workspace_roots([
        WorkspaceRoot::new("workspace-parent", r"C:\Dev"),
        WorkspaceRoot::new("workspace-fixture", r"C:\Dev\Fixture"),
    ]);

    let report = watcher.reconcile(1_788_134_402_000).expect("reconcile");
    let snapshot = watcher.registry().snapshot();
    let by_id = snapshot
        .threads
        .iter()
        .map(|thread| (thread.key.thread_id.as_str(), thread))
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(
        by_id["desktop-root"].producer_surface.surface,
        ProducerSurface::Desktop
    );
    assert_eq!(
        by_id["desktop-child"].producer_surface.surface,
        ProducerSurface::Desktop
    );
    assert_eq!(
        by_id["cli-thread"].producer_surface.surface,
        ProducerSurface::Cli
    );
    assert_eq!(
        by_id["vscode-only"].producer_surface.surface,
        ProducerSurface::Ambiguous
    );
    assert_eq!(
        by_id["desktop-root"]
            .workspace_assignment
            .as_ref()
            .and_then(|assignment| assignment.workspace_id.as_deref()),
        Some("workspace-fixture")
    );
    assert_eq!(
        by_id["desktop-child"]
            .parent_thread_key
            .as_ref()
            .expect("parent")
            .value
            .thread_id,
        "desktop-root"
    );
    assert_eq!(
        by_id["desktop-child"]
            .lifecycle
            .as_ref()
            .map(|lifecycle| lifecycle.value),
        Some(ExternalLifecycle::Running)
    );
    assert!(report
        .desktop_projection_observations
        .iter()
        .any(|observation| {
            observation.thread_key.thread_id == "desktop-root"
                && observation.assessment.state == DesktopProjectionState::CanonicalSupplement
        }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn desktop_projection_watcher_reports_stale_orphans_without_creating_nodes() {
    let root = temp_root();
    create_desktop_databases(
        &root,
        &["tombstoned", "confirmed-absent", "catalog-only"],
        &[],
    );
    write_global_state(&root, &["tombstoned", "confirmed-absent", "catalog-only"]);
    let mut document = DeletionTombstoneDocument::default();
    document.upsert(DeletionTombstone::confirmed(
        "00000000-0000-4000-8000-000000000001",
        key("tombstoned"),
        Vec::new(),
        1_788_134_400_000,
    ));
    DeletionTombstoneStore::new(root.join("deletion-tombstones.json"))
        .save(&document)
        .expect("tombstone fixture");
    let mut watcher = RolloutTailWatcher::new(watcher_config(&root));
    watcher.record_desktop_thread_read(key("confirmed-absent"), ThreadReadStatus::NotFound);

    let report = watcher.reconcile(1_788_134_402_000).expect("reconcile");
    let states = report
        .desktop_projection_observations
        .iter()
        .map(|observation| {
            (
                observation.thread_key.thread_id.as_str(),
                observation.assessment.state,
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(
        states["tombstoned"],
        DesktopProjectionState::DesktopStaleOrphan
    );
    assert_eq!(
        states["confirmed-absent"],
        DesktopProjectionState::DesktopStaleOrphan
    );
    assert_eq!(states["catalog-only"], DesktopProjectionState::Ambiguous);
    assert_eq!(watcher.registry().thread_count(), 0);

    let unchanged = watcher.reconcile(1_788_134_405_000).expect("unchanged");
    assert!(unchanged.desktop_projection_observations.is_empty());
    watcher.record_desktop_thread_read(key("catalog-only"), ThreadReadStatus::NotFound);
    let changed = watcher
        .reconcile(1_788_134_405_100)
        .expect("changed read evidence");
    assert_eq!(changed.desktop_projection_observations.len(), 1);
    assert_eq!(
        changed.desktop_projection_observations[0].assessment.state,
        DesktopProjectionState::DesktopStaleOrphan
    );
    assert_eq!(watcher.registry().thread_count(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn desktop_projection_checkpoint_restart_restores_file_owner_classification() {
    let root = temp_root();
    create_desktop_databases(&root, &["desktop-restart"], &["desktop-restart"]);
    write_global_state(&root, &["desktop-restart"]);
    write_rollout(
        &root,
        "desktop-restart",
        &[meta("desktop-restart", json!("vscode"), r"C:\Dev\Fixture")],
    );
    let roots = [WorkspaceRoot::new("workspace-fixture", r"C:\Dev\Fixture")];
    let mut first =
        RolloutTailWatcher::new(watcher_config(&root)).with_workspace_roots(roots.clone());
    first
        .reconcile(1_788_134_402_000)
        .expect("initial reconcile");
    assert_eq!(
        first.registry().snapshot().threads[0]
            .producer_surface
            .surface,
        ProducerSurface::Desktop
    );
    drop(first);

    let mut restarted = RolloutTailWatcher::new(watcher_config(&root)).with_workspace_roots(roots);
    restarted
        .reconcile(1_788_134_405_000)
        .expect("restart reconcile");
    let thread = &restarted.registry().snapshot().threads[0];
    assert_eq!(thread.key.thread_id, "desktop-restart");
    assert_eq!(thread.producer_surface.surface, ProducerSurface::Desktop);
    assert_eq!(
        thread
            .workspace_assignment
            .as_ref()
            .and_then(|assignment| assignment.workspace_id.as_deref()),
        Some("workspace-fixture")
    );
    let _ = fs::remove_dir_all(root);
}

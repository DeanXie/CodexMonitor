use super::*;
use crate::shared::global_sources_core::desktop_metadata::{
    DesktopMetadataPaths, DesktopMetadataReader, DesktopMetadataSnapshot, DesktopPersistedThread,
};
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use serde_json::Value;
use std::{fs, path::PathBuf};

const HOST: &str = "local:fixture";

fn fixtures() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("project-workspace-interop")
        .join("contract-scenarios.json");
    serde_json::from_slice(&fs::read(path).expect("read Phase 3.2.5 fixtures"))
        .expect("valid Phase 3.2.5 fixtures")
}

fn workspace_case(gate: &str) -> Value {
    fixtures()["workspaceCases"]
        .as_array()
        .expect("workspace cases")
        .iter()
        .find(|case| case["gate"] == gate)
        .cloned()
        .expect("workspace Gate fixture")
}

fn project_case(gate: &str) -> Value {
    fixtures()["projectCases"][gate].clone()
}

fn relation_case(gate: &str) -> Value {
    fixtures()["relationCases"][gate].clone()
}

fn environment() -> ExecutionEnvironmentKey {
    ExecutionEnvironmentKey::new(HOST).expect("valid environment")
}

fn fixture_resolution(case: &Value) -> WorkspaceResolution {
    let roots = case["roots"]
        .as_array()
        .expect("roots")
        .iter()
        .map(|root| ConfiguredWorkspaceRoot::new(root["root"].as_str().expect("root locator")))
        .collect::<Vec<_>>();
    resolve_workspace_root(&WorkspaceResolutionInput {
        cwd: case["cwd"].as_str().expect("cwd"),
        platform: RootLocatorPlatform::Windows,
        execution_environment_key: &environment(),
        configured_roots: &roots,
    })
}

fn projection(case: &Value) -> DesktopProjectProjection {
    let thread_id = case["threadId"].as_str().expect("thread id");
    let mut snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &case["globalState"],
    );
    if case.get("persistedProjectId").is_some() {
        snapshot.persisted_state_available = true;
        snapshot.persisted_project_id_available = true;
        snapshot.persisted_threads.insert(
            thread_id.to_string(),
            DesktopPersistedThread {
                thread_id: thread_id.to_string(),
                project_id: case["persistedProjectId"].as_str().map(ToString::to_string),
                ..DesktopPersistedThread::default()
            },
        );
    }
    resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &CodexThreadKey::new("codex-home:fixture", thread_id),
        desktop_host_identity: HOST,
        metadata: &snapshot,
    })
}

fn empty_project_projection(thread_id: &str) -> DesktopProjectProjection {
    let case = project_case("E");
    let snapshot = DesktopMetadataSnapshot::from_global_state_value(
        "codex-home:fixture",
        &case["globalState"],
    );
    resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &CodexThreadKey::new("codex-home:fixture", thread_id),
        desktop_host_identity: HOST,
        metadata: &snapshot,
    })
}

fn runtime() -> RuntimeWorkspaceReconciler {
    let mut runtime = RuntimeWorkspaceReconciler::new(
        "codex-home:fixture",
        environment(),
        RootLocatorPlatform::Windows,
    );
    runtime.register_workspace("workspace-monitor", r"F:\AI\CodexMonitor");
    runtime.register_workspace("workspace-isolated", r"D:\isolated\repo");
    runtime
}

fn ambiguous_resolution() -> WorkspaceResolution {
    let locator =
        NormalizedRootLocator::parse(r"F:\AI\CodexMonitor", RootLocatorPlatform::Windows).unwrap();
    let key_b = WorkspaceKey::new(
        ExecutionEnvironmentKey::new("local:b").unwrap(),
        locator.clone(),
    );
    let key_a = WorkspaceKey::new(ExecutionEnvironmentKey::new("local:a").unwrap(), locator);
    super::resolver::finalize_workspace_candidates(vec![(3, key_b), (3, key_a)])
}

#[test]
fn phase_3_2_5_contract_gate_a_unique_workspace_root() {
    let case = workspace_case("A");
    let resolution = fixture_resolution(&case);
    assert_eq!(resolution.state, WorkspaceResolutionState::Assigned);
    assert_eq!(resolution.candidate_workspace_keys.len(), 1);
    assert_eq!(
        resolution
            .workspace_key
            .as_ref()
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "f:/ai/codexmonitor"
    );

    let mut runtime = runtime();
    let origin = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-gate-a",
        thread_start_cwd: case["cwd"].as_str(),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let turn = runtime.observe_turn(RuntimeTurnWorkspaceObservation {
        thread_id: "thread-gate-a",
        turn_id: "turn-gate-a",
        explicit_turn_cwd: case["cwd"].as_str(),
        turn_context_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 2,
    });
    assert_eq!(origin.workspace_id.as_deref(), Some("workspace-monitor"));
    assert_eq!(origin.workspace_key, turn.workspace_key);
}

#[test]
fn phase_3_2_5_contract_gate_b_nested_root_uses_longest_boundary_match() {
    let resolution = fixture_resolution(&workspace_case("B"));
    assert_eq!(resolution.state, WorkspaceResolutionState::Assigned);
    assert_eq!(
        resolution
            .workspace_key
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "f:/ai/codexmonitor"
    );
}

#[test]
fn phase_3_2_5_contract_gate_c_duplicate_root_is_one_canonical_candidate() {
    let case = workspace_case("C");
    let resolution = fixture_resolution(&case);
    assert_eq!(resolution.state, WorkspaceResolutionState::Assigned);
    assert_eq!(resolution.candidate_workspace_keys.len(), 1);

    let mut runtime = RuntimeWorkspaceReconciler::new(
        "codex-home:fixture",
        environment(),
        RootLocatorPlatform::Windows,
    );
    for root in case["roots"].as_array().unwrap() {
        runtime.register_workspace(
            root["workspaceId"].as_str().unwrap(),
            root["root"].as_str().unwrap(),
        );
    }
    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-gate-c",
        thread_start_cwd: case["cwd"].as_str(),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    assert_eq!(route.state, WorkspaceResolutionState::Assigned);
    assert_eq!(route.workspace_id.as_deref(), Some("workspace-a"));
}

#[test]
fn phase_3_2_5_contract_gate_d_equal_longest_distinct_keys_do_not_route() {
    let resolution = ambiguous_resolution();
    assert_eq!(resolution.state, WorkspaceResolutionState::Ambiguous);
    assert_eq!(
        resolution.candidate_workspace_keys[0]
            .execution_environment_key
            .as_str(),
        "local:a"
    );

    let observation = WorkspaceRelationObservation::from_resolution(
        resolution,
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        Some(r"F:\AI\CodexMonitor".to_string()),
        1,
    );
    let relation = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key: &CodexThreadKey::new("codex-home:fixture", "thread-gate-d"),
        thread_start: Some(&observation),
        session_meta: None,
        parent_fallback: None,
        observed_at: 1,
    });
    let route = runtime().observe_relation(relation);
    assert_eq!(route.state, WorkspaceResolutionState::Ambiguous);
    assert!(route.workspace_id.is_none());
    assert!(route.workspace_key.is_none());
}

#[test]
fn phase_3_2_5_contract_gate_e_overlapping_project_roots_do_not_assign_thread() {
    let project = projection(&project_case("E"));
    assert_eq!(project.state, WorkspaceResolutionState::Unassigned);
    assert!(project.candidate_projects.is_empty());
    assert!(project.configured_roots.is_empty());
    assert_eq!(
        fixture_resolution(&workspace_case("A")).state,
        WorkspaceResolutionState::Assigned
    );
}

#[test]
fn phase_3_2_5_contract_gate_f_explicit_assignment_and_alias_are_one_candidate() {
    let project = projection(&project_case("F"));
    assert_eq!(project.state, WorkspaceResolutionState::Assigned);
    assert_eq!(project.candidate_projects.len(), 1);
    assert_eq!(project.legacy_project_id.as_deref(), Some("legacy-a"));
    assert_eq!(
        project.app_server_project_id.as_deref(),
        Some("app-project-p")
    );
    assert_eq!(project.direct_assignments.len(), 1);
    assert_eq!(
        fixture_resolution(&workspace_case("A"))
            .workspace_key
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "f:/ai/codexmonitor"
    );
}

#[test]
fn phase_3_2_5_contract_gate_g_origin_and_later_turn_keep_one_thread_identity() {
    let case = relation_case("G");
    let thread_id = case["threadId"].as_str().unwrap();
    let mut runtime = RuntimeWorkspaceReconciler::new(
        "codex-home:fixture",
        environment(),
        RootLocatorPlatform::Windows,
    );
    runtime.register_workspace("workspace-origin", case["originRoot"].as_str().unwrap());
    runtime.register_workspace("workspace-turn", case["turnRoot"].as_str().unwrap());
    let origin = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id,
        thread_start_cwd: None,
        session_meta_cwd: case["originCwd"].as_str(),
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let turn = runtime.observe_turn(RuntimeTurnWorkspaceObservation {
        thread_id,
        turn_id: case["turnId"].as_str().unwrap(),
        explicit_turn_cwd: case["turnCwd"].as_str(),
        turn_context_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 2,
    });
    assert_eq!(origin.workspace_id.as_deref(), Some("workspace-origin"));
    assert_eq!(turn.workspace_id.as_deref(), Some("workspace-turn"));
    assert_eq!(
        runtime.route_for_origin(thread_id).unwrap().workspace_key,
        origin.workspace_key
    );
    assert_eq!(
        runtime
            .route_for_turn(thread_id, case["turnId"].as_str().unwrap())
            .unwrap()
            .workspace_key,
        turn.workspace_key
    );
}

#[test]
fn phase_3_2_5_contract_gate_h_child_direct_and_confirmed_parent_fallback() {
    let case = relation_case("H");
    let mut runtime = runtime();
    runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["parentThreadId"].as_str().unwrap(),
        thread_start_cwd: case["parentCwd"].as_str(),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let direct = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["childDirectThreadId"].as_str().unwrap(),
        thread_start_cwd: case["childDirectCwd"].as_str(),
        session_meta_cwd: None,
        confirmed_parent_thread_id: case["parentThreadId"].as_str(),
        observed_at: 2,
    });
    let fallback = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["childFallbackThreadId"].as_str().unwrap(),
        thread_start_cwd: None,
        session_meta_cwd: None,
        confirmed_parent_thread_id: case["parentThreadId"].as_str(),
        observed_at: 3,
    });
    assert_eq!(direct.workspace_id.as_deref(), Some("workspace-isolated"));
    assert_eq!(direct.basis, ThreadWorkspaceRelationBasis::DirectCwd);
    assert_eq!(fallback.workspace_id.as_deref(), Some("workspace-monitor"));
    assert_eq!(fallback.basis, ThreadWorkspaceRelationBasis::ParentFallback);

    let observation = WorkspaceRelationObservation::from_resolution(
        ambiguous_resolution(),
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        Some(r"F:\AI\CodexMonitor".to_string()),
        4,
    );
    let parent = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key: &CodexThreadKey::new("codex-home:fixture", "thread-ambiguous-parent"),
        thread_start: Some(&observation),
        session_meta: None,
        parent_fallback: None,
        observed_at: 4,
    });
    runtime.observe_relation(parent);
    let blocked = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["childBlockedThreadId"].as_str().unwrap(),
        thread_start_cwd: None,
        session_meta_cwd: None,
        confirmed_parent_thread_id: Some("thread-ambiguous-parent"),
        observed_at: 5,
    });
    assert_eq!(blocked.state, WorkspaceResolutionState::Unknown);
    assert!(blocked.workspace_id.is_none());
}

#[test]
fn phase_3_2_5_contract_gate_i_external_cli_workspace_and_project_stay_independent() {
    let case = relation_case("I");
    assert_eq!(case["creator"], "CLI");
    let mut runtime = runtime();
    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["threadId"].as_str().unwrap(),
        thread_start_cwd: None,
        session_meta_cwd: case["sessionMetaCwd"].as_str(),
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let project = empty_project_projection(case["threadId"].as_str().unwrap());
    assert_eq!(route.state, WorkspaceResolutionState::Assigned);
    assert_eq!(project.state, WorkspaceResolutionState::Unassigned);
}

#[test]
fn phase_3_2_5_contract_gate_j_monitor_start_cwd_does_not_create_project_assignment() {
    let case = relation_case("J");
    assert_eq!(case["creator"], "Monitor");
    let mut runtime = runtime();
    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: case["threadId"].as_str().unwrap(),
        thread_start_cwd: case["threadStartCwd"].as_str(),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let project = empty_project_projection(case["threadId"].as_str().unwrap());
    assert_eq!(route.state, WorkspaceResolutionState::Assigned);
    assert_eq!(project.state, WorkspaceResolutionState::Unassigned);
}

#[test]
fn phase_3_2_5_contract_gate_k_missing_invalid_and_no_match_are_distinct() {
    let missing = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key: &CodexThreadKey::new("codex-home:fixture", "thread-missing-cwd"),
        thread_start: None,
        session_meta: None,
        parent_fallback: None,
        observed_at: 1,
    });
    assert_eq!(missing.state, WorkspaceResolutionState::Unknown);
    assert_eq!(
        fixture_resolution(&workspace_case("K-invalid")).state,
        WorkspaceResolutionState::Unknown
    );
    assert_eq!(
        fixture_resolution(&workspace_case("K-no-match")).state,
        WorkspaceResolutionState::Unassigned
    );
}

#[test]
fn phase_3_2_5_contract_gate_l_conflict_and_schema_drift_do_not_affect_workspace() {
    let conflict = projection(&project_case("L-conflict"));
    assert_eq!(conflict.state, WorkspaceResolutionState::Ambiguous);
    assert_eq!(conflict.candidate_projects.len(), 2);
    let drift = projection(&project_case("L-schema-drift"));
    assert_eq!(drift.state, WorkspaceResolutionState::Unknown);
    assert!(drift
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "private-schema-drift"));
    assert_eq!(
        fixture_resolution(&workspace_case("A")).state,
        WorkspaceResolutionState::Assigned
    );
}

#[test]
fn phase_3_2_5_contract_restart_is_deterministic_and_does_not_forge_turn_history() {
    let case = relation_case("G");
    let thread_id = case["threadId"].as_str().unwrap();
    let mut before = RuntimeWorkspaceReconciler::new(
        "codex-home:fixture",
        environment(),
        RootLocatorPlatform::Windows,
    );
    before.register_workspace("workspace-turn", case["turnRoot"].as_str().unwrap());
    before.register_workspace("workspace-origin", case["originRoot"].as_str().unwrap());
    before.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id,
        thread_start_cwd: None,
        session_meta_cwd: case["originCwd"].as_str(),
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    before.observe_turn(RuntimeTurnWorkspaceObservation {
        thread_id,
        turn_id: case["turnId"].as_str().unwrap(),
        explicit_turn_cwd: None,
        turn_context_cwd: case["turnCwd"].as_str(),
        confirmed_parent_thread_id: None,
        observed_at: 2,
    });

    let mut restarted = RuntimeWorkspaceReconciler::new(
        "codex-home:fixture",
        environment(),
        RootLocatorPlatform::Windows,
    );
    restarted.register_workspace("workspace-origin", case["originRoot"].as_str().unwrap());
    restarted.register_workspace("workspace-turn", case["turnRoot"].as_str().unwrap());
    restarted.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id,
        thread_start_cwd: None,
        session_meta_cwd: case["originCwd"].as_str(),
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });

    assert_eq!(
        before.route_for_origin(thread_id),
        restarted.route_for_origin(thread_id)
    );
    assert!(before
        .route_for_turn(thread_id, case["turnId"].as_str().unwrap())
        .is_some());
    assert!(restarted
        .route_for_turn(thread_id, case["turnId"].as_str().unwrap())
        .is_none());
}

#[test]
#[ignore = "reads the current Desktop metadata and Monitor workspace store without writing"]
fn phase_3_2_5_real_environment_probe() {
    let codex_home =
        PathBuf::from(std::env::var("PHASE_3_2_5_CODEX_HOME").expect("PHASE_3_2_5_CODEX_HOME"));
    let workspace_store = PathBuf::from(
        std::env::var("PHASE_3_2_5_WORKSPACE_STORE").expect("PHASE_3_2_5_WORKSPACE_STORE"),
    );
    let snapshot = DesktopMetadataReader::read(&DesktopMetadataPaths::for_codex_home(
        "codex-home:real-probe",
        &codex_home,
    ));
    let mut hosts = snapshot
        .project_migrations_by_host
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    hosts.sort();
    let host = hosts.first().expect("Desktop migration host");
    let migration = snapshot
        .project_migrations_by_host
        .get(host)
        .expect("migration state");
    let aliases = snapshot
        .project_id_mappings_by_host
        .get(host)
        .expect("project aliases");
    let mut direct = snapshot
        .project_assignments
        .iter()
        .filter_map(|(thread_id, legacy_project_id)| {
            aliases.get(legacy_project_id).map(|app_server_project_id| {
                (
                    thread_id.clone(),
                    legacy_project_id.clone(),
                    app_server_project_id.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    direct.sort();
    let (thread_id, legacy_project_id, app_server_project_id) =
        direct.first().expect("mapped direct assignment");
    let projection = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
        thread_key: &CodexThreadKey::new("codex-home:real-probe", thread_id),
        desktop_host_identity: host,
        metadata: &snapshot,
    });
    let persisted_project_id = snapshot
        .persisted_threads
        .get(thread_id)
        .and_then(|thread| thread.project_id.clone());
    let overlap_count = snapshot
        .projects
        .values()
        .filter(|project| {
            project
                .root_paths
                .iter()
                .any(|root| root.eq_ignore_ascii_case(r"F:\AI\CodexMonitor"))
        })
        .count();
    let workspaces = crate::storage::read_workspaces(&workspace_store).expect("workspace store");
    let mut matching_workspace_ids = workspaces
        .values()
        .filter(|workspace| workspace.path.eq_ignore_ascii_case(r"F:\AI\CodexMonitor"))
        .map(|workspace| workspace.id.clone())
        .collect::<Vec<_>>();
    matching_workspace_ids.sort();
    let probe_threads = [
        (
            "CLI",
            std::env::var("PHASE_3_2_5_CLI_THREAD_ID").expect("CLI test Thread"),
            false,
            WorkspaceResolutionState::Unassigned,
        ),
        (
            "MonitorProject",
            std::env::var("PHASE_3_2_5_MONITOR_THREAD_ID").expect("Monitor test Thread"),
            true,
            WorkspaceResolutionState::Assigned,
        ),
        (
            "AppServer",
            std::env::var("PHASE_3_2_5_APP_SERVER_THREAD_ID").expect("app-server test Thread"),
            true,
            WorkspaceResolutionState::Unassigned,
        ),
    ];
    let mut runtime = RuntimeWorkspaceReconciler::new(
        "codex-home:real-probe",
        ExecutionEnvironmentKey::new(host).expect("real environment key"),
        RootLocatorPlatform::Windows,
    );
    runtime.register_workspace(
        matching_workspace_ids
            .first()
            .expect("matching WorkspaceEntry"),
        r"F:\AI\CodexMonitor",
    );
    let mut probe_results = Vec::new();
    for (creator, thread_id, thread_start_is_direct, expected_project_state) in probe_threads {
        let project = resolve_desktop_project_projection(&DesktopProjectProjectionInput {
            thread_key: &CodexThreadKey::new("codex-home:real-probe", &thread_id),
            desktop_host_identity: host,
            metadata: &snapshot,
        });
        assert_eq!(project.state, expected_project_state);
        let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
            thread_id: &thread_id,
            thread_start_cwd: thread_start_is_direct.then_some(r"F:\AI\CodexMonitor"),
            session_meta_cwd: (!thread_start_is_direct).then_some(r"F:\AI\CodexMonitor"),
            confirmed_parent_thread_id: None,
            observed_at: 1,
        });
        assert_eq!(route.state, WorkspaceResolutionState::Assigned);
        assert_eq!(route.workspace_id, matching_workspace_ids.first().cloned());
        probe_results.push(serde_json::json!({
            "creator": creator,
            "fullThreadId": thread_id,
            "workspaceState": "ASSIGNED",
            "workspaceId": route.workspace_id,
            "workspaceRoot": route
                .workspace_key
                .as_ref()
                .map(|key| key.normalized_root_locator.as_str()),
            "originBasis": if thread_start_is_direct { "thread/start.cwd" } else { "session_meta.cwd" },
            "desktopProjectState": match project.state {
                WorkspaceResolutionState::Assigned => "ASSIGNED",
                WorkspaceResolutionState::Ambiguous => "AMBIGUOUS",
                WorkspaceResolutionState::Unassigned => "UNASSIGNED",
                WorkspaceResolutionState::Unknown => "UNKNOWN",
            },
            "legacyProjectId": project.legacy_project_id,
            "appServerProjectId": project.app_server_project_id,
            "canonicalProjectCandidateCount": project.candidate_projects.len()
        }));
    }

    assert_eq!(migration.projects_migrated, Some(true));
    assert_eq!(migration.thread_assignments_migrated, Some(false));
    assert!(overlap_count >= 2);
    assert_eq!(projection.state, WorkspaceResolutionState::Assigned);
    assert_eq!(projection.candidate_projects.len(), 1);
    assert_eq!(
        projection.legacy_project_id.as_ref(),
        Some(legacy_project_id)
    );
    assert_eq!(
        projection.app_server_project_id.as_ref(),
        Some(app_server_project_id)
    );
    assert_eq!(persisted_project_id, None);
    assert_eq!(matching_workspace_ids.len(), 1);

    println!(
        "{}",
        serde_json::json!({
            "host": host,
            "projectsMigrated": migration.projects_migrated,
            "threadAssignmentsMigrated": migration.thread_assignments_migrated,
            "projectCount": snapshot.projects.len(),
            "explicitAssignmentCount": snapshot.project_assignments.len(),
            "codexMonitorConfiguredProjectRootCount": overlap_count,
            "codexMonitorWorkspaceIds": matching_workspace_ids,
            "selectedExplicitAssignment": {
                "fullThreadId": thread_id,
                "legacyProjectId": legacy_project_id,
                "appServerProjectId": app_server_project_id,
                "persistedProjectId": persisted_project_id,
                "projectionState": "ASSIGNED",
                "canonicalCandidateCount": projection.candidate_projects.len()
            },
            "isolatedTestThreads": probe_results,
            "diagnostics": snapshot
                .diagnostics
                .iter()
                .map(|diagnostic| serde_json::json!({
                    "source": diagnostic.source,
                    "code": diagnostic.code,
                    "message": diagnostic.message
                }))
                .collect::<Vec<_>>()
        })
    );
}

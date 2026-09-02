use super::*;
use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};

fn environment(value: &str) -> ExecutionEnvironmentKey {
    ExecutionEnvironmentKey::new(value).expect("valid execution environment key")
}

fn resolve(
    cwd: &str,
    platform: RootLocatorPlatform,
    environment_key: &ExecutionEnvironmentKey,
    roots: &[&str],
) -> WorkspaceResolution {
    let configured_roots = roots
        .iter()
        .map(|root| ConfiguredWorkspaceRoot::new(*root))
        .collect::<Vec<_>>();
    resolve_workspace_root(&WorkspaceResolutionInput {
        cwd,
        platform,
        execution_environment_key: environment_key,
        configured_roots: &configured_roots,
    })
}

#[test]
fn windows_case_slash_namespace_variants_match() {
    let environment_key = environment("windows-host-a");

    let result = resolve(
        r"\\?\C:\REPO\src",
        RootLocatorPlatform::Windows,
        &environment_key,
        &["c:/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Assigned);
    assert_eq!(
        result.workspace_key,
        Some(WorkspaceKey::new(
            environment_key,
            NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap(),
        ))
    );
}

#[test]
fn trailing_slash_does_not_change_root() {
    let without_slash =
        NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();
    let with_slash =
        NormalizedRootLocator::parse(r"c:/REPO/", RootLocatorPlatform::Windows).unwrap();

    assert_eq!(without_slash, with_slash);
    assert_eq!(without_slash.as_str(), "c:/repo");
}

#[test]
fn path_component_boundary_prevents_prefix_collision() {
    let result = resolve(
        r"C:\repo2\src",
        RootLocatorPlatform::Windows,
        &environment("windows-host-a"),
        &[r"C:\repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Unassigned);
    assert!(result.workspace_key.is_none());
    assert!(result.candidate_workspace_keys.is_empty());
}

#[test]
fn nested_roots_choose_longest() {
    let environment_key = environment("linux-container-a");
    let result = resolve(
        "/srv/repo/packages/app/src",
        RootLocatorPlatform::Posix,
        &environment_key,
        &["/srv/repo", "/srv/repo/packages/app"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Assigned);
    assert_eq!(
        result.workspace_key,
        Some(WorkspaceKey::new(
            environment_key,
            NormalizedRootLocator::parse("/srv/repo/packages/app", RootLocatorPlatform::Posix,)
                .unwrap(),
        ))
    );
}

#[test]
fn duplicate_roots_normalizing_to_same_workspace_key_are_assigned() {
    let environment_key = environment("windows-host-a");
    let result = resolve(
        r"C:\repo\src",
        RootLocatorPlatform::Windows,
        &environment_key,
        &[r"C:\repo", r"\\?\c:\REPO\"],
    );

    let expected_key = WorkspaceKey::new(
        environment_key,
        NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap(),
    );
    assert_eq!(result.state, WorkspaceResolutionState::Assigned);
    assert_eq!(result.workspace_key, Some(expected_key.clone()));
    assert_eq!(result.candidate_workspace_keys, vec![expected_key]);
}

#[test]
fn same_path_in_different_execution_environments_does_not_match() {
    let locator = NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();

    let host_key = WorkspaceKey::new(environment("windows-host-a"), locator.clone());
    let container_key = WorkspaceKey::new(environment("windows-container-b"), locator);

    assert_ne!(host_key, container_key);
}

#[test]
fn unresolved_symlink_or_relocation_never_claims_confirmed_identity() {
    let result = resolve(
        "/srv/repo/relocated-link/src",
        RootLocatorPlatform::Posix,
        &environment("linux-host-a"),
        &["/srv/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Assigned);
    assert_eq!(result.physical_identity, PhysicalIdentityStatus::Unresolved);
}

#[test]
fn windows_drive_root_remains_valid() {
    let root = NormalizedRootLocator::parse(r"C:\", RootLocatorPlatform::Windows).unwrap();
    let result = resolve(
        r"c:\repo\src",
        RootLocatorPlatform::Windows,
        &environment("windows-host-a"),
        &[r"C:\"],
    );

    assert_eq!(root.as_str(), "c:/");
    assert_eq!(result.state, WorkspaceResolutionState::Assigned);
}

#[test]
fn unc_root_normalization_is_stable() {
    let namespaced =
        NormalizedRootLocator::parse(r"\\?\UNC\Server\Share\", RootLocatorPlatform::Windows)
            .unwrap();
    let device_namespaced =
        NormalizedRootLocator::parse(r"\\.\UNC\SERVER\SHARE", RootLocatorPlatform::Windows)
            .unwrap();
    let ordinary =
        NormalizedRootLocator::parse(r"\\server\share", RootLocatorPlatform::Windows).unwrap();

    assert_eq!(namespaced.as_str(), "//server/share");
    assert_eq!(namespaced, device_namespaced);
    assert_eq!(namespaced, ordinary);
}

#[test]
fn no_matching_root_is_unassigned() {
    let result = resolve(
        "/opt/other/src",
        RootLocatorPlatform::Posix,
        &environment("linux-host-a"),
        &["/srv/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Unassigned);
    assert!(result.workspace_key.is_none());
}

#[test]
fn malformed_or_uninterpretable_locator_is_unknown() {
    let result = resolve(
        "relative/path",
        RootLocatorPlatform::Posix,
        &environment("linux-host-a"),
        &["/srv/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Unknown);
    assert!(result.workspace_key.is_none());
    assert!(result.candidate_workspace_keys.is_empty());
}

#[test]
fn parent_component_is_unknown_without_physical_path_evidence() {
    let result = resolve(
        "/srv/repo/linked-directory/../src",
        RootLocatorPlatform::Posix,
        &environment("linux-host-a"),
        &["/srv/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Unknown);
    assert!(result.workspace_key.is_none());
}

#[test]
fn duplicate_projection_records_do_not_create_canonical_ambiguity() {
    let environment_key = environment("windows-host-a");
    let forward = resolve(
        r"C:\repo\src",
        RootLocatorPlatform::Windows,
        &environment_key,
        &[r"\\?\C:\REPO", r"C:/repo/", r"\\.\c:\repo"],
    );
    let reversed = resolve(
        r"C:\repo\src",
        RootLocatorPlatform::Windows,
        &environment_key,
        &[r"\\.\c:\repo", r"C:/repo/", r"\\?\C:\REPO"],
    );

    let expected_key = WorkspaceKey::new(
        environment_key,
        NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap(),
    );
    assert_eq!(forward.state, WorkspaceResolutionState::Assigned);
    assert_eq!(forward.workspace_key, Some(expected_key.clone()));
    assert_eq!(forward.candidate_workspace_keys, vec![expected_key]);
    assert_eq!(
        forward.candidate_workspace_keys,
        reversed.candidate_workspace_keys
    );
}

#[test]
fn equal_longest_distinct_workspace_keys_remain_ambiguous() {
    let locator = NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();
    let key_b = WorkspaceKey::new(environment("windows-host-b"), locator.clone());
    let key_a = WorkspaceKey::new(environment("windows-host-a"), locator);

    let result = super::resolver::finalize_workspace_candidates(vec![
        (1, key_b.clone()),
        (1, key_a.clone()),
    ]);

    assert_eq!(result.state, WorkspaceResolutionState::Ambiguous);
    assert!(result.workspace_key.is_none());
    assert_eq!(result.candidate_workspace_keys, vec![key_a, key_b]);
}

#[test]
fn posix_matching_remains_case_sensitive() {
    let result = resolve(
        "/srv/Repo/src",
        RootLocatorPlatform::Posix,
        &environment("linux-host-a"),
        &["/srv/repo"],
    );

    assert_eq!(result.state, WorkspaceResolutionState::Unassigned);
}

#[test]
fn dot_components_are_normalized_without_filesystem_identity_claims() {
    let locator =
        NormalizedRootLocator::parse("/srv/repo/./src/./", RootLocatorPlatform::Posix).unwrap();

    assert_eq!(locator.as_str(), "/srv/repo/src");
}

fn thread(value: &str) -> CodexThreadKey {
    CodexThreadKey::new("codex-home-fixture", value)
}

fn observed_cwd(
    cwd: &str,
    roots: &[&str],
    source: ThreadWorkspaceProvenanceKind,
    observed_at: u64,
) -> WorkspaceRelationObservation {
    WorkspaceRelationObservation::from_cwd(
        cwd,
        RootLocatorPlatform::Windows,
        environment("windows-host-a"),
        roots
            .iter()
            .map(|root| ConfiguredWorkspaceRoot::new(*root))
            .collect(),
        source,
        observed_at,
    )
}

fn origin_relation(
    thread_key: &CodexThreadKey,
    thread_start: Option<&WorkspaceRelationObservation>,
    session_meta: Option<&WorkspaceRelationObservation>,
    parent_fallback: Option<ParentWorkspaceFallback<'_>>,
) -> ThreadWorkspaceRelation {
    resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key,
        thread_start,
        session_meta,
        parent_fallback,
        observed_at: 100,
    })
}

fn turn_relation(
    turn_key: &CodexTurnKey,
    explicit_turn: Option<&WorkspaceRelationObservation>,
    turn_context: Option<&WorkspaceRelationObservation>,
    parent_fallback: Option<ParentWorkspaceFallback<'_>>,
) -> ThreadWorkspaceRelation {
    resolve_turn_execution_workspace_relation(&TurnExecutionWorkspaceRelationInput {
        turn_key,
        explicit_turn,
        turn_context,
        parent_fallback,
        observed_at: 100,
    })
}

#[test]
fn origin_workspace_is_preserved_when_later_turn_changes_cwd() {
    let thread_key = thread("same-full-thread-id");
    let origin = observed_cwd(
        r"C:\Users\fixture\repo\src",
        &[r"C:\Users\fixture\repo", r"F:\AI\CodexMonitor"],
        ThreadWorkspaceProvenanceKind::SessionMetaCwd,
        10,
    );
    let later_turn = observed_cwd(
        r"F:\AI\CodexMonitor.worktrees\phase-3\src",
        &[
            r"C:\Users\fixture\repo",
            r"F:\AI\CodexMonitor.worktrees\phase-3",
        ],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        20,
    );
    let origin = origin_relation(&thread_key, None, Some(&origin), None);
    let turn = turn_relation(
        &CodexTurnKey::new(thread_key.clone(), "turn-later"),
        Some(&later_turn),
        None,
        None,
    );

    assert_eq!(
        origin
            .workspace_key()
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "c:/users/fixture/repo"
    );
    assert_eq!(
        turn.workspace_key()
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "f:/ai/codexmonitor.worktrees/phase-3"
    );
    assert_eq!(origin.key.thread_key, turn.key.thread_key);
    assert_eq!(origin.basis, ThreadWorkspaceRelationBasis::DirectCwd);
    assert_eq!(
        origin.provenance[0].kind,
        ThreadWorkspaceProvenanceKind::SessionMetaCwd
    );
}

#[test]
fn each_turn_has_independent_execution_workspace_relation() {
    let thread_key = thread("thread-a");
    let turn_a = CodexTurnKey::new(thread_key.clone(), "turn-a");
    let turn_b = CodexTurnKey::new(thread_key, "turn-b");
    let cwd_a = observed_cwd(
        r"C:\repo\src",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        1,
    );
    let cwd_b = observed_cwd(
        r"F:\repo\src",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::TurnContextCwd,
        2,
    );

    let relation_a = turn_relation(&turn_a, Some(&cwd_a), None, None);
    let relation_b = turn_relation(&turn_b, None, Some(&cwd_b), None);

    assert_ne!(relation_a.key, relation_b.key);
    assert_ne!(relation_a.workspace_key(), relation_b.workspace_key());
}

#[test]
fn same_thread_can_have_multiple_turn_workspace_keys() {
    let thread_key = thread("thread-a");
    let first = observed_cwd(
        r"C:\repo",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        1,
    );
    let second = observed_cwd(
        r"F:\repo",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );
    let mut store = ThreadWorkspaceRelationStore::default();

    store.observe(turn_relation(
        &CodexTurnKey::new(thread_key.clone(), "turn-1"),
        Some(&first),
        None,
        None,
    ));
    store.observe(turn_relation(
        &CodexTurnKey::new(thread_key.clone(), "turn-2"),
        Some(&second),
        None,
        None,
    ));

    let relations = store.relations_for_thread(&thread_key);
    assert_eq!(relations.len(), 2);
    assert_ne!(relations[0].workspace_key(), relations[1].workspace_key());
}

#[test]
fn turn_direct_cwd_wins_over_parent_fallback() {
    let parent = thread("parent");
    let child = thread("child");
    let parent_cwd = observed_cwd(
        r"C:\parent",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let parent_relation = origin_relation(&parent, Some(&parent_cwd), None, None);
    let child_turn = CodexTurnKey::new(child, "turn-1");
    let direct = observed_cwd(
        r"F:\child",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );

    let relation = turn_relation(
        &child_turn,
        Some(&direct),
        None,
        Some(ParentWorkspaceFallback::confirmed(&parent_relation)),
    );

    assert_eq!(relation.basis, ThreadWorkspaceRelationBasis::DirectCwd);
    assert_eq!(
        relation
            .workspace_key()
            .unwrap()
            .normalized_root_locator
            .as_str(),
        "f:/child"
    );
}

#[test]
fn child_without_cwd_can_inherit_confirmed_parent_workspace() {
    let parent_cwd = observed_cwd(
        r"C:\parent",
        &[r"C:\parent"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let parent_relation = origin_relation(&thread("parent"), Some(&parent_cwd), None, None);

    let child_relation = origin_relation(
        &thread("child"),
        None,
        None,
        Some(ParentWorkspaceFallback::confirmed(&parent_relation)),
    );

    assert_eq!(child_relation.state, WorkspaceResolutionState::Assigned);
    assert_eq!(
        child_relation.basis,
        ThreadWorkspaceRelationBasis::ParentFallback
    );
    assert_eq!(
        child_relation.confidence,
        ThreadWorkspaceRelationConfidence::Inferred
    );
    assert_eq!(
        child_relation.workspace_key(),
        parent_relation.workspace_key()
    );
}

#[test]
fn child_without_confirmed_parent_remains_unknown() {
    let parent_cwd = observed_cwd(
        r"C:\parent",
        &[r"C:\parent"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let parent_relation = origin_relation(&thread("parent"), Some(&parent_cwd), None, None);

    let child_relation = origin_relation(
        &thread("child"),
        None,
        None,
        Some(ParentWorkspaceFallback::unconfirmed(&parent_relation)),
    );

    assert_eq!(child_relation.state, WorkspaceResolutionState::Unknown);
    assert!(child_relation.workspace_key().is_none());
    assert!(child_relation.candidate_workspace_keys.is_empty());
}

#[test]
fn ambiguous_root_resolution_is_preserved_in_relation() {
    let locator = NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();
    let key_a = WorkspaceKey::new(environment("windows-host-a"), locator.clone());
    let key_b = WorkspaceKey::new(environment("windows-host-b"), locator);
    let resolution = super::resolver::finalize_workspace_candidates(vec![
        (1, key_b.clone()),
        (1, key_a.clone()),
    ]);
    let observation = WorkspaceRelationObservation::from_resolution(
        resolution,
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        Some(r"C:\repo".to_string()),
        1,
    );

    let relation = turn_relation(
        &CodexTurnKey::new(thread("thread-a"), "turn-1"),
        Some(&observation),
        None,
        None,
    );

    assert_eq!(relation.state, WorkspaceResolutionState::Ambiguous);
    assert!(relation.workspace_key().is_none());
    assert_eq!(relation.candidate_workspace_keys, vec![key_a, key_b]);
}

#[test]
fn unassigned_and_unknown_remain_distinct() {
    let no_match = observed_cwd(
        r"F:\other",
        &[r"C:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        1,
    );
    let turn_key = CodexTurnKey::new(thread("thread-a"), "turn-1");

    let unassigned = turn_relation(&turn_key, Some(&no_match), None, None);
    let unknown = turn_relation(&turn_key, None, None, None);

    assert_eq!(unassigned.state, WorkspaceResolutionState::Unassigned);
    assert_eq!(unknown.state, WorkspaceResolutionState::Unknown);
    assert!(unassigned.candidate_workspace_keys.is_empty());
    assert!(unknown.candidate_workspace_keys.is_empty());
}

#[test]
fn duplicate_workspace_candidates_do_not_create_false_ambiguity() {
    let observation = observed_cwd(
        r"C:\repo\src",
        &[r"C:\repo", r"\\?\c:\REPO\", r"\\.\C:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        1,
    );

    let relation = turn_relation(
        &CodexTurnKey::new(thread("thread-a"), "turn-1"),
        Some(&observation),
        None,
        None,
    );

    assert_eq!(relation.state, WorkspaceResolutionState::Assigned);
    assert_eq!(relation.candidate_workspace_keys.len(), 1);
}

#[test]
fn later_observation_does_not_rewrite_historical_turn_relation() {
    let thread_key = thread("thread-a");
    let first = observed_cwd(
        r"C:\repo",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        1,
    );
    let later = observed_cwd(
        r"F:\repo",
        &[r"C:\repo", r"F:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );
    let first_key =
        ThreadWorkspaceRelationKey::turn(CodexTurnKey::new(thread_key.clone(), "turn-1"));
    let mut store = ThreadWorkspaceRelationStore::default();
    store.observe(turn_relation(
        &CodexTurnKey::new(thread_key.clone(), "turn-1"),
        Some(&first),
        None,
        None,
    ));
    let historical = store.current(&first_key).unwrap().clone();

    store.observe(turn_relation(
        &CodexTurnKey::new(thread_key, "turn-2"),
        Some(&later),
        None,
        None,
    ));

    assert_eq!(store.current(&first_key), Some(&historical));
}

#[test]
fn direct_evidence_supersedes_fallback_without_erasing_observation_history() {
    let parent_cwd = observed_cwd(
        r"C:\parent",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let parent_relation = origin_relation(&thread("parent"), Some(&parent_cwd), None, None);
    let turn_key = CodexTurnKey::new(thread("child"), "turn-1");
    let relation_key = ThreadWorkspaceRelationKey::turn(turn_key.clone());
    let fallback = turn_relation(
        &turn_key,
        None,
        None,
        Some(ParentWorkspaceFallback::confirmed(&parent_relation)),
    );
    let direct_cwd = observed_cwd(
        r"F:\child",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::TurnContextCwd,
        2,
    );
    let direct = turn_relation(
        &turn_key,
        None,
        Some(&direct_cwd),
        Some(ParentWorkspaceFallback::confirmed(&parent_relation)),
    );
    let mut store = ThreadWorkspaceRelationStore::default();

    assert!(store.observe(fallback));
    assert!(store.observe(direct));

    assert_eq!(store.history(&relation_key).len(), 2);
    assert_eq!(
        store.history(&relation_key)[0].basis,
        ThreadWorkspaceRelationBasis::ParentFallback
    );
    assert_eq!(
        store.current(&relation_key).unwrap().basis,
        ThreadWorkspaceRelationBasis::DirectCwd
    );
}

#[test]
fn effective_relation_uses_evidence_precedence_not_observation_order() {
    let parent_cwd = observed_cwd(
        r"C:\parent",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let parent_relation = origin_relation(&thread("parent"), Some(&parent_cwd), None, None);
    let turn_key = CodexTurnKey::new(thread("child"), "turn-1");
    let relation_key = ThreadWorkspaceRelationKey::turn(turn_key.clone());
    let direct_cwd = observed_cwd(
        r"F:\child",
        &[r"C:\parent", r"F:\child"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );
    let direct = turn_relation(&turn_key, Some(&direct_cwd), None, None);
    let fallback = turn_relation(
        &turn_key,
        None,
        None,
        Some(ParentWorkspaceFallback::confirmed(&parent_relation)),
    );
    let mut store = ThreadWorkspaceRelationStore::default();

    assert!(store.observe(direct));
    assert!(store.observe(fallback));

    assert_eq!(store.history(&relation_key).len(), 2);
    assert_eq!(
        store.history(&relation_key).last().unwrap().basis,
        ThreadWorkspaceRelationBasis::ParentFallback
    );
    assert_eq!(
        store.current(&relation_key).unwrap().basis,
        ThreadWorkspaceRelationBasis::DirectCwd
    );
}

#[test]
fn effective_relation_tie_break_is_independent_from_observation_order() {
    let turn_key = CodexTurnKey::new(thread("thread-a"), "turn-1");
    let relation_key = ThreadWorkspaceRelationKey::turn(turn_key.clone());
    let workspace_a = turn_relation(
        &turn_key,
        Some(&observed_cwd(
            r"C:\repo",
            &[r"C:\repo", r"F:\repo"],
            ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
            1,
        )),
        None,
        None,
    );
    let workspace_b = turn_relation(
        &turn_key,
        Some(&observed_cwd(
            r"F:\repo",
            &[r"C:\repo", r"F:\repo"],
            ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
            1,
        )),
        None,
        None,
    );
    let mut forward = ThreadWorkspaceRelationStore::default();
    let mut reverse = ThreadWorkspaceRelationStore::default();

    assert!(forward.observe(workspace_a.clone()));
    assert!(forward.observe(workspace_b.clone()));
    assert!(reverse.observe(workspace_b));
    assert!(reverse.observe(workspace_a));

    assert_eq!(
        forward.current(&relation_key),
        reverse.current(&relation_key)
    );
}

#[test]
fn repeated_relation_observation_is_idempotent() {
    let cwd = observed_cwd(
        r"C:\repo",
        &[r"C:\repo"],
        ThreadWorkspaceProvenanceKind::SessionMetaCwd,
        1,
    );
    let relation = origin_relation(&thread("thread-a"), None, Some(&cwd), None);
    let key = relation.key.clone();
    let mut store = ThreadWorkspaceRelationStore::default();

    assert!(store.observe(relation.clone()));
    assert!(!store.observe(relation));
    assert_eq!(store.history(&key).len(), 1);
}

#[test]
fn thread_identity_is_independent_from_workspace_relation() {
    let thread_key = thread("same-full-thread-id");
    let origin_cwd = observed_cwd(
        r"C:\origin",
        &[r"C:\origin", r"F:\turn"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let turn_cwd = observed_cwd(
        r"F:\turn",
        &[r"C:\origin", r"F:\turn"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );
    let origin = origin_relation(&thread_key, Some(&origin_cwd), None, None);
    let turn = turn_relation(
        &CodexTurnKey::new(thread_key.clone(), "turn-1"),
        Some(&turn_cwd),
        None,
        None,
    );

    assert_eq!(origin.key.thread_key, thread_key);
    assert_eq!(turn.key.thread_key, thread_key);
    assert_ne!(origin.workspace_key(), turn.workspace_key());
}

fn runtime_reconciler() -> RuntimeWorkspaceReconciler {
    RuntimeWorkspaceReconciler::new(
        "codex-home-fixture",
        environment("windows-host-a"),
        RootLocatorPlatform::Windows,
    )
}

#[test]
fn runtime_unique_root_routes_to_correct_workspace_entry() {
    let mut runtime = runtime_reconciler();
    runtime.register_workspace("workspace-a", r"C:\repo");

    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-a",
        thread_start_cwd: Some(r"C:\repo\src"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });

    assert_eq!(route.state, WorkspaceResolutionState::Assigned);
    assert_eq!(route.workspace_id.as_deref(), Some("workspace-a"));
}

#[test]
fn runtime_nested_roots_choose_longest_workspace_entry() {
    let mut runtime = runtime_reconciler();
    runtime.register_workspace("workspace-parent", r"C:\repo");
    runtime.register_workspace("workspace-child", r"C:\repo\nested");

    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-a",
        thread_start_cwd: Some(r"C:\repo\nested\src"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });

    assert_eq!(route.workspace_id.as_deref(), Some("workspace-child"));
}

#[test]
fn runtime_duplicate_config_for_same_workspace_key_is_assigned_deterministically() {
    let mut forward = runtime_reconciler();
    forward.register_workspace("workspace-b", r"C:\repo");
    forward.register_workspace("workspace-a", r"c:/REPO/");
    let mut reverse = runtime_reconciler();
    reverse.register_workspace("workspace-a", r"c:/REPO/");
    reverse.register_workspace("workspace-b", r"C:\repo");

    let observation = RuntimeOriginWorkspaceObservation {
        thread_id: "thread-a",
        thread_start_cwd: None,
        session_meta_cwd: Some(r"C:\repo"),
        confirmed_parent_thread_id: None,
        observed_at: 1,
    };
    let forward_route = forward.observe_origin(observation);
    let reverse_route = reverse.observe_origin(observation);

    assert_eq!(forward_route.state, WorkspaceResolutionState::Assigned);
    assert_eq!(forward_route.workspace_id.as_deref(), Some("workspace-a"));
    assert_eq!(forward_route, reverse_route);
}

#[test]
fn runtime_ambiguous_unassigned_and_unknown_relations_never_route() {
    let mut runtime = runtime_reconciler();
    runtime.register_workspace("workspace-a", r"C:\repo");
    let thread_key = CodexThreadKey::new("codex-home-fixture", "thread-a");
    let locator = NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();
    let key_a = WorkspaceKey::new(environment("windows-host-a"), locator.clone());
    let key_b = WorkspaceKey::new(environment("windows-host-b"), locator);
    let ambiguous = super::resolver::finalize_workspace_candidates(vec![(1, key_a), (1, key_b)]);
    let ambiguous_relation = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key: &thread_key,
        thread_start: Some(&WorkspaceRelationObservation::from_resolution(
            ambiguous,
            ThreadWorkspaceProvenanceKind::ThreadStartCwd,
            Some(r"C:\repo".to_string()),
            1,
        )),
        session_meta: None,
        parent_fallback: None,
        observed_at: 1,
    });

    let ambiguous_route = runtime.observe_relation(ambiguous_relation);
    let unassigned_route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-unassigned",
        thread_start_cwd: Some(r"F:\outside"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let unknown_route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-unknown",
        thread_start_cwd: Some("relative/path"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });

    assert_eq!(ambiguous_route.state, WorkspaceResolutionState::Ambiguous);
    assert!(ambiguous_route.workspace_id.is_none());
    assert_eq!(unassigned_route.state, WorkspaceResolutionState::Unassigned);
    assert!(unassigned_route.workspace_id.is_none());
    assert_eq!(unknown_route.state, WorkspaceResolutionState::Unknown);
    assert!(unknown_route.workspace_id.is_none());
}

#[test]
fn runtime_origin_and_turn_execution_relations_remain_independent() {
    let mut runtime = runtime_reconciler();
    runtime.register_workspace("workspace-a", r"C:\origin");
    runtime.register_workspace("workspace-b", r"F:\turn");

    runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-a",
        thread_start_cwd: Some(r"C:\origin"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    runtime.observe_turn(RuntimeTurnWorkspaceObservation {
        thread_id: "thread-a",
        turn_id: "turn-1",
        explicit_turn_cwd: Some(r"C:\origin"),
        turn_context_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 2,
    });
    runtime.observe_turn(RuntimeTurnWorkspaceObservation {
        thread_id: "thread-a",
        turn_id: "turn-2",
        explicit_turn_cwd: Some(r"F:\turn"),
        turn_context_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 3,
    });

    assert_eq!(
        runtime
            .route_for_origin("thread-a")
            .unwrap()
            .workspace_id
            .as_deref(),
        Some("workspace-a")
    );
    assert_eq!(
        runtime
            .route_for_turn("thread-a", "turn-1")
            .unwrap()
            .workspace_id
            .as_deref(),
        Some("workspace-a")
    );
    assert_eq!(
        runtime
            .route_for_turn("thread-a", "turn-2")
            .unwrap()
            .workspace_id
            .as_deref(),
        Some("workspace-b")
    );
}

#[test]
fn runtime_child_fallback_requires_confirmed_assigned_parent() {
    let mut runtime = runtime_reconciler();
    runtime.register_workspace("workspace-a", r"C:\repo");
    runtime.register_workspace("workspace-b", r"F:\direct");
    runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "parent",
        thread_start_cwd: Some(r"C:\repo"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });

    let inherited = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "child",
        thread_start_cwd: None,
        session_meta_cwd: None,
        confirmed_parent_thread_id: Some("parent"),
        observed_at: 2,
    });
    let unknown = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "orphan",
        thread_start_cwd: None,
        session_meta_cwd: None,
        confirmed_parent_thread_id: Some("missing-parent"),
        observed_at: 2,
    });
    let direct = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "child-direct",
        thread_start_cwd: Some(r"F:\direct"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: Some("parent"),
        observed_at: 3,
    });

    let ambiguous_parent_key = CodexThreadKey::new("codex-home-fixture", "ambiguous-parent");
    let locator = NormalizedRootLocator::parse(r"C:\repo", RootLocatorPlatform::Windows).unwrap();
    let ambiguous = super::resolver::finalize_workspace_candidates(vec![
        (
            1,
            WorkspaceKey::new(environment("windows-host-a"), locator.clone()),
        ),
        (1, WorkspaceKey::new(environment("windows-host-b"), locator)),
    ]);
    runtime.observe_relation(resolve_origin_workspace_relation(
        &OriginWorkspaceRelationInput {
            thread_key: &ambiguous_parent_key,
            thread_start: Some(&WorkspaceRelationObservation::from_resolution(
                ambiguous,
                ThreadWorkspaceProvenanceKind::ThreadStartCwd,
                Some(r"C:\repo".to_string()),
                1,
            )),
            session_meta: None,
            parent_fallback: None,
            observed_at: 1,
        },
    ));
    let ambiguous_parent_fallback = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "ambiguous-child",
        thread_start_cwd: None,
        session_meta_cwd: None,
        confirmed_parent_thread_id: Some("ambiguous-parent"),
        observed_at: 3,
    });

    assert_eq!(inherited.workspace_id.as_deref(), Some("workspace-a"));
    assert_eq!(
        inherited.basis,
        ThreadWorkspaceRelationBasis::ParentFallback
    );
    assert_eq!(unknown.state, WorkspaceResolutionState::Unknown);
    assert!(unknown.workspace_id.is_none());
    assert_eq!(direct.workspace_id.as_deref(), Some("workspace-b"));
    assert_eq!(direct.basis, ThreadWorkspaceRelationBasis::DirectCwd);
    assert_eq!(
        ambiguous_parent_fallback.state,
        WorkspaceResolutionState::Unknown
    );
    assert!(ambiguous_parent_fallback.workspace_id.is_none());
}

#[test]
fn runtime_reconstruction_and_repeated_observations_are_deterministic() {
    let mut first = runtime_reconciler();
    first.register_workspace("workspace-b", r"C:\repo");
    first.register_workspace("workspace-a", r"c:/REPO/");
    let mut second = runtime_reconciler();
    second.register_workspace("workspace-a", r"c:/REPO/");
    second.register_workspace("workspace-b", r"C:\repo");
    let observation = RuntimeOriginWorkspaceObservation {
        thread_id: "thread-a",
        thread_start_cwd: None,
        session_meta_cwd: Some(r"C:\repo"),
        confirmed_parent_thread_id: None,
        observed_at: 7,
    };

    let first_route = first.observe_origin(observation);
    let repeated_route = first.observe_origin(observation);
    let second_route = second.observe_origin(observation);

    assert_eq!(first_route, repeated_route);
    assert_eq!(first_route, second_route);
    assert_eq!(first.history_len_for_origin("thread-a"), 1);
}

#[test]
fn unknown_relation_retains_the_observation_time() {
    let relation = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
        thread_key: &thread("thread-a"),
        thread_start: None,
        session_meta: None,
        parent_fallback: None,
        observed_at: 42,
    });

    assert_eq!(relation.state, WorkspaceResolutionState::Unknown);
    assert_eq!(relation.observed_at, 42);
}

#[test]
fn scope_specific_cwd_evidence_cannot_cross_relation_boundaries() {
    let origin_only = observed_cwd(
        r"C:\repo",
        &[r"C:\repo"],
        ThreadWorkspaceProvenanceKind::ThreadStartCwd,
        1,
    );
    let turn_only = observed_cwd(
        r"C:\repo",
        &[r"C:\repo"],
        ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
        2,
    );

    let turn_relation = turn_relation(
        &CodexTurnKey::new(thread("thread-a"), "turn-1"),
        Some(&origin_only),
        None,
        None,
    );
    let origin_relation = origin_relation(&thread("thread-a"), None, Some(&turn_only), None);

    assert_eq!(turn_relation.state, WorkspaceResolutionState::Unknown);
    assert_eq!(origin_relation.state, WorkspaceResolutionState::Unknown);
}

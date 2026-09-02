use super::*;

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

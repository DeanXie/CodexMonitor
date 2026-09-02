use super::{ExecutionEnvironmentKey, NormalizedRootLocator, RootLocatorPlatform, WorkspaceKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfiguredWorkspaceRoot {
    locator: String,
}

impl ConfiguredWorkspaceRoot {
    pub(crate) fn new(locator: impl Into<String>) -> Self {
        Self {
            locator: locator.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceResolutionState {
    Assigned,
    Ambiguous,
    Unassigned,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhysicalIdentityStatus {
    Inferred,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceResolution {
    pub state: WorkspaceResolutionState,
    pub workspace_key: Option<WorkspaceKey>,
    pub candidate_workspace_keys: Vec<WorkspaceKey>,
    pub physical_identity: PhysicalIdentityStatus,
}

pub(crate) struct WorkspaceResolutionInput<'a> {
    pub cwd: &'a str,
    pub platform: RootLocatorPlatform,
    pub execution_environment_key: &'a ExecutionEnvironmentKey,
    pub configured_roots: &'a [ConfiguredWorkspaceRoot],
}

pub(crate) fn resolve_workspace_root(input: &WorkspaceResolutionInput<'_>) -> WorkspaceResolution {
    let Ok(cwd) = NormalizedRootLocator::parse(input.cwd, input.platform) else {
        return unknown();
    };

    let mut matches = Vec::new();
    for configured_root in input.configured_roots {
        let Ok(root) = NormalizedRootLocator::parse(&configured_root.locator, input.platform)
        else {
            return unknown();
        };
        if root.contains(&cwd) {
            matches.push((
                root.component_count(),
                WorkspaceKey::new(input.execution_environment_key.clone(), root),
            ));
        }
    }

    finalize_workspace_candidates(matches)
}

pub(super) fn finalize_workspace_candidates(
    matches: Vec<(usize, WorkspaceKey)>,
) -> WorkspaceResolution {
    let Some(longest) = matches.iter().map(|(length, _)| *length).max() else {
        return WorkspaceResolution {
            state: WorkspaceResolutionState::Unassigned,
            workspace_key: None,
            candidate_workspace_keys: Vec::new(),
            physical_identity: PhysicalIdentityStatus::Unresolved,
        };
    };

    let mut candidates = matches
        .into_iter()
        .filter_map(|(length, key)| (length == longest).then_some(key))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();

    if candidates.len() == 1 {
        return WorkspaceResolution {
            state: WorkspaceResolutionState::Assigned,
            workspace_key: candidates.first().cloned(),
            candidate_workspace_keys: candidates,
            physical_identity: PhysicalIdentityStatus::Unresolved,
        };
    }

    WorkspaceResolution {
        state: WorkspaceResolutionState::Ambiguous,
        workspace_key: None,
        candidate_workspace_keys: candidates,
        physical_identity: PhysicalIdentityStatus::Unresolved,
    }
}

fn unknown() -> WorkspaceResolution {
    WorkspaceResolution {
        state: WorkspaceResolutionState::Unknown,
        workspace_key: None,
        candidate_workspace_keys: Vec::new(),
        physical_identity: PhysicalIdentityStatus::Unresolved,
    }
}

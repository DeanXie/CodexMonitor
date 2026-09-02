use std::collections::HashMap;

use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};

use super::{
    resolve_workspace_root, ConfiguredWorkspaceRoot, ExecutionEnvironmentKey, RootLocatorPlatform,
    WorkspaceKey, WorkspaceResolution, WorkspaceResolutionInput, WorkspaceResolutionState,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ThreadWorkspaceRelationScope {
    Origin,
    TurnExecution { turn_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ThreadWorkspaceRelationKey {
    pub thread_key: CodexThreadKey,
    pub scope: ThreadWorkspaceRelationScope,
}

impl ThreadWorkspaceRelationKey {
    pub(crate) fn origin(thread_key: CodexThreadKey) -> Self {
        Self {
            thread_key,
            scope: ThreadWorkspaceRelationScope::Origin,
        }
    }

    pub(crate) fn turn(turn_key: CodexTurnKey) -> Self {
        Self {
            thread_key: turn_key.thread_key,
            scope: ThreadWorkspaceRelationScope::TurnExecution {
                turn_id: turn_key.turn_id,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThreadWorkspaceRelationBasis {
    DirectCwd,
    ParentFallback,
    InsufficientEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ThreadWorkspaceRelationConfidence {
    Direct,
    Inferred,
    Insufficient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ThreadWorkspaceProvenanceKind {
    ThreadStartCwd,
    SessionMetaCwd,
    ExplicitTurnCwd,
    TurnContextCwd,
    ConfirmedParentWorkspace,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ThreadWorkspaceProvenance {
    pub kind: ThreadWorkspaceProvenanceKind,
    pub locator: Option<String>,
    pub observed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceRelationObservation {
    resolution: WorkspaceResolution,
    provenance: ThreadWorkspaceProvenance,
}

impl WorkspaceRelationObservation {
    pub(crate) fn from_cwd(
        cwd: impl Into<String>,
        platform: RootLocatorPlatform,
        execution_environment_key: ExecutionEnvironmentKey,
        configured_roots: Vec<ConfiguredWorkspaceRoot>,
        source: ThreadWorkspaceProvenanceKind,
        observed_at: u64,
    ) -> Self {
        let cwd = cwd.into();
        let resolution = resolve_workspace_root(&WorkspaceResolutionInput {
            cwd: &cwd,
            platform,
            execution_environment_key: &execution_environment_key,
            configured_roots: &configured_roots,
        });
        Self::from_resolution(resolution, source, Some(cwd), observed_at)
    }

    pub(crate) fn from_resolution(
        resolution: WorkspaceResolution,
        source: ThreadWorkspaceProvenanceKind,
        locator: Option<String>,
        observed_at: u64,
    ) -> Self {
        Self {
            resolution,
            provenance: ThreadWorkspaceProvenance {
                kind: source,
                locator,
                observed_at,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ThreadWorkspaceRelation {
    pub key: ThreadWorkspaceRelationKey,
    pub workspace_key: Option<WorkspaceKey>,
    pub state: WorkspaceResolutionState,
    pub basis: ThreadWorkspaceRelationBasis,
    pub provenance: Vec<ThreadWorkspaceProvenance>,
    pub confidence: ThreadWorkspaceRelationConfidence,
    pub candidate_workspace_keys: Vec<WorkspaceKey>,
    pub observed_at: u64,
}

impl ThreadWorkspaceRelation {
    pub(crate) fn workspace_key(&self) -> Option<&WorkspaceKey> {
        self.workspace_key.as_ref()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ParentWorkspaceFallback<'a> {
    confirmed_parent_edge: bool,
    parent_relation: &'a ThreadWorkspaceRelation,
}

impl<'a> ParentWorkspaceFallback<'a> {
    pub(crate) fn confirmed(parent_relation: &'a ThreadWorkspaceRelation) -> Self {
        Self {
            confirmed_parent_edge: true,
            parent_relation,
        }
    }

    pub(crate) fn unconfirmed(parent_relation: &'a ThreadWorkspaceRelation) -> Self {
        Self {
            confirmed_parent_edge: false,
            parent_relation,
        }
    }
}

pub(crate) struct OriginWorkspaceRelationInput<'a> {
    pub thread_key: &'a CodexThreadKey,
    pub thread_start: Option<&'a WorkspaceRelationObservation>,
    pub session_meta: Option<&'a WorkspaceRelationObservation>,
    pub parent_fallback: Option<ParentWorkspaceFallback<'a>>,
    pub observed_at: u64,
}

pub(crate) struct TurnExecutionWorkspaceRelationInput<'a> {
    pub turn_key: &'a CodexTurnKey,
    pub explicit_turn: Option<&'a WorkspaceRelationObservation>,
    pub turn_context: Option<&'a WorkspaceRelationObservation>,
    pub parent_fallback: Option<ParentWorkspaceFallback<'a>>,
    pub observed_at: u64,
}

pub(crate) fn resolve_origin_workspace_relation(
    input: &OriginWorkspaceRelationInput<'_>,
) -> ThreadWorkspaceRelation {
    let key = ThreadWorkspaceRelationKey::origin(input.thread_key.clone());
    let direct = input
        .thread_start
        .filter(|observation| {
            observation.provenance.kind == ThreadWorkspaceProvenanceKind::ThreadStartCwd
        })
        .or_else(|| {
            input.session_meta.filter(|observation| {
                observation.provenance.kind == ThreadWorkspaceProvenanceKind::SessionMetaCwd
            })
        });
    resolve_scoped_relation(key, direct, input.parent_fallback, input.observed_at)
}

pub(crate) fn resolve_turn_execution_workspace_relation(
    input: &TurnExecutionWorkspaceRelationInput<'_>,
) -> ThreadWorkspaceRelation {
    let key = ThreadWorkspaceRelationKey::turn(input.turn_key.clone());
    let direct = input
        .explicit_turn
        .filter(|observation| {
            observation.provenance.kind == ThreadWorkspaceProvenanceKind::ExplicitTurnCwd
        })
        .or_else(|| {
            input.turn_context.filter(|observation| {
                observation.provenance.kind == ThreadWorkspaceProvenanceKind::TurnContextCwd
            })
        });
    resolve_scoped_relation(key, direct, input.parent_fallback, input.observed_at)
}

fn resolve_scoped_relation(
    key: ThreadWorkspaceRelationKey,
    direct: Option<&WorkspaceRelationObservation>,
    parent_fallback: Option<ParentWorkspaceFallback<'_>>,
    observed_at: u64,
) -> ThreadWorkspaceRelation {
    if let Some(direct) = direct {
        return relation_from_resolution(
            key,
            &direct.resolution,
            ThreadWorkspaceRelationBasis::DirectCwd,
            ThreadWorkspaceRelationConfidence::Direct,
            vec![direct.provenance.clone()],
            direct.provenance.observed_at,
        );
    }

    if let Some(fallback) = parent_fallback.filter(|fallback| fallback.confirmed_parent_edge) {
        if let Some(workspace_key) = fallback.parent_relation.workspace_key.clone() {
            let mut provenance = fallback.parent_relation.provenance.clone();
            provenance.push(ThreadWorkspaceProvenance {
                kind: ThreadWorkspaceProvenanceKind::ConfirmedParentWorkspace,
                locator: None,
                observed_at,
            });
            return ThreadWorkspaceRelation {
                key,
                workspace_key: Some(workspace_key.clone()),
                state: WorkspaceResolutionState::Assigned,
                basis: ThreadWorkspaceRelationBasis::ParentFallback,
                provenance,
                confidence: ThreadWorkspaceRelationConfidence::Inferred,
                candidate_workspace_keys: vec![workspace_key],
                observed_at,
            };
        }
    }

    ThreadWorkspaceRelation {
        key,
        workspace_key: None,
        state: WorkspaceResolutionState::Unknown,
        basis: ThreadWorkspaceRelationBasis::InsufficientEvidence,
        provenance: Vec::new(),
        confidence: ThreadWorkspaceRelationConfidence::Insufficient,
        candidate_workspace_keys: Vec::new(),
        observed_at,
    }
}

fn relation_from_resolution(
    key: ThreadWorkspaceRelationKey,
    resolution: &WorkspaceResolution,
    basis: ThreadWorkspaceRelationBasis,
    confidence: ThreadWorkspaceRelationConfidence,
    provenance: Vec<ThreadWorkspaceProvenance>,
    observed_at: u64,
) -> ThreadWorkspaceRelation {
    ThreadWorkspaceRelation {
        key,
        workspace_key: resolution.workspace_key.clone(),
        state: resolution.state,
        basis,
        provenance,
        confidence,
        candidate_workspace_keys: resolution.candidate_workspace_keys.clone(),
        observed_at,
    }
}

#[derive(Default)]
pub(crate) struct ThreadWorkspaceRelationStore {
    history_by_key: HashMap<ThreadWorkspaceRelationKey, Vec<ThreadWorkspaceRelation>>,
}

impl ThreadWorkspaceRelationStore {
    pub(crate) fn observe(&mut self, relation: ThreadWorkspaceRelation) -> bool {
        let history = self.history_by_key.entry(relation.key.clone()).or_default();
        if history.contains(&relation) {
            return false;
        }
        history.push(relation);
        true
    }

    pub(crate) fn history(&self, key: &ThreadWorkspaceRelationKey) -> &[ThreadWorkspaceRelation] {
        self.history_by_key
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn current(
        &self,
        key: &ThreadWorkspaceRelationKey,
    ) -> Option<&ThreadWorkspaceRelation> {
        self.history(key).iter().max_by(|left, right| {
            relation_effective_precedence(left)
                .cmp(&relation_effective_precedence(right))
                .then_with(|| {
                    relation_deterministic_tie_break(left)
                        .cmp(&relation_deterministic_tie_break(right))
                })
        })
    }

    pub(crate) fn relations_for_thread(
        &self,
        thread_key: &CodexThreadKey,
    ) -> Vec<&ThreadWorkspaceRelation> {
        let mut relations = self
            .history_by_key
            .keys()
            .filter(|key| &key.thread_key == thread_key)
            .filter_map(|key| self.current(key))
            .collect::<Vec<_>>();
        relations.sort_by(|left, right| {
            relation_scope_sort_key(&left.key.scope).cmp(&relation_scope_sort_key(&right.key.scope))
        });
        relations
    }
}

fn relation_effective_precedence(relation: &ThreadWorkspaceRelation) -> (u8, u8, u64) {
    (
        relation_basis_precedence(relation.basis),
        relation_source_precedence(relation),
        relation.observed_at,
    )
}

fn relation_deterministic_tie_break(
    relation: &ThreadWorkspaceRelation,
) -> (
    u8,
    Option<&WorkspaceKey>,
    &[WorkspaceKey],
    ThreadWorkspaceRelationConfidence,
    &[ThreadWorkspaceProvenance],
) {
    (
        relation_state_sort_key(relation.state),
        relation.workspace_key.as_ref(),
        relation.candidate_workspace_keys.as_slice(),
        relation.confidence,
        relation.provenance.as_slice(),
    )
}

fn relation_state_sort_key(state: WorkspaceResolutionState) -> u8 {
    match state {
        WorkspaceResolutionState::Assigned => 0,
        WorkspaceResolutionState::Ambiguous => 1,
        WorkspaceResolutionState::Unassigned => 2,
        WorkspaceResolutionState::Unknown => 3,
    }
}

fn relation_basis_precedence(basis: ThreadWorkspaceRelationBasis) -> u8 {
    match basis {
        ThreadWorkspaceRelationBasis::InsufficientEvidence => 0,
        ThreadWorkspaceRelationBasis::ParentFallback => 1,
        ThreadWorkspaceRelationBasis::DirectCwd => 2,
    }
}

fn relation_source_precedence(relation: &ThreadWorkspaceRelation) -> u8 {
    relation
        .provenance
        .first()
        .map(|provenance| match provenance.kind {
            ThreadWorkspaceProvenanceKind::ConfirmedParentWorkspace => 0,
            ThreadWorkspaceProvenanceKind::SessionMetaCwd => 1,
            ThreadWorkspaceProvenanceKind::TurnContextCwd => 1,
            ThreadWorkspaceProvenanceKind::ThreadStartCwd => 2,
            ThreadWorkspaceProvenanceKind::ExplicitTurnCwd => 2,
        })
        .unwrap_or(0)
}

fn relation_scope_sort_key(scope: &ThreadWorkspaceRelationScope) -> (u8, &str) {
    match scope {
        ThreadWorkspaceRelationScope::Origin => (0, ""),
        ThreadWorkspaceRelationScope::TurnExecution { turn_id } => (1, turn_id.as_str()),
    }
}

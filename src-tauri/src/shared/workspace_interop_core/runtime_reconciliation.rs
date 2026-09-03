use std::collections::BTreeMap;

use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};

use super::{
    resolve_origin_workspace_relation, resolve_turn_execution_workspace_relation,
    ConfiguredWorkspaceRoot, ExecutionEnvironmentKey, OriginWorkspaceRelationInput,
    ParentWorkspaceFallback, RootLocatorPlatform, ThreadWorkspaceProvenanceKind,
    ThreadWorkspaceRelation, ThreadWorkspaceRelationBasis, ThreadWorkspaceRelationKey,
    ThreadWorkspaceRelationStore, TurnExecutionWorkspaceRelationInput, WorkspaceKey,
    WorkspaceRelationObservation, WorkspaceResolutionState,
};

#[derive(Clone, Copy)]
pub(crate) struct RuntimeOriginWorkspaceObservation<'a> {
    pub thread_id: &'a str,
    pub thread_start_cwd: Option<&'a str>,
    pub session_meta_cwd: Option<&'a str>,
    pub confirmed_parent_thread_id: Option<&'a str>,
    pub observed_at: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeTurnWorkspaceObservation<'a> {
    pub thread_id: &'a str,
    pub turn_id: &'a str,
    pub explicit_turn_cwd: Option<&'a str>,
    pub turn_context_cwd: Option<&'a str>,
    pub confirmed_parent_thread_id: Option<&'a str>,
    pub observed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeWorkspaceRoute {
    pub state: WorkspaceResolutionState,
    pub workspace_id: Option<String>,
    pub workspace_key: Option<WorkspaceKey>,
    pub basis: ThreadWorkspaceRelationBasis,
}

pub(crate) struct RuntimeWorkspaceReconciler {
    codex_home_identity: String,
    execution_environment_key: ExecutionEnvironmentKey,
    platform: RootLocatorPlatform,
    workspace_roots: BTreeMap<String, String>,
    relations: ThreadWorkspaceRelationStore,
}

impl RuntimeWorkspaceReconciler {
    /// The already-resolved runtime namespace, shared with creation acknowledgement.
    pub(crate) fn codex_home_identity(&self) -> &str {
        &self.codex_home_identity
    }

    pub(crate) fn new(
        codex_home_identity: impl Into<String>,
        execution_environment_key: ExecutionEnvironmentKey,
        platform: RootLocatorPlatform,
    ) -> Self {
        Self {
            codex_home_identity: codex_home_identity.into(),
            execution_environment_key,
            platform,
            workspace_roots: BTreeMap::new(),
            relations: ThreadWorkspaceRelationStore::default(),
        }
    }

    pub(crate) fn register_workspace(
        &mut self,
        workspace_id: impl Into<String>,
        root_locator: impl Into<String>,
    ) {
        self.workspace_roots
            .insert(workspace_id.into(), root_locator.into());
    }

    pub(crate) fn unregister_workspace(&mut self, workspace_id: &str) {
        self.workspace_roots.remove(workspace_id);
    }

    pub(crate) fn observe_origin(
        &mut self,
        input: RuntimeOriginWorkspaceObservation<'_>,
    ) -> RuntimeWorkspaceRoute {
        let thread_key = self.thread_key(input.thread_id);
        let thread_start = input.thread_start_cwd.map(|cwd| {
            self.cwd_observation(
                cwd,
                ThreadWorkspaceProvenanceKind::ThreadStartCwd,
                input.observed_at,
            )
        });
        let session_meta = input.session_meta_cwd.map(|cwd| {
            self.cwd_observation(
                cwd,
                ThreadWorkspaceProvenanceKind::SessionMetaCwd,
                input.observed_at,
            )
        });
        let parent_relation = input
            .confirmed_parent_thread_id
            .and_then(|parent_id| self.effective_origin_relation(parent_id).cloned());
        let relation = resolve_origin_workspace_relation(&OriginWorkspaceRelationInput {
            thread_key: &thread_key,
            thread_start: thread_start.as_ref(),
            session_meta: session_meta.as_ref(),
            parent_fallback: parent_relation
                .as_ref()
                .map(ParentWorkspaceFallback::confirmed),
            observed_at: input.observed_at,
        });
        self.observe_relation(relation)
    }

    pub(crate) fn observe_turn(
        &mut self,
        input: RuntimeTurnWorkspaceObservation<'_>,
    ) -> RuntimeWorkspaceRoute {
        let turn_key = CodexTurnKey::new(self.thread_key(input.thread_id), input.turn_id);
        let explicit_turn = input.explicit_turn_cwd.map(|cwd| {
            self.cwd_observation(
                cwd,
                ThreadWorkspaceProvenanceKind::ExplicitTurnCwd,
                input.observed_at,
            )
        });
        let turn_context = input.turn_context_cwd.map(|cwd| {
            self.cwd_observation(
                cwd,
                ThreadWorkspaceProvenanceKind::TurnContextCwd,
                input.observed_at,
            )
        });
        let parent_relation = input
            .confirmed_parent_thread_id
            .and_then(|parent_id| self.effective_origin_relation(parent_id).cloned());
        let relation =
            resolve_turn_execution_workspace_relation(&TurnExecutionWorkspaceRelationInput {
                turn_key: &turn_key,
                explicit_turn: explicit_turn.as_ref(),
                turn_context: turn_context.as_ref(),
                parent_fallback: parent_relation
                    .as_ref()
                    .map(ParentWorkspaceFallback::confirmed),
                observed_at: input.observed_at,
            });
        self.observe_relation(relation)
    }

    pub(crate) fn observe_relation(
        &mut self,
        relation: ThreadWorkspaceRelation,
    ) -> RuntimeWorkspaceRoute {
        let key = relation.key.clone();
        self.relations.observe(relation);
        self.route_for_key(&key)
            .expect("an observed relation must have an effective value")
    }

    pub(crate) fn route_for_origin(&self, thread_id: &str) -> Option<RuntimeWorkspaceRoute> {
        self.route_for_key(&ThreadWorkspaceRelationKey::origin(
            self.thread_key(thread_id),
        ))
    }

    pub(crate) fn route_for_turn(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Option<RuntimeWorkspaceRoute> {
        self.route_for_key(&ThreadWorkspaceRelationKey::turn(CodexTurnKey::new(
            self.thread_key(thread_id),
            turn_id,
        )))
    }

    pub(crate) fn history_len_for_origin(&self, thread_id: &str) -> usize {
        self.relations
            .history(&ThreadWorkspaceRelationKey::origin(
                self.thread_key(thread_id),
            ))
            .len()
    }

    fn cwd_observation(
        &self,
        cwd: &str,
        source: ThreadWorkspaceProvenanceKind,
        observed_at: u64,
    ) -> WorkspaceRelationObservation {
        WorkspaceRelationObservation::from_cwd(
            cwd,
            self.platform,
            self.execution_environment_key.clone(),
            self.workspace_roots
                .values()
                .cloned()
                .map(ConfiguredWorkspaceRoot::new)
                .collect(),
            source,
            observed_at,
        )
    }

    fn thread_key(&self, thread_id: &str) -> CodexThreadKey {
        CodexThreadKey::new(self.codex_home_identity.clone(), thread_id)
    }

    fn route_for_key(&self, key: &ThreadWorkspaceRelationKey) -> Option<RuntimeWorkspaceRoute> {
        self.relations
            .current(key)
            .map(|relation| self.route_for_relation(relation))
    }

    fn route_for_relation(&self, relation: &ThreadWorkspaceRelation) -> RuntimeWorkspaceRoute {
        let workspace_id = if relation.state == WorkspaceResolutionState::Assigned {
            relation
                .workspace_key()
                .and_then(|workspace_key| self.workspace_id_for_key(workspace_key))
        } else {
            None
        };
        RuntimeWorkspaceRoute {
            state: relation.state,
            workspace_id,
            workspace_key: relation.workspace_key.clone(),
            basis: relation.basis,
        }
    }

    fn workspace_id_for_key(&self, workspace_key: &WorkspaceKey) -> Option<String> {
        self.workspace_roots
            .iter()
            .filter_map(|(workspace_id, root)| {
                let normalized = super::NormalizedRootLocator::parse(root, self.platform).ok()?;
                let candidate =
                    WorkspaceKey::new(self.execution_environment_key.clone(), normalized);
                (candidate == *workspace_key).then_some(workspace_id)
            })
            .min()
            .cloned()
    }

    fn effective_origin_relation(&self, thread_id: &str) -> Option<&ThreadWorkspaceRelation> {
        self.relations.current(&ThreadWorkspaceRelationKey::origin(
            self.thread_key(thread_id),
        ))
    }
}

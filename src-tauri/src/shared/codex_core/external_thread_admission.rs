use crate::shared::global_sources_core::desktop_projection::{
    WorkspaceAssignment, WorkspaceAssignmentState,
};
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum KnowledgeState {
    Unknown,
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum WriterOccupancy {
    Unknown,
    Unoccupied,
    Occupied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum Surface {
    Monitor,
    Cli,
    Desktop,
    Ide,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SurfaceProjectionEvidence {
    pub surface: Surface,
    pub project_assigned: Option<bool>,
    pub sidebar_visible: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalThreadAdmissionEvidence {
    pub thread_key: CodexThreadKey,
    pub title: Option<String>,
    pub exact_read_exists: Option<bool>,
    pub tombstoned: bool,
    pub workspace_assignment: WorkspaceAssignment,
    pub surface_projection: Option<SurfaceProjectionEvidence>,
    pub writer_occupancy: Option<WriterOccupancy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExternalThreadAdmissionState {
    pub thread_key: CodexThreadKey,
    pub title: Option<String>,
    pub exists: KnowledgeState,
    pub resumable: KnowledgeState,
    pub writer_occupancy: WriterOccupancy,
    pub workspace_assignment: WorkspaceAssignment,
    pub surface_projections: BTreeMap<Surface, SurfaceProjectionEvidence>,
    pub project_assigned: KnowledgeState,
    pub sidebar_visible: KnowledgeState,
    tombstoned: bool,
}

#[derive(Default)]
pub(crate) struct ExternalThreadAdmissionRegistry {
    records: HashMap<CodexThreadKey, ExternalThreadAdmissionState>,
}

impl ExternalThreadAdmissionRegistry {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, key: &CodexThreadKey) -> Option<&ExternalThreadAdmissionState> {
        self.records.get(key)
    }

    pub(crate) fn observe(
        &mut self,
        evidence: ExternalThreadAdmissionEvidence,
    ) -> &ExternalThreadAdmissionState {
        let key = evidence.thread_key.clone();
        let state =
            self.records
                .entry(key.clone())
                .or_insert_with(|| ExternalThreadAdmissionState {
                    thread_key: key,
                    title: None,
                    exists: KnowledgeState::Unknown,
                    resumable: KnowledgeState::Unknown,
                    writer_occupancy: WriterOccupancy::Unknown,
                    workspace_assignment: unassigned_workspace(),
                    surface_projections: BTreeMap::new(),
                    project_assigned: KnowledgeState::Unknown,
                    sidebar_visible: KnowledgeState::Unknown,
                    tombstoned: false,
                });

        if evidence.tombstoned {
            state.tombstoned = true;
        }
        if let Some(title) = evidence.title.filter(|title| !title.trim().is_empty()) {
            state.title = Some(title);
        }
        if evidence.workspace_assignment.state != WorkspaceAssignmentState::Unassigned
            || state.workspace_assignment.state == WorkspaceAssignmentState::Unassigned
        {
            state.workspace_assignment = evidence.workspace_assignment;
        }
        if let Some(projection) = evidence.surface_projection {
            state
                .surface_projections
                .insert(projection.surface, projection);
        }
        if let Some(writer_occupancy) = evidence.writer_occupancy {
            state.writer_occupancy = writer_occupancy;
        }

        if state.tombstoned {
            state.exists = KnowledgeState::No;
            state.resumable = KnowledgeState::No;
        } else if let Some(exists) = evidence.exact_read_exists {
            state.exists = knowledge(exists);
            state.resumable = knowledge(exists);
        }

        state.project_assigned = projection_knowledge(
            state
                .surface_projections
                .values()
                .filter_map(|projection| projection.project_assigned),
        );
        state.sidebar_visible = projection_knowledge(
            state
                .surface_projections
                .values()
                .filter_map(|projection| projection.sidebar_visible),
        );

        state
    }
}

fn knowledge(value: bool) -> KnowledgeState {
    if value {
        KnowledgeState::Yes
    } else {
        KnowledgeState::No
    }
}

fn projection_knowledge(values: impl Iterator<Item = bool>) -> KnowledgeState {
    let mut observed = None;
    for value in values {
        if observed.is_some_and(|candidate| candidate != value) {
            return KnowledgeState::Unknown;
        }
        observed = Some(value);
    }
    observed.map_or(KnowledgeState::Unknown, knowledge)
}

fn unassigned_workspace() -> WorkspaceAssignment {
    WorkspaceAssignment {
        state: WorkspaceAssignmentState::Unassigned,
        workspace_id: None,
        provenance: "no-workspace-evidence".to_string(),
        matched_path: None,
        candidate_workspace_ids: Vec::new(),
    }
}

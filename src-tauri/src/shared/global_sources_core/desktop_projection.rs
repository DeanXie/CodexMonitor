use super::source_envelope::{EvidenceConfidence, SourceKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ProducerSurface {
    #[serde(rename = "MONITOR")]
    Monitor,
    #[serde(rename = "DESKTOP")]
    Desktop,
    #[serde(rename = "CLI")]
    Cli,
    #[serde(rename = "IDE")]
    Ide,
    #[serde(rename = "AMBIGUOUS")]
    Ambiguous,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProducerSurfaceClassification {
    pub surface: ProducerSurface,
    pub confidence: EvidenceConfidence,
    pub evidence: Vec<String>,
    pub provenance: Vec<String>,
}

impl Default for ProducerSurfaceClassification {
    fn default() -> Self {
        Self {
            surface: ProducerSurface::Unknown,
            confidence: EvidenceConfidence::Unknown,
            evidence: vec!["no producer evidence".to_string()],
            provenance: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProducerSurfaceInput<'a> {
    pub source_kind: SourceKind,
    pub rollout_source: Option<&'a str>,
    pub originator: Option<&'a str>,
    pub desktop_catalog_membership: bool,
    pub confirmed_parent_surface: Option<ProducerSurface>,
}

impl<'a> ProducerSurfaceInput<'a> {
    pub(crate) fn monitor_live() -> Self {
        Self {
            source_kind: SourceKind::MonitorAppServer,
            rollout_source: None,
            originator: None,
            desktop_catalog_membership: false,
            confirmed_parent_surface: None,
        }
    }

    pub(crate) fn rollout(
        rollout_source: Option<&'a str>,
        originator: Option<&'a str>,
        desktop_catalog_membership: bool,
        confirmed_parent_surface: Option<ProducerSurface>,
    ) -> Self {
        Self {
            source_kind: SourceKind::CodexCliRollout,
            rollout_source,
            originator,
            desktop_catalog_membership,
            confirmed_parent_surface,
        }
    }
}

pub(crate) fn classify_producer_surface(
    input: &ProducerSurfaceInput<'_>,
) -> ProducerSurfaceClassification {
    let mut evidence = Vec::new();
    let mut provenance = vec![format!("transport:{:?}", input.source_kind)];
    if let Some(originator) = input.originator {
        evidence.push(format!("weak originator={originator}"));
        provenance.push("rollout.session_meta.originator".to_string());
    }
    if input.source_kind == SourceKind::MonitorAppServer {
        evidence.push("fresh Monitor app-server LIVE lane".to_string());
        provenance.push("app-server-live".to_string());
        return classification(
            ProducerSurface::Monitor,
            EvidenceConfidence::Confirmed,
            evidence,
            provenance,
        );
    }

    let source = input.rollout_source.map(|value| value.to_ascii_lowercase());
    if let Some(source) = source.as_deref() {
        evidence.push(format!("rollout source={source}"));
        provenance.push("rollout.file-owner.session_meta.source".to_string());
    }
    if input.desktop_catalog_membership {
        evidence.push("exact fullThreadId Desktop catalog membership".to_string());
        provenance.push("desktop.local_thread_catalog".to_string());
    }

    if let Some(parent_surface) = input.confirmed_parent_surface {
        let explicit_surface = match source.as_deref() {
            Some("cli" | "exec") => Some(ProducerSurface::Cli),
            Some("vscode") if input.desktop_catalog_membership => Some(ProducerSurface::Desktop),
            Some("ide") => Some(ProducerSurface::Ide),
            _ => None,
        };
        provenance.push("rollout.session_meta.source.subagent.thread_spawn".to_string());
        evidence.push("confirmed parent edge derives producer".to_string());
        if explicit_surface.is_some_and(|surface| surface != parent_surface) {
            return classification(
                ProducerSurface::Ambiguous,
                EvidenceConfidence::Inferred,
                with(
                    evidence,
                    "child source conflicts with confirmed parent surface",
                ),
                provenance,
            );
        }
        if explicit_surface.is_none() || explicit_surface == Some(parent_surface) {
            return classification(
                parent_surface,
                EvidenceConfidence::Inferred,
                evidence,
                provenance,
            );
        }
    }

    match source.as_deref() {
        Some("cli" | "exec") if input.desktop_catalog_membership => classification(
            ProducerSurface::Ambiguous,
            EvidenceConfidence::Inferred,
            with(
                evidence,
                "CLI-like rollout conflicts with Desktop membership",
            ),
            provenance,
        ),
        Some("cli" | "exec") => classification(
            ProducerSurface::Cli,
            EvidenceConfidence::Confirmed,
            evidence,
            provenance,
        ),
        Some("vscode") if input.desktop_catalog_membership => classification(
            ProducerSurface::Desktop,
            EvidenceConfidence::Confirmed,
            evidence,
            provenance,
        ),
        Some("vscode") => classification(
            ProducerSurface::Ambiguous,
            EvidenceConfidence::Inferred,
            with(evidence, "vscode source lacks Desktop corroboration"),
            provenance,
        ),
        Some("ide") if input.desktop_catalog_membership => classification(
            ProducerSurface::Ambiguous,
            EvidenceConfidence::Inferred,
            with(evidence, "IDE source conflicts with Desktop membership"),
            provenance,
        ),
        Some("ide") => classification(
            ProducerSurface::Ide,
            EvidenceConfidence::Confirmed,
            evidence,
            provenance,
        ),
        Some(_) if input.desktop_catalog_membership => classification(
            ProducerSurface::Ambiguous,
            EvidenceConfidence::Inferred,
            with(
                evidence,
                "unrecognized rollout source conflicts with Desktop membership",
            ),
            provenance,
        ),
        Some(_) => classification(
            ProducerSurface::Unknown,
            EvidenceConfidence::Unknown,
            evidence,
            provenance,
        ),
        None if input.desktop_catalog_membership => classification(
            ProducerSurface::Ambiguous,
            EvidenceConfidence::Unknown,
            with(
                evidence,
                "Desktop membership alone is not producer authority",
            ),
            provenance,
        ),
        None => ProducerSurfaceClassification::default(),
    }
}

fn classification(
    surface: ProducerSurface,
    confidence: EvidenceConfidence,
    evidence: Vec<String>,
    provenance: Vec<String>,
) -> ProducerSurfaceClassification {
    ProducerSurfaceClassification {
        surface,
        confidence,
        evidence,
        provenance,
    }
}

fn with(mut evidence: Vec<String>, value: &str) -> Vec<String> {
    evidence.push(value.to_string());
    evidence
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthorityPresence {
    Present,
    Absent,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThreadReadStatus {
    Exists,
    NotFound,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopProjectionEvidence {
    pub exact_catalog_match: bool,
    pub monitor_tombstone: bool,
    pub confirmed_rollout_identity: bool,
    pub authoritative_persisted_thread: AuthorityPresence,
    pub thread_read: ThreadReadStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DesktopProjectionState {
    #[serde(rename = "CANONICAL_SUPPLEMENT")]
    CanonicalSupplement,
    #[serde(rename = "DESKTOP_STALE_ORPHAN")]
    DesktopStaleOrphan,
    #[serde(rename = "AMBIGUOUS")]
    Ambiguous,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProjectionAssessment {
    pub state: DesktopProjectionState,
    pub canonical_ingest_allowed: bool,
    pub evidence: Vec<String>,
}

pub(crate) fn assess_desktop_projection(
    evidence: &DesktopProjectionEvidence,
) -> DesktopProjectionAssessment {
    if !evidence.exact_catalog_match {
        return assessment(
            DesktopProjectionState::Ambiguous,
            false,
            "no exact fullThreadId Desktop projection match",
        );
    }
    if evidence.monitor_tombstone {
        return assessment(
            DesktopProjectionState::DesktopStaleOrphan,
            false,
            "Monitor deletion tombstone overrides every lower authority",
        );
    }
    if evidence.confirmed_rollout_identity {
        return assessment(
            DesktopProjectionState::CanonicalSupplement,
            true,
            "confirmed rollout identity authorizes supplemental Desktop metadata",
        );
    }
    match (
        evidence.authoritative_persisted_thread,
        evidence.thread_read,
    ) {
        (AuthorityPresence::Absent, ThreadReadStatus::NotFound) => assessment(
            DesktopProjectionState::DesktopStaleOrphan,
            false,
            "rollout absent, persisted Thread absent, and thread/read not-found",
        ),
        (AuthorityPresence::Present, ThreadReadStatus::Exists) => assessment(
            DesktopProjectionState::Ambiguous,
            false,
            "authoritative Thread exists but no confirmed rollout identity was observed",
        ),
        _ => assessment(
            DesktopProjectionState::Ambiguous,
            false,
            "Desktop projection evidence is incomplete or conflicting",
        ),
    }
}

fn assessment(
    state: DesktopProjectionState,
    canonical_ingest_allowed: bool,
    evidence: &str,
) -> DesktopProjectionAssessment {
    DesktopProjectionAssessment {
        state,
        canonical_ingest_allowed,
        evidence: vec![evidence.to_string()],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceRoot {
    pub workspace_id: String,
    pub root_path: String,
}

impl WorkspaceRoot {
    pub(crate) fn new(workspace_id: impl Into<String>, root_path: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            root_path: root_path.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceMappingInput<'a> {
    pub rollout_cwd: Option<&'a str>,
    pub configured_roots: &'a [WorkspaceRoot],
    pub desktop_project_roots: &'a [String],
    pub confirmed_parent_workspace_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum WorkspaceAssignmentState {
    #[serde(rename = "ASSIGNED")]
    Assigned,
    #[serde(rename = "AMBIGUOUS")]
    Ambiguous,
    #[serde(rename = "UNASSIGNED")]
    Unassigned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceAssignment {
    pub state: WorkspaceAssignmentState,
    pub workspace_id: Option<String>,
    pub provenance: String,
    pub matched_path: Option<String>,
    pub candidate_workspace_ids: Vec<String>,
}

pub(crate) fn resolve_workspace_assignment(
    input: &WorkspaceMappingInput<'_>,
) -> WorkspaceAssignment {
    if let Some(cwd) = input.rollout_cwd {
        let assignment =
            match_workspace_path(cwd, input.configured_roots, "rollout-cwd-longest-root");
        if assignment.state != WorkspaceAssignmentState::Unassigned {
            return assignment;
        }
    }
    if let Some(parent) = input.confirmed_parent_workspace_id {
        return WorkspaceAssignment {
            state: WorkspaceAssignmentState::Assigned,
            workspace_id: Some(parent.to_string()),
            provenance: "confirmed-parent-edge".to_string(),
            matched_path: None,
            candidate_workspace_ids: vec![parent.to_string()],
        };
    }
    let mut desktop_matches = input
        .desktop_project_roots
        .iter()
        .map(|path| {
            match_workspace_path(path, input.configured_roots, "desktop-project-assignment")
        })
        .filter(|assignment| assignment.state != WorkspaceAssignmentState::Unassigned)
        .collect::<Vec<_>>();
    if desktop_matches.is_empty() {
        return WorkspaceAssignment {
            state: WorkspaceAssignmentState::Unassigned,
            workspace_id: None,
            provenance: "no-workspace-evidence".to_string(),
            matched_path: None,
            candidate_workspace_ids: Vec::new(),
        };
    }
    desktop_matches.sort_by(|left, right| {
        right
            .matched_path
            .as_ref()
            .map(|path| path.len())
            .cmp(&left.matched_path.as_ref().map(|path| path.len()))
            .then_with(|| left.workspace_id.cmp(&right.workspace_id))
    });
    desktop_matches.remove(0)
}

fn match_workspace_path(
    path: &str,
    roots: &[WorkspaceRoot],
    provenance: &str,
) -> WorkspaceAssignment {
    let normalized = normalize_path(path);
    let mut matches = roots
        .iter()
        .filter_map(|root| {
            let candidate = normalize_path(&root.root_path);
            path_contains(&normalized, &candidate).then_some((
                candidate.len(),
                root.workspace_id.clone(),
                candidate,
            ))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let candidate_workspace_ids = matches
        .iter()
        .map(|(_, id, _)| id.clone())
        .collect::<Vec<_>>();
    let Some((best_length, best_id, best_path)) = matches.first().cloned() else {
        return WorkspaceAssignment {
            state: WorkspaceAssignmentState::Unassigned,
            workspace_id: None,
            provenance: provenance.to_string(),
            matched_path: None,
            candidate_workspace_ids,
        };
    };
    let tied = matches
        .iter()
        .filter(|(length, _, _)| *length == best_length)
        .map(|(_, id, _)| id)
        .collect::<BTreeSet<_>>();
    if tied.len() > 1 {
        return WorkspaceAssignment {
            state: WorkspaceAssignmentState::Ambiguous,
            workspace_id: None,
            provenance: format!("{provenance}:equal-longest-root-conflict"),
            matched_path: Some(best_path),
            candidate_workspace_ids,
        };
    }
    WorkspaceAssignment {
        state: WorkspaceAssignmentState::Assigned,
        workspace_id: Some(best_id),
        provenance: provenance.to_string(),
        matched_path: Some(best_path),
        candidate_workspace_ids,
    }
}

fn normalize_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    if value.to_ascii_lowercase().starts_with("//?/unc/") {
        value = format!("//{}", &value[8..]);
    } else if value.to_ascii_lowercase().starts_with("//?/") {
        value = value[4..].to_string();
    }
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    if value.get(1..2) == Some(":") || value.starts_with("//") {
        value.make_ascii_lowercase();
    }
    value
}

fn path_contains(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

//! Process-local projection freshness contract.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;

use super::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
use super::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use super::codex_identity::CodexThreadKey;
use super::remote_host_identity::DaemonProcessGeneration;
use super::remote_request_provenance::RemoteTransportGeneration;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionFreshnessStatus {
    NotHydrated,
    Hydrating,
    Current,
    Stale,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionFreshnessCoverage {
    WorkspaceCatalog,
    ThreadCatalog,
    ThreadDetail,
    ObservationSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectionFreshnessSource {
    WorkspaceList,
    ThreadList,
    ThreadRead,
    ObservationQuery,
    Event,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectionFreshnessGenerationVector {
    pub daemon_process_generation: Option<DaemonProcessGeneration>,
    pub remote_transport_generation: Option<RemoteTransportGeneration>,
    pub workspace_session_generation: Option<WorkspaceSessionGeneration>,
    pub app_server_connection_generation: Option<AppServerConnectionGeneration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectionFreshnessKey {
    pub workspace_id: Option<String>,
    pub thread_key: Option<CodexThreadKey>,
    pub coverage: ProjectionFreshnessCoverage,
}

impl ProjectionFreshnessKey {
    pub(crate) fn workspace_catalog() -> Self {
        Self {
            workspace_id: None,
            thread_key: None,
            coverage: ProjectionFreshnessCoverage::WorkspaceCatalog,
        }
    }

    pub(crate) fn thread_catalog(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: Some(workspace_id.into()),
            thread_key: None,
            coverage: ProjectionFreshnessCoverage::ThreadCatalog,
        }
    }

    pub(crate) fn thread_detail(
        workspace_id: impl Into<String>,
        thread_key: CodexThreadKey,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id.into()),
            thread_key: Some(thread_key),
            coverage: ProjectionFreshnessCoverage::ThreadDetail,
        }
    }

    pub(crate) fn observation_snapshot(
        workspace_id: impl Into<String>,
        thread_key: CodexThreadKey,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id.into()),
            thread_key: Some(thread_key),
            coverage: ProjectionFreshnessCoverage::ObservationSnapshot,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectionFreshnessSnapshot {
    pub coverage: ProjectionFreshnessCoverage,
    pub status: ProjectionFreshnessStatus,
    pub generations: ProjectionFreshnessGenerationVector,
    pub source: Option<ProjectionFreshnessSource>,
    pub observed_at: Option<i64>,
    pub hydrated_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectionFreshnessQuerySnapshot {
    pub workspace_id: String,
    pub thread_key: Option<CodexThreadKey>,
    pub coverages: Vec<ProjectionFreshnessSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectionFreshnessQueryError {
    WorkspaceIdRequired,
    WorkspaceNotFound,
}

impl fmt::Display for ProjectionFreshnessQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WorkspaceIdRequired => "workspaceId is required",
            Self::WorkspaceNotFound => "workspace not found",
        })
    }
}

impl ProjectionFreshnessQuerySnapshot {
    pub(crate) fn status(
        &self,
        coverage: ProjectionFreshnessCoverage,
    ) -> Option<ProjectionFreshnessStatus> {
        self.coverages
            .iter()
            .find(|item| item.coverage == coverage)
            .map(|item| item.status)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectionFreshnessEvidence {
    status: ProjectionFreshnessStatus,
    generations: ProjectionFreshnessGenerationVector,
    source: ProjectionFreshnessSource,
    observed_at: i64,
    hydrated_at: Option<i64>,
}

pub(crate) struct ProjectionFreshnessRuntime {
    base_generations: ProjectionFreshnessGenerationVector,
    history: Mutex<HashMap<ProjectionFreshnessKey, Vec<ProjectionFreshnessEvidence>>>,
}

impl Default for ProjectionFreshnessRuntime {
    fn default() -> Self {
        Self::new(ProjectionFreshnessGenerationVector::default())
    }
}

impl ProjectionFreshnessRuntime {
    pub(crate) fn new(base_generations: ProjectionFreshnessGenerationVector) -> Self {
        Self {
            base_generations,
            history: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn base_generations(&self) -> ProjectionFreshnessGenerationVector {
        self.base_generations.clone()
    }

    pub(crate) fn generations_for_session(
        &self,
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> ProjectionFreshnessGenerationVector {
        ProjectionFreshnessGenerationVector {
            workspace_session_generation: Some(workspace_session_generation),
            app_server_connection_generation: Some(app_server_connection_generation),
            ..self.base_generations()
        }
    }

    pub(crate) fn record_hydrating(
        &self,
        key: ProjectionFreshnessKey,
        generations: ProjectionFreshnessGenerationVector,
        source: ProjectionFreshnessSource,
        observed_at: i64,
    ) -> Result<(), String> {
        self.record(
            key,
            ProjectionFreshnessEvidence {
                status: ProjectionFreshnessStatus::Hydrating,
                generations,
                source,
                observed_at,
                hydrated_at: None,
            },
        )
    }

    pub(crate) fn record_current(
        &self,
        key: ProjectionFreshnessKey,
        generations: ProjectionFreshnessGenerationVector,
        source: ProjectionFreshnessSource,
        observed_at: i64,
        hydrated_at: i64,
    ) -> Result<(), String> {
        self.record(
            key,
            ProjectionFreshnessEvidence {
                status: ProjectionFreshnessStatus::Current,
                generations,
                source,
                observed_at,
                hydrated_at: Some(hydrated_at),
            },
        )
    }

    pub(crate) fn record_unavailable(
        &self,
        key: ProjectionFreshnessKey,
        generations: ProjectionFreshnessGenerationVector,
        source: ProjectionFreshnessSource,
        observed_at: i64,
    ) -> Result<(), String> {
        self.record(
            key,
            ProjectionFreshnessEvidence {
                status: ProjectionFreshnessStatus::Unavailable,
                generations,
                source,
                observed_at,
                hydrated_at: None,
            },
        )
    }

    pub(crate) fn record_unknown(
        &self,
        key: ProjectionFreshnessKey,
        generations: ProjectionFreshnessGenerationVector,
        source: ProjectionFreshnessSource,
        observed_at: i64,
    ) -> Result<(), String> {
        self.record(
            key,
            ProjectionFreshnessEvidence {
                status: ProjectionFreshnessStatus::Unknown,
                generations,
                source,
                observed_at,
                hydrated_at: None,
            },
        )
    }

    pub(crate) fn record_event_if_current(
        &self,
        key: ProjectionFreshnessKey,
        generations: ProjectionFreshnessGenerationVector,
        observed_at: i64,
    ) -> Result<bool, String> {
        let snapshot = self.snapshot(&key, &generations);
        if snapshot.status != ProjectionFreshnessStatus::Current {
            return Ok(false);
        }
        self.record(
            key,
            ProjectionFreshnessEvidence {
                status: ProjectionFreshnessStatus::Current,
                generations,
                source: ProjectionFreshnessSource::Event,
                observed_at,
                hydrated_at: snapshot.hydrated_at,
            },
        )?;
        Ok(true)
    }

    pub(crate) fn snapshot(
        &self,
        key: &ProjectionFreshnessKey,
        current: &ProjectionFreshnessGenerationVector,
    ) -> ProjectionFreshnessSnapshot {
        let history = self.history.lock().expect("projection freshness lock");
        let Some(evidence) = history.get(key) else {
            return not_hydrated_snapshot(key.coverage, current.clone());
        };

        if let Some(current_evidence) = evidence
            .iter()
            .filter(|candidate| evidence_matches_current(key.coverage, candidate, current))
            .max_by(|left, right| compare_evidence(left, right))
        {
            return snapshot_from_evidence(key.coverage, current_evidence, false);
        }

        evidence
            .iter()
            .max_by(|left, right| compare_evidence(left, right))
            .map(|historical| snapshot_from_evidence(key.coverage, historical, true))
            .unwrap_or_else(|| not_hydrated_snapshot(key.coverage, current.clone()))
    }

    pub(crate) fn query(
        &self,
        workspace_id: &str,
        thread_key: Option<CodexThreadKey>,
        current: &ProjectionFreshnessGenerationVector,
    ) -> ProjectionFreshnessQuerySnapshot {
        let mut keys = vec![
            Self::workspace_key(),
            ProjectionFreshnessKey::thread_catalog(workspace_id),
        ];
        if let Some(key) = thread_key.clone() {
            keys.push(ProjectionFreshnessKey::thread_detail(
                workspace_id,
                key.clone(),
            ));
            keys.push(ProjectionFreshnessKey::observation_snapshot(
                workspace_id,
                key,
            ));
        }
        ProjectionFreshnessQuerySnapshot {
            workspace_id: workspace_id.to_string(),
            thread_key,
            coverages: keys.iter().map(|key| self.snapshot(key, current)).collect(),
        }
    }

    pub(crate) fn unavailable_query(
        &self,
        workspace_id: &str,
        include_thread_scopes: bool,
    ) -> ProjectionFreshnessQuerySnapshot {
        let base = self.base_generations();
        let mut coverages = vec![
            self.snapshot(&ProjectionFreshnessKey::workspace_catalog(), &base),
            unavailable_snapshot(ProjectionFreshnessCoverage::ThreadCatalog, base.clone()),
        ];
        if include_thread_scopes {
            coverages.push(unavailable_snapshot(
                ProjectionFreshnessCoverage::ThreadDetail,
                base.clone(),
            ));
            coverages.push(unavailable_snapshot(
                ProjectionFreshnessCoverage::ObservationSnapshot,
                base,
            ));
        }
        ProjectionFreshnessQuerySnapshot {
            workspace_id: workspace_id.to_string(),
            thread_key: None,
            coverages,
        }
    }

    fn workspace_key() -> ProjectionFreshnessKey {
        ProjectionFreshnessKey::workspace_catalog()
    }

    pub(crate) fn evidence_count(&self) -> usize {
        self.history
            .lock()
            .expect("projection freshness lock")
            .values()
            .map(Vec::len)
            .sum()
    }

    fn record(
        &self,
        key: ProjectionFreshnessKey,
        evidence: ProjectionFreshnessEvidence,
    ) -> Result<(), String> {
        validate_source(key.coverage, evidence.source)?;
        self.history
            .lock()
            .map_err(|_| "projection freshness lock poisoned".to_string())?
            .entry(key)
            .or_default()
            .push(evidence);
        Ok(())
    }
}

fn not_hydrated_snapshot(
    coverage: ProjectionFreshnessCoverage,
    generations: ProjectionFreshnessGenerationVector,
) -> ProjectionFreshnessSnapshot {
    ProjectionFreshnessSnapshot {
        coverage,
        status: ProjectionFreshnessStatus::NotHydrated,
        generations,
        source: None,
        observed_at: None,
        hydrated_at: None,
    }
}

fn unavailable_snapshot(
    coverage: ProjectionFreshnessCoverage,
    generations: ProjectionFreshnessGenerationVector,
) -> ProjectionFreshnessSnapshot {
    ProjectionFreshnessSnapshot {
        coverage,
        status: ProjectionFreshnessStatus::Unavailable,
        generations,
        source: None,
        observed_at: None,
        hydrated_at: None,
    }
}

fn snapshot_from_evidence(
    coverage: ProjectionFreshnessCoverage,
    evidence: &ProjectionFreshnessEvidence,
    historical: bool,
) -> ProjectionFreshnessSnapshot {
    ProjectionFreshnessSnapshot {
        coverage,
        status: if historical {
            ProjectionFreshnessStatus::Stale
        } else {
            evidence.status
        },
        generations: evidence.generations.clone(),
        source: Some(evidence.source),
        observed_at: Some(evidence.observed_at),
        hydrated_at: evidence.hydrated_at,
    }
}

fn evidence_matches_current(
    coverage: ProjectionFreshnessCoverage,
    evidence: &ProjectionFreshnessEvidence,
    current: &ProjectionFreshnessGenerationVector,
) -> bool {
    optional_generation_matches(
        evidence.generations.daemon_process_generation.as_ref(),
        current.daemon_process_generation.as_ref(),
    ) && optional_generation_matches(
        evidence.generations.remote_transport_generation.as_ref(),
        current.remote_transport_generation.as_ref(),
    ) && (!matches!(
        coverage,
        ProjectionFreshnessCoverage::ThreadCatalog
            | ProjectionFreshnessCoverage::ThreadDetail
            | ProjectionFreshnessCoverage::ObservationSnapshot
    ) || optional_generation_matches(
        evidence.generations.workspace_session_generation.as_ref(),
        current.workspace_session_generation.as_ref(),
    )) && (!matches!(
        coverage,
        ProjectionFreshnessCoverage::ThreadDetail
            | ProjectionFreshnessCoverage::ObservationSnapshot
    ) || optional_generation_matches(
        evidence
            .generations
            .app_server_connection_generation
            .as_ref(),
        current.app_server_connection_generation.as_ref(),
    ))
}

fn optional_generation_matches<T: PartialEq>(evidence: Option<&T>, current: Option<&T>) -> bool {
    match evidence {
        None => true,
        Some(evidence) => current.is_some_and(|current| current == evidence),
    }
}

fn compare_evidence(
    left: &ProjectionFreshnessEvidence,
    right: &ProjectionFreshnessEvidence,
) -> Ordering {
    source_authority(left.source)
        .cmp(&source_authority(right.source))
        .then_with(|| left.observed_at.cmp(&right.observed_at))
}

fn source_authority(source: ProjectionFreshnessSource) -> u8 {
    match source {
        ProjectionFreshnessSource::ThreadList
        | ProjectionFreshnessSource::ThreadRead
        | ProjectionFreshnessSource::ObservationQuery
        | ProjectionFreshnessSource::WorkspaceList => 2,
        ProjectionFreshnessSource::Event => 1,
    }
}

fn validate_source(
    coverage: ProjectionFreshnessCoverage,
    source: ProjectionFreshnessSource,
) -> Result<(), String> {
    let valid = matches!(source, ProjectionFreshnessSource::Event)
        || matches!(
            (coverage, source),
            (
                ProjectionFreshnessCoverage::WorkspaceCatalog,
                ProjectionFreshnessSource::WorkspaceList
            ) | (
                ProjectionFreshnessCoverage::ThreadCatalog,
                ProjectionFreshnessSource::ThreadList
            ) | (
                ProjectionFreshnessCoverage::ThreadDetail,
                ProjectionFreshnessSource::ThreadRead
            ) | (
                ProjectionFreshnessCoverage::ObservationSnapshot,
                ProjectionFreshnessSource::ObservationQuery
            )
        );
    if valid {
        Ok(())
    } else {
        Err("freshness source does not match coverage".to_string())
    }
}

pub(crate) fn record_authoritative_read_outcome(
    runtime: &ProjectionFreshnessRuntime,
    key: ProjectionFreshnessKey,
    generations: ProjectionFreshnessGenerationVector,
    source: ProjectionFreshnessSource,
    result: &Result<(), String>,
    observed_at: i64,
) {
    match result {
        Ok(_) => {
            let _ = runtime.record_current(key, generations, source, observed_at, observed_at);
        }
        Err(_) => {
            let _ = runtime.record_unavailable(key, generations, source, observed_at);
        }
    }
}

pub(crate) fn record_workspace_catalog_current(
    runtime: &ProjectionFreshnessRuntime,
    observed_at: i64,
) {
    let _ = runtime.record_current(
        ProjectionFreshnessKey::workspace_catalog(),
        runtime.base_generations(),
        ProjectionFreshnessSource::WorkspaceList,
        observed_at,
        observed_at,
    );
}

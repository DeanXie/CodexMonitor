//! Pure Phase 3.3 execution-settings evidence contract.
//!
//! This module deliberately has no app-server, rollout, UI, creation, or
//! workspace ingestion. Callers provide already-classified evidence.
//!
//! `comparison_id` is evidence-correlation identity, not Thread or Turn
//! identity. Ingestion must derive it from an authoritative request/Turn/
//! settings correlation. Time proximity, equal values, cwd, prompt content,
//! or the latest settings event are not correlation evidence. A Turn-scoped
//! group should use its real full Turn ID. A settings update with only a
//! Thread ID remains a Thread-default snapshot and must not be assigned to a
//! Turn.

#![allow(dead_code)] // Phase 3.3.3a defines the model before 3.3.3b ingestion.

use std::collections::{BTreeMap, HashMap};

use super::global_sources_core::rollout_identity::CodexThreadKey;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingsScope {
    ThreadDefault,
    TurnExecution { turn_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExecutionSettingsObservationKey {
    pub thread_key: CodexThreadKey,
    pub scope: ExecutionSettingsScope,
}

impl ExecutionSettingsObservationKey {
    pub(crate) fn thread_default(thread_key: CodexThreadKey) -> Self {
        Self {
            thread_key,
            scope: ExecutionSettingsScope::ThreadDefault,
        }
    }

    pub(crate) fn turn(thread_key: CodexThreadKey, turn_id: impl Into<String>) -> Self {
        Self {
            thread_key,
            scope: ExecutionSettingsScope::TurnExecution {
                turn_id: turn_id.into(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingField {
    Model,
    Effort,
    ApprovalPolicy,
    SandboxPolicy,
    NetworkAccess,
    WritableRoots,
    Cwd,
    CollaborationMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingValue {
    Text(String),
    Bool(bool),
    StringList(Vec<String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingsEvidenceLayer {
    Requested,
    ServerEffective,
    PersistedObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingsAssessment {
    Unknown,
    RequestedOnly,
    EffectiveConfirmed,
    ObservedConfirmed,
    Match,
    Mismatch,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingsEvidenceReason {
    Overridden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExecutionSettingsEvidenceConfidence {
    Unknown,
    Inferred,
    Confirmed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExecutionSettingsProvenance {
    pub source: String,
    /// Evidence from different comparison IDs is never compared directly.
    pub comparison_id: String,
    pub observed_at: u64,
    pub confidence: ExecutionSettingsEvidenceConfidence,
    pub reason: Option<ExecutionSettingsEvidenceReason>,
}

impl ExecutionSettingsProvenance {
    pub(crate) fn confirmed(
        source: impl Into<String>,
        comparison_id: impl Into<String>,
        observed_at: u64,
    ) -> Self {
        Self {
            source: source.into(),
            comparison_id: comparison_id.into(),
            observed_at,
            confidence: ExecutionSettingsEvidenceConfidence::Confirmed,
            reason: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExecutionSettingsEvidenceRecord<T> {
    pub layer: ExecutionSettingsEvidenceLayer,
    pub value: T,
    pub provenance: ExecutionSettingsProvenance,
}

impl<T> ExecutionSettingsEvidenceRecord<T> {
    pub(crate) fn new(
        layer: ExecutionSettingsEvidenceLayer,
        value: T,
        provenance: ExecutionSettingsProvenance,
    ) -> Self {
        Self {
            layer,
            value,
            provenance,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingEvidence<T> {
    pub requested: Vec<ExecutionSettingsEvidenceRecord<T>>,
    pub server_effective: Vec<ExecutionSettingsEvidenceRecord<T>>,
    pub persisted_observed: Vec<ExecutionSettingsEvidenceRecord<T>>,
    pub assessment: ExecutionSettingsAssessment,
    pub provenance: Vec<ExecutionSettingsProvenance>,
    pub observed_at: Option<u64>,
    pub confidence: ExecutionSettingsEvidenceConfidence,
}

impl<T> Default for SettingEvidence<T> {
    fn default() -> Self {
        Self {
            requested: Vec::new(),
            server_effective: Vec::new(),
            persisted_observed: Vec::new(),
            assessment: ExecutionSettingsAssessment::Unknown,
            provenance: Vec::new(),
            observed_at: None,
            confidence: ExecutionSettingsEvidenceConfidence::Unknown,
        }
    }
}

impl<T: Eq> SettingEvidence<T> {
    pub(crate) fn canonical_observed_value(&self) -> Option<&T> {
        let first = &self.persisted_observed.first()?.value;
        self.persisted_observed
            .iter()
            .all(|record| &record.value == first)
            .then_some(first)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionSettingsObservation {
    pub key: ExecutionSettingsObservationKey,
    pub fields: BTreeMap<ExecutionSettingField, SettingEvidence<ExecutionSettingValue>>,
}

#[derive(Default)]
pub(crate) struct ExecutionSettingsEvidenceStore {
    history_by_field: HashMap<
        (ExecutionSettingsObservationKey, ExecutionSettingField),
        Vec<ExecutionSettingsEvidenceRecord<ExecutionSettingValue>>,
    >,
}

impl ExecutionSettingsEvidenceStore {
    pub(crate) fn observe(
        &mut self,
        key: ExecutionSettingsObservationKey,
        field: ExecutionSettingField,
        evidence: ExecutionSettingsEvidenceRecord<ExecutionSettingValue>,
    ) -> bool {
        let history = self.history_by_field.entry((key, field)).or_default();
        if history.contains(&evidence) {
            return false;
        }
        history.push(evidence);
        true
    }

    pub(crate) fn history(
        &self,
        key: &ExecutionSettingsObservationKey,
        field: ExecutionSettingField,
    ) -> &[ExecutionSettingsEvidenceRecord<ExecutionSettingValue>] {
        self.history_by_field
            .get(&(key.clone(), field))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn select(
        &self,
        key: &ExecutionSettingsObservationKey,
        field: ExecutionSettingField,
    ) -> SettingEvidence<ExecutionSettingValue> {
        select_setting_evidence(self.history(key, field))
    }

    pub(crate) fn observation(
        &self,
        key: &ExecutionSettingsObservationKey,
    ) -> ExecutionSettingsObservation {
        let mut fields = self
            .history_by_field
            .keys()
            .filter(|(candidate, _)| candidate == key)
            .map(|(_, field)| *field)
            .collect::<Vec<_>>();
        fields.sort();
        fields.dedup();
        ExecutionSettingsObservation {
            key: key.clone(),
            fields: fields
                .into_iter()
                .map(|field| (field, self.select(key, field)))
                .collect(),
        }
    }
}

fn select_setting_evidence<T>(records: &[ExecutionSettingsEvidenceRecord<T>]) -> SettingEvidence<T>
where
    T: Clone + Eq + Ord,
{
    let Some(comparison_id) = records
        .iter()
        .map(|record| {
            (
                record.provenance.observed_at,
                record.provenance.comparison_id.as_str(),
            )
        })
        .max()
        .map(|(_, comparison_id)| comparison_id)
    else {
        return SettingEvidence::default();
    };

    let mut comparable = records
        .iter()
        .filter(|record| record.provenance.comparison_id == comparison_id)
        .cloned()
        .collect::<Vec<_>>();
    comparable.sort();

    let requested = records_for_layer(&comparable, ExecutionSettingsEvidenceLayer::Requested);
    let server_effective =
        records_for_layer(&comparable, ExecutionSettingsEvidenceLayer::ServerEffective);
    let persisted_observed = records_for_layer(
        &comparable,
        ExecutionSettingsEvidenceLayer::PersistedObserved,
    );
    let assessment = assess(&requested, &server_effective, &persisted_observed);
    let provenance = comparable
        .iter()
        .map(|record| record.provenance.clone())
        .collect::<Vec<_>>();
    let observed_at = provenance.iter().map(|item| item.observed_at).max();
    let confidence = provenance
        .iter()
        .map(|item| item.confidence)
        .max()
        .unwrap_or(ExecutionSettingsEvidenceConfidence::Unknown);

    SettingEvidence {
        requested,
        server_effective,
        persisted_observed,
        assessment,
        provenance,
        observed_at,
        confidence,
    }
}

fn records_for_layer<T: Clone>(
    records: &[ExecutionSettingsEvidenceRecord<T>],
    layer: ExecutionSettingsEvidenceLayer,
) -> Vec<ExecutionSettingsEvidenceRecord<T>> {
    records
        .iter()
        .filter(|record| record.layer == layer)
        .cloned()
        .collect()
}

fn assess<T: Eq + Ord>(
    requested: &[ExecutionSettingsEvidenceRecord<T>],
    server_effective: &[ExecutionSettingsEvidenceRecord<T>],
    persisted_observed: &[ExecutionSettingsEvidenceRecord<T>],
) -> ExecutionSettingsAssessment {
    let requested_values = canonical_values(requested);
    let effective_values = canonical_values(server_effective);
    let observed_values = canonical_values(persisted_observed);

    if !effective_values.is_empty()
        && !observed_values.is_empty()
        && effective_values != observed_values
    {
        return ExecutionSettingsAssessment::Conflict;
    }

    let later_values = if !observed_values.is_empty() {
        &observed_values
    } else {
        &effective_values
    };
    if !requested_values.is_empty() && !later_values.is_empty() {
        return if requested_values == *later_values {
            ExecutionSettingsAssessment::Match
        } else {
            ExecutionSettingsAssessment::Mismatch
        };
    }

    if !observed_values.is_empty() {
        ExecutionSettingsAssessment::ObservedConfirmed
    } else if !effective_values.is_empty() {
        ExecutionSettingsAssessment::EffectiveConfirmed
    } else if !requested_values.is_empty() {
        ExecutionSettingsAssessment::RequestedOnly
    } else {
        ExecutionSettingsAssessment::Unknown
    }
}

fn canonical_values<'a, T: Eq + Ord>(
    records: &'a [ExecutionSettingsEvidenceRecord<T>],
) -> Vec<&'a T> {
    let mut values = records
        .iter()
        .map(|record| &record.value)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexHomeIdentity {
    pub normalized_path: String,
    pub identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceFileIdentity {
    pub normalized_path: String,
    pub filesystem_id: Option<String>,
    pub generation: String,
    pub session_meta_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceCursor {
    pub byte_start: u64,
    pub byte_end: u64,
    pub record_ordinal: u64,
    pub line_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum SourceKind {
    #[serde(rename = "monitor-app-server")]
    MonitorAppServer,
    #[serde(rename = "codex-cli-rollout")]
    CodexCliRollout,
    #[serde(rename = "historical-rollout-scan")]
    HistoricalRolloutScan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum SourceTemporalClass {
    #[serde(rename = "LIVE")]
    Live,
    #[serde(rename = "NEAR_LIVE")]
    NearLive,
    #[serde(rename = "HISTORICAL")]
    Historical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SourceTimestampKind {
    Record,
    Lifecycle,
    Filesystem,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceTimestamps {
    pub source_timestamp_ms: Option<i64>,
    pub source_timestamp_kind: SourceTimestampKind,
    pub observed_timestamp_ms: i64,
    pub lag_ms: Option<i64>,
}

impl SourceTimestamps {
    pub(crate) fn new(
        source_timestamp_ms: Option<i64>,
        source_timestamp_kind: SourceTimestampKind,
        observed_timestamp_ms: i64,
    ) -> Self {
        Self {
            source_timestamp_ms,
            source_timestamp_kind,
            observed_timestamp_ms,
            lag_ms: source_timestamp_ms.map(|source| observed_timestamp_ms - source),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FreshnessState {
    Fresh,
    Stale,
    Settled,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FreshnessEvidence {
    pub state: FreshnessState,
    pub last_complete_record_observed_at_ms: Option<i64>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaEvidence {
    pub producer: String,
    pub producer_version: Option<String>,
    pub record_schema: String,
    pub schema_version: Option<String>,
    pub schema_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EvidenceConfidence {
    Confirmed,
    Inferred,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfidenceEvidence {
    pub level: EvidenceConfidence,
    pub basis: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProvenanceEvidence {
    pub evidence_kind: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceEnvelope<T> {
    pub envelope_version: u32,
    pub observation_id: String,
    pub source_kind: SourceKind,
    pub temporal_class: SourceTemporalClass,
    pub source_instance_id: String,
    pub codex_home: Option<CodexHomeIdentity>,
    pub source_file: Option<SourceFileIdentity>,
    pub cursor: Option<SourceCursor>,
    pub timestamps: SourceTimestamps,
    pub freshness: FreshnessEvidence,
    pub schema: SchemaEvidence,
    pub confidence: ConfidenceEvidence,
    pub provenance: ProvenanceEvidence,
    pub record: T,
}

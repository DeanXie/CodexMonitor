use crate::backend::events::AppServerEvent;
use crate::shared::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::projection_freshness::{
    ProjectionFreshnessCoverage, ProjectionFreshnessGenerationVector, ProjectionFreshnessSnapshot,
    ProjectionFreshnessSource, ProjectionFreshnessStatus,
};
use crate::shared::remote_host_identity::{DaemonProcessGeneration, RemoteHostIdentity};
use crate::shared::remote_request_provenance::RemoteTransportGeneration;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const FIXTURE_DIRECTORY: &str = "phase-3-5-4-compatibility";

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
}

fn fixture_path(name: &str) -> PathBuf {
    fixture_root().join(FIXTURE_DIRECTORY).join(name)
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_fixture(name: &str) -> Value {
    let path = fixture_path(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read compatibility fixture {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse compatibility fixture {}: {error}", path.display()))
}

fn authority() -> Value {
    let fixture = load_fixture("authority-contract.json");
    assert_eq!(fixture["schemaVersion"], 1);
    fixture
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|entry| entry.as_str().expect("string entry").to_string())
        .collect()
}

fn object_keys(value: &Value, keys: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                keys.push(key.clone());
                object_keys(value, keys);
            }
        }
        Value::Array(values) => {
            for value in values {
                object_keys(value, keys);
            }
        }
        _ => {}
    }
}

fn session_generation(value: &str) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(value).expect("WorkspaceSession generation")
}

fn connection_generation(value: &str) -> AppServerConnectionGeneration {
    AppServerConnectionGeneration::new(value).expect("app-server connection generation")
}

fn local_event() -> AppServerEvent {
    AppServerEvent::for_session(
        "workspace-sanitized",
        json!({"method":"thread/updated"}),
        session_generation("workspace-generation-a"),
        connection_generation("connection-generation-a"),
    )
}

fn current_snapshot() -> ProjectionFreshnessSnapshot {
    ProjectionFreshnessSnapshot {
        coverage: ProjectionFreshnessCoverage::ThreadCatalog,
        status: ProjectionFreshnessStatus::Current,
        generations: ProjectionFreshnessGenerationVector {
            daemon_process_generation: Some(
                DaemonProcessGeneration::new("daemon-generation-a").unwrap(),
            ),
            remote_transport_generation: Some(
                RemoteTransportGeneration::new("transport-generation-a").unwrap(),
            ),
            workspace_session_generation: Some(session_generation("workspace-generation-a")),
            app_server_connection_generation: Some(connection_generation(
                "connection-generation-a",
            )),
        },
        source: Some(ProjectionFreshnessSource::ThreadList),
        observed_at: Some(10),
        hydrated_at: Some(10),
    }
}

#[test]
fn projection_freshness_schema_stable() {
    let actual = [
        ProjectionFreshnessStatus::NotHydrated,
        ProjectionFreshnessStatus::Hydrating,
        ProjectionFreshnessStatus::Current,
        ProjectionFreshnessStatus::Stale,
        ProjectionFreshnessStatus::Unavailable,
        ProjectionFreshnessStatus::Unknown,
    ]
    .into_iter()
    .map(|state| {
        serde_json::to_value(state)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    })
    .collect::<Vec<_>>();
    assert_eq!(
        actual,
        strings(&authority()["projectionFreshness"]["statuses"])
    );
}

#[test]
fn projection_coverage_schema_stable() {
    let coverages = [
        ProjectionFreshnessCoverage::WorkspaceCatalog,
        ProjectionFreshnessCoverage::ThreadCatalog,
        ProjectionFreshnessCoverage::ThreadDetail,
        ProjectionFreshnessCoverage::ObservationSnapshot,
    ]
    .into_iter()
    .map(|coverage| {
        serde_json::to_value(coverage)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    })
    .collect::<Vec<_>>();
    let sources = [
        ProjectionFreshnessSource::WorkspaceList,
        ProjectionFreshnessSource::ThreadList,
        ProjectionFreshnessSource::ThreadRead,
        ProjectionFreshnessSource::ObservationQuery,
        ProjectionFreshnessSource::Event,
    ]
    .into_iter()
    .map(|source| {
        serde_json::to_value(source)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    })
    .collect::<Vec<_>>();
    assert_eq!(
        coverages,
        strings(&authority()["projectionFreshness"]["coverages"])
    );
    assert_eq!(
        sources,
        strings(&authority()["projectionFreshness"]["sources"])
    );
}

#[test]
fn generation_event_schema_stable() {
    let serialized = serde_json::to_value(local_event()).unwrap();
    let actual = serialized
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let expected = strings(&authority()["generationEvent"]["fields"])
        .into_iter()
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn app_daemon_ts_event_parity() {
    let local = serde_json::to_value(local_event()).unwrap();
    assert!(local["daemonProcessGeneration"].is_null());
    assert!(local["remoteTransportGeneration"].is_null());
    let remote = serde_json::to_value(local_event().for_remote_delivery(
        DaemonProcessGeneration::new("daemon-generation-a").unwrap(),
        RemoteTransportGeneration::new("transport-generation-a").unwrap(),
    ))
    .unwrap();
    assert_eq!(remote["daemonProcessGeneration"], "daemon-generation-a");
    assert_eq!(
        remote["remoteTransportGeneration"],
        "transport-generation-a"
    );
    assert_eq!(
        authority()["generationEvent"]["localRemoteFields"],
        json!([null, null])
    );
}

#[test]
fn observation_snapshot_parity_stable() {
    assert_eq!(
        serde_json::to_value(ProjectionFreshnessCoverage::ObservationSnapshot).unwrap(),
        "observation_snapshot"
    );
    assert_eq!(
        serde_json::to_value(ProjectionFreshnessSource::ObservationQuery).unwrap(),
        "observation_query"
    );
    assert_eq!(authority()["observationSnapshot"]["readOnly"], true);
}

#[test]
fn event_gap_contract_stable() {
    assert_eq!(
        strings(&authority()["eventGap"]["fields"]),
        [
            "skipped",
            "observedAt",
            "daemonProcessGeneration",
            "remoteTransportGeneration",
            "affectedCoverages",
        ]
    );
    assert_eq!(authority()["eventGap"]["persistent"], false);
    assert_eq!(authority()["eventGap"]["mutatesSharedAuthority"], false);
}

#[test]
fn generation_hierarchy_stable() {
    assert_eq!(
        strings(&authority()["generationHierarchy"]),
        [
            "RemoteHostIdentity",
            "DaemonProcessGeneration",
            "RemoteTransportGeneration",
            "WorkspaceSessionGeneration",
            "AppServerConnectionGeneration",
        ]
    );
    assert_eq!(
        serde_json::to_value(
            RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap()
        )
        .unwrap(),
        "6ba7b810-9dad-41d1-80b4-00c04fd430c8"
    );
}

#[test]
fn authority_precedence_stable() {
    assert_eq!(
        strings(&authority()["authorityPrecedence"]),
        [
            "current_generation_direct_upstream_evidence",
            "current_shared_session_observation",
            "recovered_authoritative_read",
            "generation_tagged_historical_evidence",
            "stale_ui_cache_projection",
        ]
    );
}

#[test]
fn mutation_retry_replay_zero() {
    for field in [
        "resumeThreadRetry",
        "resumeThreadReplay",
        "approvalDecisionRetry",
        "approvalDecisionReplay",
        "threadDeleteRetry",
        "threadDeleteReplay",
        "upstreamUnsubscribeRetry",
        "upstreamUnsubscribeReplay",
    ] {
        assert_eq!(authority()["mutationPolicy"][field], 0, "policy {field}");
    }
    assert_eq!(authority()["mutationPolicy"]["forceTakeover"], false);
}

#[test]
fn fixture_families_are_present_and_sanitized() {
    let manifest = load_fixture("fixture-family-manifest.json");
    assert_eq!(manifest["schemaVersion"], 1);
    let families = manifest["families"].as_array().expect("fixture families");
    assert!(families.len() >= 26);
    for family in families {
        let path = repository_root().join(family["path"].as_str().expect("fixture path"));
        assert!(path.exists(), "missing fixture family {}", path.display());
        assert!(path.starts_with(repository_root()));
    }
}

#[test]
fn telemetry_classification_stable() {
    let telemetry = &authority()["telemetryClassification"];
    assert_eq!(
        strings(&telemetry["authoritativeState"]),
        [
            "RemoteHostIdentity",
            "WorkspaceSessionGeneration",
            "AppServerConnectionGeneration",
            "approvalObservation",
            "deleteObservation",
        ]
    );
    assert!(strings(&telemetry["diagnosticTelemetry"]).contains(&"eventGaps".to_string()));
    assert!(strings(&telemetry["uiProjection"]).contains(&"ProjectionFreshness".to_string()));
}

#[test]
fn telemetry_persistence_contract_stable() {
    let persistence = &authority()["telemetryPersistence"];
    assert_eq!(persistence["telemetryDatabase"], false);
    assert_eq!(persistence["persistentFreshness"], false);
    assert_eq!(persistence["persistentGapLedger"], false);
    assert_eq!(persistence["durableEventQueue"], false);
}

#[test]
fn remaining_not_proven_stable() {
    assert_eq!(
        strings(&authority()["remainingNotProven"]),
        ["daemon_broadcast_event_stream_completeness"]
    );
}

#[test]
fn forbidden_inference_absent() {
    assert_eq!(
        authority()["inferencePolicy"]["transportDisconnectedMeansThreadAbsent"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["uiEmptyMeansThreadAbsent"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["threadListMissingMeansDeleted"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["eventSilenceMeansIdle"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["eventSilenceMeansWriterFree"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["eventSilenceMeansNoApproval"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["staleCacheIsCanonicalTruth"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["newerTimestampMeansNewerCanonicalTruth"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["sameHostIdentityMeansSameSession"],
        false
    );
    assert_eq!(
        authority()["inferencePolicy"]["gapMutatesSharedAuthority"],
        false
    );
}

#[test]
fn forbidden_ownership_semantics_absent() {
    let values = [
        serde_json::to_value(current_snapshot()).unwrap(),
        serde_json::to_value(local_event()).unwrap(),
        load_fixture("fixture-family-manifest.json"),
    ];
    let forbidden = [
        "RecoveryGeneration",
        "EventGeneration",
        "RemoteClientIdentity",
        "UIAuthority",
        "ClientProjectionAuthority",
        "projectionOwner",
        "recoveryOwner",
        "writerOwner",
        "subscriptionOwner",
        "leaseId",
        "forceTakeover",
        "telemetryAuthority",
    ];
    for value in values {
        let mut keys = Vec::new();
        object_keys(&value, &mut keys);
        for forbidden in forbidden {
            assert!(
                !keys.iter().any(|key| key.eq_ignore_ascii_case(forbidden)),
                "forbidden schema field {forbidden}"
            );
        }
    }
}

#[test]
fn fixture_paths_remain_inside_repository() {
    for name in ["authority-contract.json", "fixture-family-manifest.json"] {
        assert!(fixture_path(name).starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")));
    }
}

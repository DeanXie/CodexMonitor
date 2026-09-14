use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionAttemptId, WriterAdmissionNonTransitionEvent,
    WriterAdmissionObservationQueryError, WriterAdmissionObservationRuntime,
    WriterAdmissionObservationSnapshot, WriterAdmissionObservationTracker,
    WriterAdmissionSessionEndEvidenceKind,
};
use crate::shared::codex_identity::CodexThreadKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

const FIXTURE_DIRECTORY: &str = "writer-admission-observation";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolFixture {
    schema_version: u32,
    scenario: String,
    request: FixtureRequest,
    response_or_error_evidence: Value,
    normalized: FixtureNormalized,
    attempt_provenance: FixtureAttemptProvenance,
    provenance: Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureRequest {
    method: String,
    workspace_id: String,
    full_thread_id: String,
    thread_key: CodexThreadKey,
    workspace_session_generation: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureAttemptProvenance {
    attempt_id: Option<String>,
    requested_full_thread_id: Option<String>,
    observed_at: Option<i64>,
    dispatch_status: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureNormalized {
    outcome: String,
    snapshot: Option<Value>,
    error: Option<String>,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("app-server")
        .join(FIXTURE_DIRECTORY)
        .join(name)
}

fn load_fixture(name: &str) -> ProtocolFixture {
    let path = fixture_path(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read protocol fixture {}: {error}", path.display()));
    let fixture: ProtocolFixture = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse protocol fixture {}: {error}", path.display()));
    assert_eq!(fixture.schema_version, 1);
    assert!(!fixture.scenario.trim().is_empty());
    assert!(!fixture.request.workspace_id.trim().is_empty());
    assert_eq!(
        fixture.request.thread_key.thread_id,
        fixture.request.full_thread_id
    );
    assert!(fixture.provenance.is_object());
    fixture
}

fn generation(fixture: &ProtocolFixture) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(fixture.request.workspace_session_generation.clone())
        .expect("fixture generation")
}

fn attempt(fixture: &ProtocolFixture) -> WriterAdmissionAttemptId {
    WriterAdmissionAttemptId::new(
        fixture
            .attempt_provenance
            .attempt_id
            .clone()
            .expect("fixture attempt id"),
    )
    .expect("fixture attempt")
}

fn pending_tracker(fixture: &ProtocolFixture) -> WriterAdmissionObservationTracker {
    let mut tracker = WriterAdmissionObservationTracker::new(
        fixture.request.thread_key.clone(),
        generation(fixture),
    );
    tracker
        .begin_resume(
            attempt(fixture),
            fixture
                .attempt_provenance
                .requested_full_thread_id
                .as_deref()
                .expect("requested full Thread id"),
            fixture
                .attempt_provenance
                .observed_at
                .expect("attempt observedAt"),
        )
        .expect("fixture pending transition");
    tracker
}

fn expected_snapshot(fixture: &ProtocolFixture) -> &Value {
    fixture
        .normalized
        .snapshot
        .as_ref()
        .expect("fixture snapshot")
}

fn actual_snapshot(tracker: &WriterAdmissionObservationTracker) -> Value {
    let snapshot = WriterAdmissionObservationSnapshot::from(
        tracker
            .latest_observation()
            .expect("fixture observation")
            .clone(),
    );
    serde_json::to_value(snapshot).expect("serialize snapshot")
}

#[test]
fn accepted_fixture_maps_to_admitted() {
    let fixture = load_fixture("exact-resume-accepted.json");
    let mut tracker = pending_tracker(&fixture);
    let returned_id = fixture.response_or_error_evidence["returnedFullThreadId"]
        .as_str()
        .expect("returned full Thread id");
    tracker
        .record_exact_resume_success(
            returned_id,
            fixture.response_or_error_evidence["observedAt"]
                .as_i64()
                .expect("response observedAt"),
        )
        .expect("accepted transition");

    assert_eq!(fixture.normalized.outcome, "snapshot");
    assert_eq!(actual_snapshot(&tracker), *expected_snapshot(&fixture));
}

#[test]
fn blocked_fixture_maps_to_blocked() {
    let fixture = load_fixture("active-writer-blocked.json");
    let mut tracker = pending_tracker(&fixture);
    tracker
        .record_active_writer_blocked(
            fixture.response_or_error_evidence["code"]
                .as_i64()
                .expect("error code"),
            fixture.response_or_error_evidence["message"]
                .as_str()
                .expect("error message"),
            fixture.response_or_error_evidence["observedAt"]
                .as_i64()
                .expect("error observedAt"),
        )
        .expect("blocked transition");

    assert_eq!(actual_snapshot(&tracker), *expected_snapshot(&fixture));
}

#[test]
fn unknown_fixture_maps_to_unknown() {
    let fixture = load_fixture("admission-outcome-unknown.json");
    let mut tracker = pending_tracker(&fixture);
    tracker
        .record_timeout(
            fixture.response_or_error_evidence["message"]
                .as_str()
                .expect("timeout message"),
            fixture.response_or_error_evidence["observedAt"]
                .as_i64()
                .expect("timeout observedAt"),
        )
        .expect("unknown transition");

    assert_eq!(actual_snapshot(&tracker), *expected_snapshot(&fixture));
}

#[test]
fn session_end_fixture_maps_to_release_unobserved() {
    let fixture = load_fixture("session-ended-release-unobserved.json");
    let mut tracker = pending_tracker(&fixture);
    tracker
        .record_exact_resume_success(
            &fixture.request.full_thread_id,
            fixture.response_or_error_evidence["admittedAt"]
                .as_i64()
                .expect("admittedAt"),
        )
        .expect("admitted transition");
    tracker
        .record_session_ended_with_evidence(
            &generation(&fixture),
            WriterAdmissionSessionEndEvidenceKind::AppServerProcessExited,
            fixture.response_or_error_evidence["message"]
                .as_str()
                .expect("session-end message"),
            fixture.response_or_error_evidence["observedAt"]
                .as_i64()
                .expect("session-end observedAt"),
        )
        .expect("session-end transition");

    assert_eq!(actual_snapshot(&tracker), *expected_snapshot(&fixture));
}

#[test]
fn not_observed_fixture_is_not_unavailable() {
    let fixture = load_fixture("current-generation-not-observed.json");
    let runtime = WriterAdmissionObservationRuntime::new(generation(&fixture));
    let snapshot = serde_json::to_value(runtime.current_snapshot(&fixture.request.thread_key))
        .expect("serialize not-observed snapshot");

    assert_eq!(fixture.normalized.outcome, "snapshot");
    assert!(fixture.normalized.error.is_none());
    assert_eq!(snapshot, *expected_snapshot(&fixture));
}

#[test]
fn workspace_unavailable_is_not_not_observed() {
    for (name, error) in [
        (
            "workspace-unavailable.json",
            WriterAdmissionObservationQueryError::WorkspaceNotFound,
        ),
        (
            "workspace-session-unavailable.json",
            WriterAdmissionObservationQueryError::WorkspaceSessionUnavailable,
        ),
    ] {
        let fixture = load_fixture(name);
        assert_eq!(fixture.normalized.outcome, "query_error");
        assert!(fixture.normalized.snapshot.is_none());
        let expected_error = error.to_string();
        assert_eq!(
            fixture.normalized.error.as_deref(),
            Some(expected_error.as_str())
        );
        assert_ne!(fixture.normalized.error.as_deref(), Some("not_observed"));
    }
}

#[test]
fn new_generation_does_not_inherit_old_snapshot() {
    let ended = load_fixture("session-ended-release-unobserved.json");
    let current = load_fixture("current-generation-not-observed.json");
    assert_ne!(
        ended.request.workspace_session_generation,
        current.request.workspace_session_generation
    );
    assert_eq!(
        expected_snapshot(&ended)["state"],
        "session_ended_release_unobserved"
    );
    assert_eq!(expected_snapshot(&current)["state"], "not_observed");
    assert!(expected_snapshot(&current)["attemptId"].is_null());
}

#[test]
fn app_daemon_serialization_parity() {
    for name in [
        "exact-resume-accepted.json",
        "active-writer-blocked.json",
        "admission-outcome-unknown.json",
        "session-ended-release-unobserved.json",
        "current-generation-not-observed.json",
    ] {
        let fixture = load_fixture(name);
        let snapshot: WriterAdmissionObservationSnapshot =
            serde_json::from_value(expected_snapshot(&fixture).clone())
                .expect("App snapshot contract");
        let app_payload = serde_json::to_value(&snapshot).expect("App serialization");
        let daemon_payload = serde_json::to_value(snapshot).expect("daemon serialization");
        assert_eq!(app_payload, daemon_payload, "fixture {name}");
        assert_eq!(app_payload, *expected_snapshot(&fixture), "fixture {name}");
    }
}

#[test]
fn no_free_available_released_states() {
    for name in fixture_names() {
        let fixture = load_fixture(name);
        let serialized = serde_json::to_string(&fixture.normalized).expect("serialize fixture");
        for forbidden in ["\"free\"", "\"available\"", "\"released\""] {
            assert!(
                !serialized.contains(forbidden),
                "{name} contains {forbidden}"
            );
        }
    }
}

#[test]
fn no_owner_or_lease_fields() {
    for name in fixture_names() {
        let fixture = load_fixture(name);
        let serialized = serde_json::to_string(&fixture).expect("serialize fixture");
        for forbidden in [
            "writerOwner",
            "leaseId",
            "remoteClientOwner",
            "ownerIdentity",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "{name} contains {forbidden}"
            );
        }
    }
}

#[test]
fn fixtures_contain_no_auth_secrets() {
    for name in fixture_names() {
        let fixture = load_fixture(name);
        let value = serde_json::to_value(fixture).expect("serialize fixture");
        assert_no_secret_keys(&value, name);
    }
    assert_no_secret_keys(&load_protocol_provenance(), "protocol-provenance.json");
}

#[test]
fn protocol_provenance_is_version_and_hash_pinned() {
    let provenance = load_protocol_provenance();
    assert_eq!(
        provenance["bundledAppServer"]["cliVersion"],
        "codex-cli 0.153.4"
    );
    assert_eq!(
        provenance["bundledAppServer"]["officialSourceCommit"],
        "3d2ee51ca2d5db578f328aa75e20aa22c0197c9a"
    );
    assert_eq!(
        provenance["bundledAppServer"]["executableSha256"],
        "444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B"
    );
    assert_eq!(
        provenance["upstreamReference"]["commit"],
        "e9633d7a0226eac91c7a791dc4f92cf8f25df2ae"
    );
}

#[test]
fn unsubscribe_does_not_mean_release() {
    let fixture = load_fixture("exact-resume-accepted.json");
    let mut tracker = pending_tracker(&fixture);
    tracker
        .record_exact_resume_success(
            &fixture.request.full_thread_id,
            fixture.response_or_error_evidence["observedAt"]
                .as_i64()
                .expect("response observedAt"),
        )
        .expect("accepted transition");
    let before = actual_snapshot(&tracker);
    tracker.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadUnsubscribe);
    assert_eq!(actual_snapshot(&tracker), before);

    let provenance = load_protocol_provenance();
    assert_eq!(
        provenance["codexMonitor"]["threadLiveUnsubscribe"]["dispatchesUpstream"],
        false
    );
    assert_eq!(
        provenance["normalizedContract"]["threadUnsubscribe"]["writerReleaseAcknowledgement"],
        false
    );
    assert_eq!(
        provenance["bundledAppServer"]["threadUnsubscribe"]["writerReleaseAcknowledgement"],
        false
    );
    assert_eq!(
        provenance["upstreamReference"]["threadUnsubscribe"]["writerReleaseAcknowledgement"],
        false
    );
}

#[test]
fn shared_clients_do_not_get_client_ownership() {
    let accepted = load_fixture("exact-resume-accepted.json");
    let snapshot = expected_snapshot(&accepted);
    assert!(snapshot.get("remoteClientOwner").is_none());
    assert!(snapshot.get("writerOwner").is_none());
    assert_eq!(snapshot["workspaceSessionGeneration"], "generation-a3");
}

fn fixture_names() -> &'static [&'static str] {
    &[
        "exact-resume-accepted.json",
        "active-writer-blocked.json",
        "admission-outcome-unknown.json",
        "session-ended-release-unobserved.json",
        "current-generation-not-observed.json",
        "workspace-unavailable.json",
        "workspace-session-unavailable.json",
    ]
}

fn load_protocol_provenance() -> Value {
    let path = fixture_path("protocol-provenance.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read protocol provenance {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse protocol provenance {}: {error}", path.display()))
}

fn assert_no_secret_keys(value: &Value, fixture_name: &str) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = key.to_ascii_lowercase();
                assert!(
                    !matches!(
                        normalized.as_str(),
                        "token" | "authtoken" | "authorization" | "password" | "secret"
                    ),
                    "{fixture_name} contains forbidden secret field {key}"
                );
                assert_no_secret_keys(child, fixture_name);
            }
        }
        Value::Array(items) => {
            for child in items {
                assert_no_secret_keys(child, fixture_name);
            }
        }
        _ => {}
    }
}

use super::thread_lifecycle_observation::{
    AppServerConnectionEndEvidenceKind, AppServerConnectionGeneration,
    ThreadLifecycleObservationRuntime, ThreadLifecycleObservationScope,
    ThreadRuntimeAvailabilityEvidenceSource, ThreadRuntimeAvailabilityState,
    ThreadSubscriptionEvidenceSource, ThreadSubscriptionObservationState,
    ThreadSubscriptionObservationTracker, ThreadSubscriptionOutcomeErrorKind,
};
use super::writer_admission_observation::{
    WorkspaceSessionGeneration, WriterAdmissionAttemptId, WriterAdmissionNonTransitionEvent,
    WriterAdmissionObservationSnapshot, WriterAdmissionObservationTracker,
};
use crate::backend::app_server::reconcile_thread_lifecycle_message;
use crate::shared::codex_identity::CodexThreadKey;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

const FIXTURE_DIRECTORY: &str = "thread-lifecycle-observation";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("app-server")
        .join(FIXTURE_DIRECTORY)
        .join(name)
}

fn load_fixture(name: &str) -> Value {
    let path = fixture_path(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("read protocol fixture {}: {error}", path.display()));
    let fixture: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse protocol fixture {}: {error}", path.display()));
    assert_eq!(fixture["schemaVersion"], 1, "fixture {name}");
    fixture
}

fn fixture_names() -> &'static [&'static str] {
    &[
        "subscription-states.json",
        "runtime-states.json",
        "synthetic-live-detach.json",
        "upstream-unsubscribe-outcomes.json",
        "thread-closed.json",
        "connection-generation-reset.json",
        "workspace-generation-reset.json",
        "multi-subscriber.json",
        "reconnect-new-generation.json",
    ]
}

fn scope_value(fixture: &Value) -> &Value {
    &fixture["scope"]
}

fn thread_key(fixture: &Value) -> CodexThreadKey {
    let scope = scope_value(fixture);
    CodexThreadKey::new(
        scope["threadKey"]["codexHomeIdentity"]
            .as_str()
            .expect("codexHomeIdentity"),
        scope["threadKey"]["threadId"].as_str().expect("threadId"),
    )
}

fn workspace_generation(fixture: &Value) -> WorkspaceSessionGeneration {
    WorkspaceSessionGeneration::new(
        scope_value(fixture)["workspaceSessionGeneration"]
            .as_str()
            .expect("workspaceSessionGeneration"),
    )
    .expect("workspace generation")
}

fn connection_generation(fixture: &Value) -> AppServerConnectionGeneration {
    AppServerConnectionGeneration::new(
        scope_value(fixture)["appServerConnectionGeneration"]
            .as_str()
            .expect("appServerConnectionGeneration"),
    )
    .expect("connection generation")
}

fn lifecycle_scope(fixture: &Value) -> ThreadLifecycleObservationScope {
    ThreadLifecycleObservationScope::new(
        workspace_generation(fixture),
        connection_generation(fixture),
        thread_key(fixture),
    )
}

fn subscribed_tracker(fixture: &Value) -> ThreadSubscriptionObservationTracker {
    let mut tracker = ThreadSubscriptionObservationTracker::new(lifecycle_scope(fixture));
    tracker
        .record_subscribed(ThreadSubscriptionEvidenceSource::ThreadResumeResponse, 100)
        .expect("subscription evidence");
    tracker
}

fn pending_runtime(
    fixture: &Value,
) -> (
    ThreadLifecycleObservationRuntime,
    super::thread_lifecycle_observation::ThreadUnsubscribeAttempt,
) {
    let key = thread_key(fixture);
    let runtime = ThreadLifecycleObservationRuntime::new(
        workspace_generation(fixture),
        connection_generation(fixture),
    );
    runtime
        .record_subscribed(
            key.clone(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            100,
        )
        .expect("subscription evidence");
    let attempt = runtime
        .begin_unsubscribe(key.clone(), &key.thread_id, 110)
        .expect("unsubscribe pending");
    (runtime, attempt)
}

fn admitted_writer(fixture: &Value) -> WriterAdmissionObservationTracker {
    let key = thread_key(fixture);
    let mut tracker =
        WriterAdmissionObservationTracker::new(key.clone(), workspace_generation(fixture));
    tracker
        .begin_resume(
            WriterAdmissionAttemptId::new("fixture-writer-attempt").expect("writer attempt"),
            &key.thread_id,
            80,
        )
        .expect("writer pending");
    tracker
        .record_exact_resume_success(&key.thread_id, 90)
        .expect("writer admitted");
    tracker
}

fn writer_snapshot(tracker: &WriterAdmissionObservationTracker) -> Value {
    serde_json::to_value(WriterAdmissionObservationSnapshot::from(
        tracker
            .latest_observation()
            .expect("writer observation")
            .clone(),
    ))
    .expect("writer snapshot")
}

fn assert_common_scope(fixture: &Value) {
    let scope = scope_value(fixture);
    assert!(scope["workspaceSessionGeneration"].is_string());
    assert!(scope["appServerConnectionGeneration"].is_string());
    assert!(scope["threadKey"]["codexHomeIdentity"].is_string());
    assert!(scope["threadKey"]["threadId"].is_string());
}

#[test]
fn serialization_schema_is_stable() {
    let fixture = load_fixture("subscription-states.json");
    assert_common_scope(&fixture);
    let serialized = [
        ThreadSubscriptionObservationState::NotObserved,
        ThreadSubscriptionObservationState::SubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::UnsubscribePending,
        ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::NotSubscribedForAppServerConnection,
        ThreadSubscriptionObservationState::UnsubscribeOutcomeUnknown,
    ]
    .into_iter()
    .map(|state| serde_json::to_value(state).expect("serialize subscription state"))
    .collect::<Vec<_>>();
    assert_eq!(json!(serialized), fixture["states"]);

    let runtime = load_fixture("runtime-states.json");
    assert_eq!(
        json!([
            ThreadRuntimeAvailabilityState::Unknown,
            ThreadRuntimeAvailabilityState::LoadedObserved,
            ThreadRuntimeAvailabilityState::NotLoadedObserved,
        ]
        .into_iter()
        .map(|state| serde_json::to_value(state).expect("serialize runtime state"))
        .collect::<Vec<_>>()),
        runtime["states"].clone()
    );

    let snapshot = subscribed_tracker(&fixture).snapshot();
    let keys = serde_json::to_value(snapshot)
        .expect("subscription snapshot")
        .as_object()
        .expect("snapshot object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        fixture["subscriptionSnapshotFields"]
            .as_array()
            .expect("snapshot fields")
            .iter()
            .map(|value| value.as_str().expect("field").to_string())
            .collect()
    );
}

#[test]
fn synthetic_and_upstream_unsubscribe_remain_distinct() {
    let fixture = load_fixture("synthetic-live-detach.json");
    assert_eq!(fixture["operation"], "thread_live_unsubscribe");
    assert_eq!(fixture["classification"], "local_synthetic_detach_only");
    assert_eq!(fixture["upstreamDispatchCount"], 0);
    assert_eq!(fixture["upstreamOperation"], "thread_upstream_unsubscribe");
    assert_eq!(fixture["upstreamMethod"], "thread/unsubscribe");
}

#[test]
fn unsubscribed_fixture_maps_correctly() {
    let fixture = load_fixture("upstream-unsubscribe-outcomes.json");
    let (runtime, attempt) = pending_runtime(&fixture);
    runtime
        .record_response(
            &attempt,
            &json!({ "result": fixture["outcomes"]["unsubscribed"]["responseOrErrorEvidence"] }),
            120,
        )
        .expect("unsubscribed");
    assert_eq!(
        serde_json::to_value(runtime.subscription_snapshot(&thread_key(&fixture)))
            .expect("snapshot")["state"],
        fixture["outcomes"]["unsubscribed"]["subscriptionState"]
    );
}

#[test]
fn not_subscribed_fixture_maps_correctly() {
    let fixture = load_fixture("upstream-unsubscribe-outcomes.json");
    let (runtime, attempt) = pending_runtime(&fixture);
    runtime
        .record_response(
            &attempt,
            &json!({ "result": fixture["outcomes"]["notSubscribed"]["responseOrErrorEvidence"] }),
            120,
        )
        .expect("notSubscribed");
    assert_eq!(
        serde_json::to_value(runtime.subscription_snapshot(&thread_key(&fixture)))
            .expect("snapshot")["state"],
        fixture["outcomes"]["notSubscribed"]["subscriptionState"]
    );
}

#[test]
fn not_loaded_fixture_updates_only_subscription_and_runtime() {
    let fixture = load_fixture("upstream-unsubscribe-outcomes.json");
    let key = thread_key(&fixture);
    let runtime = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    runtime
        .record_subscribed(
            key.clone(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            100,
        )
        .expect("subscribed");
    let attempt = runtime
        .begin_unsubscribe(key.clone(), &key.thread_id, 110)
        .expect("pending");
    runtime
        .record_response(
            &attempt,
            &json!({ "result": { "status": "notLoaded" } }),
            120,
        )
        .expect("notLoaded response");

    assert_eq!(
        serde_json::to_value(runtime.subscription_snapshot(&key)).expect("subscription")["state"],
        fixture["outcomes"]["notLoaded"]["subscriptionState"]
    );
    assert_eq!(
        serde_json::to_value(runtime.runtime_state(&key)).expect("runtime state"),
        fixture["outcomes"]["notLoaded"]["runtimeState"]
    );
    let mut writer = admitted_writer(&fixture);
    let before = writer_snapshot(&writer);
    writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadRuntimeNotLoaded);
    assert_eq!(writer_snapshot(&writer), before);
}

#[test]
fn unknown_outcome_fixture_does_not_retry() {
    let fixture = load_fixture("upstream-unsubscribe-outcomes.json");
    let (runtime, attempt) = pending_runtime(&fixture);
    runtime
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::Timeout,
            fixture["outcomes"]["outcomeUnknown"]["responseOrErrorEvidence"]["message"]
                .as_str()
                .expect("timeout diagnostic"),
            120,
        )
        .expect("unknown outcome");
    assert_eq!(
        serde_json::to_value(runtime.subscription_snapshot(&thread_key(&fixture)))
            .expect("snapshot")["state"],
        fixture["outcomes"]["outcomeUnknown"]["subscriptionState"]
    );
    assert_eq!(
        fixture["outcomes"]["outcomeUnknown"]["automaticRetryCount"],
        0
    );
}

#[test]
fn delayed_closed_does_not_rewrite_unsubscribe_outcome() {
    let fixture = load_fixture("thread-closed.json");
    let key = thread_key(&fixture);
    let runtime = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    runtime
        .record_subscribed(
            key.clone(),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            100,
        )
        .expect("subscribed");
    let attempt = runtime
        .begin_unsubscribe(key.clone(), &key.thread_id, 110)
        .expect("pending");
    runtime
        .record_outcome_unknown(
            &attempt,
            ThreadSubscriptionOutcomeErrorKind::DispatchDisconnected,
            "fixture response lost",
            120,
        )
        .expect("unknown outcome");
    assert!(reconcile_thread_lifecycle_message(
        &runtime,
        &key.codex_home_identity,
        &fixture["notification"],
        130,
    ));
    assert_eq!(
        serde_json::to_value(runtime.subscription_snapshot(&key)).expect("subscription")["state"],
        fixture["expected"]["subscriptionState"]
    );
    assert_eq!(
        serde_json::to_value(runtime.runtime_state(&key)).expect("runtime"),
        fixture["expected"]["runtimeState"]
    );
}

#[test]
fn thread_status_not_loaded_fixture_uses_production_ingestion() {
    let fixture = load_fixture("runtime-states.json");
    let key = thread_key(&fixture);
    let runtime = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    assert!(reconcile_thread_lifecycle_message(
        &runtime,
        &key.codex_home_identity,
        &fixture["runtimeEvidence"]["statusNotLoadedMessage"],
        fixture["observedAt"].as_i64().expect("observedAt"),
    ));
    assert_eq!(
        serde_json::to_value(runtime.runtime_state(&key)).expect("runtime state"),
        json!("not_loaded_observed")
    );
    assert_eq!(
        serde_json::to_value(
            runtime
                .runtime_observation(&key)
                .expect("runtime observation")
                .evidence_source,
        )
        .expect("runtime source"),
        fixture["runtimeEvidence"]["statusNotLoadedSource"]
    );
}

#[test]
fn reconnect_starts_new_connection_generation() {
    let fixture = load_fixture("reconnect-new-generation.json");
    let old = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    old.record_subscribed(
        thread_key(&fixture),
        ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
        100,
    )
    .expect("old subscribed");
    let next = old.for_new_app_server_connection_generation(
        AppServerConnectionGeneration::new(
            fixture["newAppServerConnectionGeneration"]
                .as_str()
                .expect("new connection generation"),
        )
        .expect("connection generation"),
    );
    assert_eq!(
        serde_json::to_value(next.subscription_snapshot(&thread_key(&fixture)))
            .expect("new snapshot")["state"],
        fixture["expected"]["subscriptionState"]
    );
    assert_eq!(
        next.app_server_connection_generation().as_str(),
        fixture["newAppServerConnectionGeneration"]
    );
    assert_eq!(
        serde_json::to_value(next.runtime_state(&thread_key(&fixture))).expect("runtime"),
        fixture["expected"]["runtimeState"]
    );
    assert_eq!(fixture["expected"]["automaticRetryCount"], 0);
}

#[test]
fn connection_generation_reset_does_not_inherit_current_models() {
    let fixture = load_fixture("connection-generation-reset.json");
    let key = thread_key(&fixture);
    let old = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    old.record_subscribed(
        key.clone(),
        ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
        100,
    )
    .expect("old subscribed");
    old.record_runtime_not_loaded(
        key.clone(),
        ThreadRuntimeAvailabilityEvidenceSource::ThreadClosedNotification,
        110,
    );
    old.record_connection_ended(
        AppServerConnectionEndEvidenceKind::TransportDisconnected,
        fixture["responseOrErrorEvidence"]["diagnostic"]
            .as_str()
            .expect("connection diagnostic"),
        fixture["observedAt"].as_i64().expect("observedAt"),
    );
    let history = old.connection_end_history();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].kind,
        AppServerConnectionEndEvidenceKind::TransportDisconnected
    );
    assert_eq!(
        fixture["responseOrErrorEvidence"]["connectionEnd"],
        "transport_disconnected"
    );
    let next = old.for_new_app_server_connection_generation(
        AppServerConnectionGeneration::new(
            fixture["newAppServerConnectionGeneration"]
                .as_str()
                .expect("new connection generation"),
        )
        .expect("connection generation"),
    );
    assert_eq!(
        serde_json::to_value(next.subscription_snapshot(&key)).expect("subscription")["state"],
        fixture["expected"]["subscriptionState"]
    );
    assert_eq!(
        serde_json::to_value(next.runtime_state(&key)).expect("runtime"),
        fixture["expected"]["runtimeState"]
    );
    assert_eq!(fixture["expected"]["ambiguousUnsubscribeReplayCount"], 0);
}

#[test]
fn new_workspace_generation_resets_current_models() {
    let fixture = load_fixture("workspace-generation-reset.json");
    let old_lifecycle = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        connection_generation(&fixture),
    );
    old_lifecycle
        .record_subscribed(
            thread_key(&fixture),
            ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
            100,
        )
        .expect("old subscribed");
    old_lifecycle.record_runtime_not_loaded(
        thread_key(&fixture),
        ThreadRuntimeAvailabilityEvidenceSource::ThreadClosedNotification,
        110,
    );
    let new_workspace = WorkspaceSessionGeneration::new(
        fixture["newWorkspaceSessionGeneration"]
            .as_str()
            .expect("new workspace generation"),
    )
    .expect("workspace generation");
    let next = old_lifecycle.for_new_workspace_session_generation(
        new_workspace.clone(),
        AppServerConnectionGeneration::new(
            fixture["newAppServerConnectionGeneration"]
                .as_str()
                .expect("new connection generation"),
        )
        .expect("connection generation"),
    );
    let new_writer =
        super::writer_admission_observation::WriterAdmissionObservationRuntime::new(new_workspace);
    assert_eq!(
        serde_json::to_value(next.subscription_snapshot(&thread_key(&fixture)))
            .expect("subscription")["state"],
        fixture["expected"]["subscriptionState"]
    );
    assert_eq!(
        serde_json::to_value(next.runtime_state(&thread_key(&fixture))).expect("runtime"),
        fixture["expected"]["runtimeState"]
    );
    assert_eq!(
        serde_json::to_value(new_writer.current_snapshot(&thread_key(&fixture))).expect("writer")
            ["state"],
        fixture["expected"]["writerState"]
    );
}

#[test]
fn multi_subscriber_fixture_preserves_other_subscription() {
    let fixture = load_fixture("multi-subscriber.json");
    let key = thread_key(&fixture);
    let connection_a = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        AppServerConnectionGeneration::new("connection-a").expect("connection A"),
    );
    let connection_b = ThreadLifecycleObservationRuntime::new(
        workspace_generation(&fixture),
        AppServerConnectionGeneration::new("connection-b").expect("connection B"),
    );
    for runtime in [&connection_a, &connection_b] {
        runtime
            .record_subscribed(
                key.clone(),
                ThreadSubscriptionEvidenceSource::ThreadResumeResponse,
                100,
            )
            .expect("subscribed");
    }
    let attempt = connection_a
        .begin_unsubscribe(key.clone(), &key.thread_id, 110)
        .expect("A pending");
    connection_a
        .record_response(
            &attempt,
            &json!({ "result": { "status": "unsubscribed" } }),
            120,
        )
        .expect("A unsubscribed");
    assert_eq!(
        serde_json::to_value(connection_a.subscription_snapshot(&key)).expect("A")["state"],
        fixture["expected"]["connectionAState"]
    );
    assert_eq!(
        serde_json::to_value(connection_b.subscription_snapshot(&key)).expect("B")["state"],
        fixture["expected"]["connectionBState"]
    );
}

#[test]
fn subscription_events_do_not_mutate_writer() {
    for name in [
        "synthetic-live-detach.json",
        "upstream-unsubscribe-outcomes.json",
        "thread-closed.json",
    ] {
        let fixture = load_fixture(name);
        let mut writer = admitted_writer(&fixture);
        let before = writer_snapshot(&writer);
        writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadUnsubscribe);
        writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadClosed);
        writer.observe_non_transition(WriterAdmissionNonTransitionEvent::ThreadRuntimeNotLoaded);
        assert_eq!(writer_snapshot(&writer), before, "fixture {name}");
        assert_eq!(
            fixture["writerObservationTransition"], false,
            "fixture {name}"
        );
    }
}

#[test]
fn no_free_available_released_states() {
    for name in fixture_names()
        .iter()
        .copied()
        .chain(["protocol-provenance.json"])
    {
        let text = serde_json::to_string(&load_fixture(name)).expect("serialize fixture");
        for forbidden in ["\"free\"", "\"available\"", "\"released\""] {
            assert!(
                !text.contains(forbidden),
                "fixture {name} contains {forbidden}"
            );
        }
    }
}

#[test]
fn no_owner_or_lease_fields() {
    for name in fixture_names()
        .iter()
        .copied()
        .chain(["protocol-provenance.json"])
    {
        let fixture = load_fixture(name);
        assert_no_forbidden_keys(&fixture, name);
    }
}

#[test]
fn unavailable_not_observed_and_unknown_remain_distinct() {
    let provenance = load_fixture("protocol-provenance.json");
    assert_eq!(
        provenance["normalizedContract"]["workspaceMissing"],
        "query_error"
    );
    assert_eq!(
        provenance["normalizedContract"]["workspaceSessionUnavailable"],
        "query_error"
    );
    assert_eq!(
        provenance["normalizedContract"]["subscriptionWithoutEvidence"],
        "not_observed"
    );
    assert_eq!(
        provenance["normalizedContract"]["runtimeWithoutEvidence"],
        "unknown"
    );
}

#[test]
fn bundled_and_upstream_provenance_are_pinned_separately() {
    let provenance = load_fixture("protocol-provenance.json");
    assert_eq!(
        provenance["bundledBehavior"]["cliVersion"],
        "codex-cli 0.153.4"
    );
    assert_eq!(
        provenance["bundledBehavior"]["executableSha256"],
        "444A3F0008050605CAE73CD9B7A2DCAC61294062DFAAB56DD20430FD6498518B"
    );
    assert_eq!(
        provenance["bundledBehavior"]["officialSourceCommit"],
        "3d2ee51ca2d5db578f328aa75e20aa22c0197c9a"
    );
    assert_eq!(
        provenance["currentUpstreamReference"]["commit"],
        "e9633d7a0226eac91c7a791dc4f92cf8f25df2ae"
    );
    assert_eq!(
        provenance["codexMonitorNormalizedContract"]["automaticRetryCount"],
        0
    );
}

fn assert_no_forbidden_keys(value: &Value, fixture_name: &str) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = key.to_ascii_lowercase();
                assert!(
                    !matches!(
                        normalized.as_str(),
                        "token"
                            | "authtoken"
                            | "authorization"
                            | "password"
                            | "secret"
                            | "writerowner"
                            | "leaseid"
                            | "remoteclientowner"
                            | "remoteclientsubscriptionowner"
                    ),
                    "fixture {fixture_name} contains forbidden field {key}"
                );
                assert_no_forbidden_keys(child, fixture_name);
            }
        }
        Value::Array(items) => {
            for child in items {
                assert_no_forbidden_keys(child, fixture_name);
            }
        }
        _ => {}
    }
}

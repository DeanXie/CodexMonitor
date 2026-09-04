use serde_json::json;

use super::codex_core::creation_coordination::{CreationCoordinator, IntentId};
use super::execution_settings_evidence::{
    ExecutionSettingField, ExecutionSettingValue, ExecutionSettingsAssessment,
    ExecutionSettingsEvidenceLayer, ExecutionSettingsObservationKey,
};
use super::execution_settings_ingestion::ExecutionSettingsEvidenceRuntime;
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::global_sources_core::rollout_watcher::RolloutTurnContextSettingsObservation;
use super::workspace_interop_core::{
    ExecutionEnvironmentKey, RootLocatorPlatform, RuntimeOriginWorkspaceObservation,
    RuntimeWorkspaceReconciler,
};

const HOME: &str = "codex-home:fixture";

fn thread(id: &str) -> CodexThreadKey {
    CodexThreadKey::new(HOME, id)
}

fn turn_request(thread_id: &str) -> serde_json::Value {
    json!({
        "threadId": thread_id,
        "model": "gpt-monitor",
        "effort": "medium",
        "approvalPolicy": "on-request",
        "sandboxPolicy": {
            "type": "workspaceWrite",
            "networkAccess": true,
            "writableRoots": ["C:\\fixture\\workspace"]
        },
        "cwd": "C:\\fixture\\workspace",
        "collaborationMode": { "mode": "default" }
    })
}

fn turn_response(turn_id: &str) -> serde_json::Value {
    json!({ "result": { "turn": { "id": turn_id, "status": "inProgress" } } })
}

#[test]
fn outgoing_turn_request_records_requested_settings_after_full_turn_id_binding() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_outgoing_request(HOME, 41, "turn/start", &turn_request("thread-1"), 10);

    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    assert!(runtime
        .history(&key, ExecutionSettingField::Model)
        .is_empty());
    assert_eq!(runtime.pending_request_count(), 1);

    runtime.observe_app_server_response(HOME, 41, "turn/start", &turn_response("turn-1"), 20);

    let model = runtime.select(&key, ExecutionSettingField::Model);
    assert_eq!(model.assessment, ExecutionSettingsAssessment::RequestedOnly);
    assert_eq!(
        model.requested[0].value,
        ExecutionSettingValue::Text("gpt-monitor".into())
    );
    assert_eq!(model.requested[0].provenance.comparison_id, "turn:turn-1");
    assert_eq!(model.requested[0].provenance.source, "monitor-request");
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::NetworkAccess)[0].value,
        ExecutionSettingValue::Bool(true)
    );
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::WritableRoots)[0].value,
        ExecutionSettingValue::StringList(vec!["C:\\fixture\\workspace".into()])
    );
    for field in [
        ExecutionSettingField::Effort,
        ExecutionSettingField::ApprovalPolicy,
        ExecutionSettingField::SandboxPolicy,
        ExecutionSettingField::Cwd,
        ExecutionSettingField::CollaborationMode,
    ] {
        assert_eq!(runtime.history(&key, field).len(), 1, "missing {field:?}");
    }
    assert_eq!(runtime.pending_request_count(), 0);
}

#[test]
fn abandoned_request_does_not_leave_uncorrelatable_pending_evidence() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    assert!(runtime.observe_outgoing_request(
        HOME,
        8,
        "turn/start",
        &json!({ "threadId": "thread-1", "model": "gpt-requested" }),
        10,
    ));

    assert!(runtime.forget_outgoing_request(HOME, 8));
    assert_eq!(runtime.pending_request_count(), 0);
    assert!(!runtime.observe_app_server_response(
        HOME,
        8,
        "turn/start",
        &turn_response("late-turn"),
        20,
    ));
}

#[test]
fn omitted_field_is_not_fabricated_as_requested() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_outgoing_request(
        HOME,
        42,
        "turn/start",
        &json!({ "threadId": "thread-1", "model": "gpt-a" }),
        10,
    );
    runtime.observe_app_server_response(HOME, 42, "turn/start", &turn_response("turn-2"), 20);
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-2");

    assert!(runtime
        .history(&key, ExecutionSettingField::Effort)
        .is_empty());
}

#[test]
fn explicit_null_is_preserved_as_distinct_request_evidence() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_outgoing_request(
        HOME,
        43,
        "turn/start",
        &json!({ "threadId": "thread-1", "model": null }),
        10,
    );
    runtime.observe_app_server_response(HOME, 43, "turn/start", &turn_response("turn-3"), 20);
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-3");

    assert_eq!(
        runtime.history(&key, ExecutionSettingField::Model)[0].value,
        ExecutionSettingValue::Null
    );
}

#[test]
fn thread_start_response_records_thread_default_effective_settings() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_outgoing_request(
        HOME,
        44,
        "thread/start",
        &json!({ "cwd": "C:\\fixture\\workspace", "approvalPolicy": "on-request" }),
        10,
    );
    runtime.observe_app_server_response(
        HOME,
        44,
        "thread/start",
        &json!({
            "result": {
                "thread": { "id": "thread-created" },
                "model": "gpt-effective",
                "reasoningEffort": "high",
                "approvalPolicy": "on-request",
                "sandbox": {
                    "type": "workspaceWrite",
                    "networkAccess": false,
                    "writableRoots": []
                }
            }
        }),
        20,
    );
    let key = ExecutionSettingsObservationKey::thread_default(thread("thread-created"));

    assert_eq!(
        runtime
            .select(&key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::EffectiveConfirmed
    );
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::NetworkAccess)[0].value,
        ExecutionSettingValue::Bool(false)
    );
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::Model)[0]
            .provenance
            .source,
        "app-server-response"
    );
}

#[test]
fn thread_settings_updated_without_turn_id_stays_thread_default() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_app_server_notification(
        HOME,
        &json!({
            "method": "thread/settings/updated",
            "params": {
                "threadId": "thread-1",
                "threadSettings": { "model": "gpt-snapshot", "effort": "low" }
            }
        }),
        "notification-1",
        30,
    );

    let default_key = ExecutionSettingsObservationKey::thread_default(thread("thread-1"));
    let turn_key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-nearest");
    assert_eq!(
        runtime
            .select(&default_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::EffectiveConfirmed
    );
    assert_eq!(
        runtime.history(&default_key, ExecutionSettingField::Model)[0]
            .provenance
            .source,
        "app-server-settings-notification"
    );
    assert!(runtime
        .history(&turn_key, ExecutionSettingField::Model)
        .is_empty());
}

#[test]
fn turn_context_records_persisted_turn_settings_with_full_turn_id() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_rollout_turn_context(
        thread("thread-cli"),
        &json!({
            "turn_id": "turn-cli-1",
            "model": "gpt-cli",
            "effort": "high",
            "approval_policy": "never",
            "sandbox_policy": { "type": "workspaceWrite" },
            "network_access": false,
            "writable_roots": ["C:\\fixture\\cli"],
            "cwd": "C:\\fixture\\cli",
            "collaboration_mode": { "mode": "default" }
        }),
        "rollout-observation-1",
        40,
    );
    let key = ExecutionSettingsObservationKey::turn(thread("thread-cli"), "turn-cli-1");

    let model = runtime.select(&key, ExecutionSettingField::Model);
    assert_eq!(
        model.assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
    assert_eq!(
        model.persisted_observed[0].provenance.comparison_id,
        "turn:turn-cli-1"
    );
    assert_eq!(
        model.persisted_observed[0].provenance.source,
        "rollout-turn-context:rollout-observation-1"
    );
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::NetworkAccess)[0].value,
        ExecutionSettingValue::Bool(false)
    );
    assert_eq!(
        runtime.history(&key, ExecutionSettingField::WritableRoots)[0].value,
        ExecutionSettingValue::StringList(vec!["C:\\fixture\\cli".into()])
    );
}

#[test]
fn unrelated_turns_never_share_comparison_group_or_overwrite_each_other() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    for (request_id, turn_id, model) in [(51, "turn-a", "gpt-a"), (52, "turn-b", "gpt-b")] {
        runtime.observe_outgoing_request(
            HOME,
            request_id,
            "turn/start",
            &json!({ "threadId": "thread-shared", "model": model }),
            request_id,
        );
        runtime.observe_app_server_response(
            HOME,
            request_id,
            "turn/start",
            &turn_response(turn_id),
            request_id + 100,
        );
    }

    let a = ExecutionSettingsObservationKey::turn(thread("thread-shared"), "turn-a");
    let b = ExecutionSettingsObservationKey::turn(thread("thread-shared"), "turn-b");
    assert_eq!(
        runtime.history(&a, ExecutionSettingField::Model)[0]
            .provenance
            .comparison_id,
        "turn:turn-a"
    );
    assert_eq!(
        runtime.history(&b, ExecutionSettingField::Model)[0]
            .provenance
            .comparison_id,
        "turn:turn-b"
    );
    assert_ne!(
        runtime.select(&a, ExecutionSettingField::Model),
        runtime.select(&b, ExecutionSettingField::Model)
    );
}

#[test]
fn request_effective_and_observed_assessments_use_only_proven_correlation() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    let key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    runtime.observe_correlated_fields(
        key.clone(),
        ExecutionSettingsEvidenceLayer::Requested,
        "monitor-request",
        "turn:turn-1",
        &json!({ "model": "gpt-a" }),
        10,
    );
    runtime.observe_correlated_fields(
        key.clone(),
        ExecutionSettingsEvidenceLayer::ServerEffective,
        "app-server-response",
        "turn:turn-1",
        &json!({ "model": "gpt-a" }),
        20,
    );
    assert_eq!(
        runtime
            .select(&key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::Match
    );

    runtime.observe_correlated_fields(
        key.clone(),
        ExecutionSettingsEvidenceLayer::PersistedObserved,
        "rollout-turn-context",
        "turn:turn-1",
        &json!({ "model": "gpt-b" }),
        30,
    );
    assert_eq!(
        runtime
            .select(&key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::Conflict
    );
}

#[test]
fn d2_cross_surface_settings_change_keeps_same_thread_identity_and_distinct_turns() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_rollout_turn_context(
        thread("thread-d2"),
        &json!({
            "turn_id": "turn-monitor",
            "approval_policy": "on-request",
            "sandbox_policy": { "type": "workspaceWrite", "network_access": true }
        }),
        "rollout-monitor",
        10,
    );
    runtime.observe_rollout_turn_context(
        thread("thread-d2"),
        &json!({
            "turn_id": "turn-cli",
            "approval_policy": "never",
            "sandbox_policy": { "type": "workspaceWrite", "network_access": false }
        }),
        "rollout-cli",
        20,
    );

    let monitor = ExecutionSettingsObservationKey::turn(thread("thread-d2"), "turn-monitor");
    let cli = ExecutionSettingsObservationKey::turn(thread("thread-d2"), "turn-cli");
    assert_eq!(monitor.thread_key, cli.thread_key);
    assert_eq!(
        runtime.history(&monitor, ExecutionSettingField::ApprovalPolicy)[0].value,
        ExecutionSettingValue::Text("on-request".into())
    );
    assert_eq!(
        runtime.history(&cli, ExecutionSettingField::ApprovalPolicy)[0].value,
        ExecutionSettingValue::Text("never".into())
    );
}

#[test]
fn uncorrelated_thread_snapshot_does_not_create_turn_conflict() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_rollout_turn_context(
        thread("thread-1"),
        &json!({ "turn_id": "turn-1", "model": "gpt-turn" }),
        "rollout-1",
        10,
    );
    runtime.observe_app_server_notification(
        HOME,
        &json!({
            "method": "thread/settings/updated",
            "params": { "threadId": "thread-1", "threadSettings": { "model": "gpt-default" } }
        }),
        "notification-2",
        20,
    );
    let turn_key = ExecutionSettingsObservationKey::turn(thread("thread-1"), "turn-1");
    assert_eq!(
        runtime
            .select(&turn_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
}

#[test]
fn rollout_report_observations_are_ingested_into_the_process_store() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    assert_eq!(
        runtime.observe_rollout_observations([RolloutTurnContextSettingsObservation {
            thread_key: thread("thread-report"),
            turn_context: json!({ "turn_id": "turn-report", "model": "gpt-report" }),
            observation_id: "rollout:report-1".into(),
            observed_timestamp_ms: 55,
        }]),
        1
    );
    let key = ExecutionSettingsObservationKey::turn(thread("thread-report"), "turn-report");
    assert_eq!(
        runtime
            .select(&key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::ObservedConfirmed
    );
}

#[test]
fn requested_observed_match_and_mismatch_are_computed_from_same_turn_id() {
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    for (request_id, turn_id, requested, observed, expected) in [
        (
            81,
            "turn-match",
            "gpt-a",
            "gpt-a",
            ExecutionSettingsAssessment::Match,
        ),
        (
            82,
            "turn-mismatch",
            "gpt-a",
            "gpt-b",
            ExecutionSettingsAssessment::Mismatch,
        ),
    ] {
        runtime.observe_outgoing_request(
            HOME,
            request_id,
            "turn/start",
            &json!({ "threadId": "thread-assess", "model": requested }),
            10,
        );
        runtime.observe_app_server_response(
            HOME,
            request_id,
            "turn/start",
            &turn_response(turn_id),
            20,
        );
        runtime.observe_rollout_turn_context(
            thread("thread-assess"),
            &json!({ "turn_id": turn_id, "model": observed }),
            &format!("rollout-{turn_id}"),
            30,
        );
        let key = ExecutionSettingsObservationKey::turn(thread("thread-assess"), turn_id);
        assert_eq!(
            runtime
                .select(&key, ExecutionSettingField::Model)
                .assessment,
            expected
        );
    }
}

#[test]
fn settings_ingestion_does_not_change_creation_coordination() {
    let coordinator = CreationCoordinator::default();
    let context = coordinator.context();
    let intent = IntentId {
        process_epoch: context["processEpoch"].as_str().unwrap().into(),
        id: "00000000-0000-4000-8000-000000000099".into(),
    };
    let before = coordinator.creation_status(&intent).unwrap();
    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_rollout_turn_context(
        thread("thread-independent"),
        &json!({ "turn_id": "turn-independent", "model": "gpt-a" }),
        "rollout-independent",
        1,
    );
    assert_eq!(coordinator.creation_status(&intent).unwrap(), before);
}

#[test]
fn settings_ingestion_does_not_change_workspace_identity() {
    let mut workspace = RuntimeWorkspaceReconciler::new(
        HOME,
        ExecutionEnvironmentKey::new("monitor-runtime").unwrap(),
        RootLocatorPlatform::Windows,
    );
    workspace.register_workspace("workspace-a", "C:\\fixture");
    workspace.observe_origin(RuntimeOriginWorkspaceObservation {
        thread_id: "thread-independent",
        thread_start_cwd: Some("C:\\fixture\\child"),
        session_meta_cwd: None,
        confirmed_parent_thread_id: None,
        observed_at: 1,
    });
    let before = workspace.route_for_origin("thread-independent");

    let runtime = ExecutionSettingsEvidenceRuntime::default();
    runtime.observe_rollout_turn_context(
        thread("thread-independent"),
        &json!({ "turn_id": "turn-independent", "cwd": "D:\\elsewhere" }),
        "rollout-independent",
        1,
    );

    assert_eq!(workspace.route_for_origin("thread-independent"), before);
}

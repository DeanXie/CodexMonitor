use super::codex_core::creation_coordination::{CreationCoordinator, IntentId, TurnIntent};
use super::execution_settings_evidence::{
    ExecutionSettingField, ExecutionSettingValue, ExecutionSettingsAssessment,
    ExecutionSettingsEvidenceLayer, ExecutionSettingsEvidenceRecord,
    ExecutionSettingsEvidenceStore, ExecutionSettingsObservationKey, ExecutionSettingsProvenance,
};
use super::global_sources_core::rollout_discovery::CodexHomeSource;
use super::global_sources_core::rollout_identity::CodexThreadKey;
use super::global_sources_core::rollout_watcher::{
    RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy,
};
use super::global_sources_core::runtime_config::discover_runtime_codex_homes;
use crate::backend::app_server::spawn_workspace_session;
use crate::backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput};
use crate::shared::codex_core::{send_user_message_core, start_thread_core};
use crate::types::{WorkspaceEntry, WorkspaceKind, WorkspaceSettings};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssessmentFixture {
    cases: Vec<AssessmentCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssessmentCase {
    name: String,
    expected: String,
    records: Vec<FixtureRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureRecord {
    layer: String,
    value: Value,
    comparison_id: String,
    observed_at: u64,
}

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("docs")
        .join("fixtures")
        .join("execution-settings")
        .join("phase-3-3-3c-assessments.json")
}

fn real_evidence_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("docs")
        .join("evidence")
        .join("phase-3-3-3c")
        .join("real-runtime.json")
}

fn layer(value: &str) -> ExecutionSettingsEvidenceLayer {
    match value {
        "requested" => ExecutionSettingsEvidenceLayer::Requested,
        "serverEffective" => ExecutionSettingsEvidenceLayer::ServerEffective,
        "persistedObserved" => ExecutionSettingsEvidenceLayer::PersistedObserved,
        other => panic!("unknown fixture layer: {other}"),
    }
}

fn setting_value(value: Value) -> ExecutionSettingValue {
    match value {
        Value::Null => ExecutionSettingValue::Null,
        Value::String(value) => ExecutionSettingValue::Text(value),
        Value::Bool(value) => ExecutionSettingValue::Bool(value),
        Value::Array(values) => ExecutionSettingValue::StringList(
            values
                .into_iter()
                .map(|value| value.as_str().expect("string list value").to_string())
                .collect(),
        ),
        other => panic!("unsupported fixture setting value: {other}"),
    }
}

fn assessment(value: &str) -> ExecutionSettingsAssessment {
    match value {
        "UNKNOWN" => ExecutionSettingsAssessment::Unknown,
        "REQUESTED_ONLY" => ExecutionSettingsAssessment::RequestedOnly,
        "EFFECTIVE_CONFIRMED" => ExecutionSettingsAssessment::EffectiveConfirmed,
        "OBSERVED_CONFIRMED" => ExecutionSettingsAssessment::ObservedConfirmed,
        "MATCH" => ExecutionSettingsAssessment::Match,
        "MISMATCH" => ExecutionSettingsAssessment::Mismatch,
        "CONFLICT" => ExecutionSettingsAssessment::Conflict,
        other => panic!("unknown expected assessment: {other}"),
    }
}

#[test]
fn phase_3_3_3c_fixture_covers_every_frozen_assessment() {
    let fixture: AssessmentFixture = serde_json::from_slice(
        &fs::read(fixture_path()).expect("Phase 3.3.3c assessment fixture must exist"),
    )
    .expect("valid assessment fixture");
    let key = ExecutionSettingsObservationKey::turn(
        CodexThreadKey::new("codex-home:fixture", "thread-fixture"),
        "turn-fixture",
    );

    let mut store = ExecutionSettingsEvidenceStore::default();
    let mut covered = Vec::new();
    for case in fixture.cases {
        let case_key = ExecutionSettingsObservationKey::turn(
            key.thread_key.clone(),
            format!("turn-fixture-{}", case.name),
        );
        for record in case.records {
            store.observe(
                case_key.clone(),
                ExecutionSettingField::Model,
                ExecutionSettingsEvidenceRecord::new(
                    layer(&record.layer),
                    setting_value(record.value),
                    ExecutionSettingsProvenance::confirmed(
                        format!("fixture:{}", case.name),
                        record.comparison_id,
                        record.observed_at,
                    ),
                ),
            );
        }
        let selected = store.select(&case_key, ExecutionSettingField::Model);
        assert_eq!(
            selected.assessment,
            assessment(&case.expected),
            "{}",
            case.name
        );
        covered.push(case.expected);
    }
    covered.sort();
    covered.dedup();
    assert_eq!(
        covered,
        vec![
            "CONFLICT",
            "EFFECTIVE_CONFIRMED",
            "MATCH",
            "MISMATCH",
            "OBSERVED_CONFIRMED",
            "REQUESTED_ONLY",
            "UNKNOWN",
        ]
    );
}

#[test]
fn phase_3_3_3c_real_evidence_preserves_scope_and_recovery_boundaries() {
    let evidence: Value = serde_json::from_slice(
        &fs::read(real_evidence_path()).expect("sanitized real evidence must exist"),
    )
    .expect("valid sanitized real evidence");
    let thread_id = evidence
        .pointer("/thread/fullThreadId")
        .and_then(Value::as_str)
        .expect("full Thread ID");

    assert_eq!(
        evidence.pointer("/threadSettingsUpdated/scope"),
        Some(&json!("THREAD_DEFAULT"))
    );
    assert_eq!(
        evidence.pointer("/threadSettingsUpdated/fullTurnIdPresent"),
        Some(&json!(false))
    );
    assert_eq!(
        evidence.pointer("/cliContinuation/sameFullThreadId"),
        Some(&json!(thread_id))
    );
    assert_ne!(
        evidence.pointer("/monitorTurns/2/fullTurnId"),
        evidence.pointer("/cliContinuation/newFullTurnId")
    );
    assert_eq!(
        evidence.pointer("/reconstruction/requested"),
        Some(&json!("NOT_RECOVERABLE_PROCESS_LOCAL"))
    );
    assert_eq!(
        evidence.pointer("/limitations/normalMonitorModelEffortOmitted"),
        Some(&json!(
            "NOT_TESTABLE_REQUEST_BUILDER_ALWAYS_EMITS_NULL_OR_CONCRETE"
        ))
    );
}

#[derive(Clone, Default)]
struct AcceptanceEventSink {
    events: Arc<StdMutex<Vec<Value>>>,
}

impl AcceptanceEventSink {
    async fn wait_for_turn_completion(&self, thread_id: &str, turn_id: &str) {
        tokio::time::timeout(Duration::from_secs(240), async {
            loop {
                let completed = self
                    .events
                    .lock()
                    .expect("acceptance event lock")
                    .iter()
                    .any(|event| {
                        event["method"] == "turn/completed"
                            && event["threadId"] == thread_id
                            && event["turnId"] == turn_id
                    });
                if completed {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("real acceptance Turn completion timeout");
    }

    fn thread_default_notification_count(&self, thread_id: &str) -> usize {
        self.events
            .lock()
            .expect("acceptance event lock")
            .iter()
            .filter(|event| {
                event["method"] == "thread/settings/updated"
                    && event["threadId"] == thread_id
                    && event.get("turnId").is_none_or(Value::is_null)
            })
            .count()
    }
}

impl EventSink for AcceptanceEventSink {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        let method = event.message.get("method").and_then(Value::as_str);
        let sanitized = match method {
            Some("turn/completed") => Some(json!({
                "method": "turn/completed",
                "threadId": event.message.pointer("/params/threadId"),
                "turnId": event.message.pointer("/params/turn/id"),
                "status": event.message.pointer("/params/turn/status"),
            })),
            Some("thread/settings/updated") => Some(json!({
                "method": "thread/settings/updated",
                "threadId": event.message.pointer("/params/threadId"),
                "turnId": event.message.pointer("/params/turnId"),
            })),
            _ => None,
        };
        if let Some(sanitized) = sanitized {
            self.events
                .lock()
                .expect("acceptance event lock")
                .push(sanitized);
        }
    }

    fn emit_terminal_output(&self, _event: TerminalOutput) {}
    fn emit_terminal_exit(&self, _event: TerminalExit) {}
}

fn intent(coordinator: &CreationCoordinator) -> IntentId {
    IntentId {
        process_epoch: coordinator.context()["processEpoch"]
            .as_str()
            .expect("coordinator process epoch")
            .to_string(),
        id: Uuid::new_v4().to_string(),
    }
}

fn turn_id(response: &Value) -> String {
    response
        .pointer("/result/turn/id")
        .and_then(Value::as_str)
        .expect("turn/start full Turn ID")
        .to_string()
}

fn evidence_summary(
    runtime: &super::execution_settings_ingestion::ExecutionSettingsEvidenceRuntime,
    key: &ExecutionSettingsObservationKey,
) -> Value {
    let fields = [
        ("model", ExecutionSettingField::Model),
        ("effort", ExecutionSettingField::Effort),
        ("approvalPolicy", ExecutionSettingField::ApprovalPolicy),
        ("sandboxPolicy", ExecutionSettingField::SandboxPolicy),
        ("networkAccess", ExecutionSettingField::NetworkAccess),
        ("writableRoots", ExecutionSettingField::WritableRoots),
        ("cwd", ExecutionSettingField::Cwd),
        (
            "collaborationMode",
            ExecutionSettingField::CollaborationMode,
        ),
    ];
    Value::Object(
        fields
            .into_iter()
            .map(|(name, field)| {
                let selected = runtime.select(key, field);
                (
                    name.to_string(),
                    json!({
                        "assessment": format!("{:?}", selected.assessment),
                        "requested": selected.requested.iter().map(|record| format!("{:?}", record.value)).collect::<Vec<_>>(),
                        "serverEffective": selected.server_effective.iter().map(|record| format!("{:?}", record.value)).collect::<Vec<_>>(),
                        "persistedObserved": selected.persisted_observed.iter().map(|record| format!("{:?}", record.value)).collect::<Vec<_>>(),
                        "provenance": selected.provenance.iter().map(|item| json!({
                            "source": item.source,
                            "comparisonId": item.comparison_id,
                            "confidence": format!("{:?}", item.confidence),
                        })).collect::<Vec<_>>(),
                    }),
                )
            })
            .collect(),
    )
}

fn watcher_for(home: CodexHomeSource, scratch: &Path) -> RolloutTailWatcher {
    RolloutTailWatcher::new(RolloutWatcherConfig {
        homes: vec![home],
        checkpoint_path: scratch.join("checkpoint.json"),
        deletion_tombstones_path: scratch.join("tombstones.json"),
        retry: WatcherRetryPolicy {
            max_attempts: 1,
            initial_backoff_ms: 0,
        },
        fresh_window_ms: 60_000,
        settled_after_ms: 60_000,
        reconciliation_interval_ms: 1_000,
    })
}

/// Explicitly opt in with `CODEX_3_3_3C_REAL=1` and a version-compatible
/// `CODEX_3_3_3C_CODEX_BIN`. This creates only a new dedicated acceptance
/// Thread and never reads or resumes an existing user Thread.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires authenticated real Codex runtime and creates a dedicated test Thread"]
async fn real_monitor_created_execution_settings_acceptance() {
    assert_eq!(std::env::var("CODEX_3_3_3C_REAL").as_deref(), Ok("1"));
    let codex_bin = std::env::var("CODEX_3_3_3C_CODEX_BIN")
        .expect("set CODEX_3_3_3C_CODEX_BIN to the version-compatible Codex binary");
    let model = std::env::var("CODEX_3_3_3C_MODEL")
        .expect("set CODEX_3_3_3C_MODEL to an available explicit model slug");
    let codex_home = crate::codex::home::resolve_default_codex_home().expect("default CODEX_HOME");
    let home = discover_runtime_codex_homes(Some(codex_home.clone()), [])
        .into_iter()
        .next()
        .expect("runtime Codex home");
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_string_lossy()
        .into_owned();
    let workspace_id = "phase-3-3-3c-real-acceptance".to_string();
    let workspace = WorkspaceEntry {
        id: workspace_id.clone(),
        name: "Phase 3.3.3c isolated acceptance".to_string(),
        path: workspace_path.clone(),
        kind: WorkspaceKind::Main,
        parent_id: None,
        worktree: None,
        settings: WorkspaceSettings::default(),
    };
    let sink = AcceptanceEventSink::default();
    let runtime = super::execution_settings_ingestion::ExecutionSettingsEvidenceRuntime::default();
    let session = spawn_workspace_session(
        workspace.clone(),
        Some(codex_bin),
        None,
        Some(codex_home.clone()),
        "phase-3-3-3c-acceptance".to_string(),
        sink.clone(),
        runtime.clone(),
    )
    .await
    .expect("spawn real Monitor app-server session");

    let sessions = Mutex::new(HashMap::from([(workspace_id.clone(), session.clone())]));
    let workspaces = Mutex::new(HashMap::from([(workspace_id.clone(), workspace)]));
    let coordinator = CreationCoordinator::default();
    let creation_intent = intent(&coordinator);
    let started = start_thread_core(
        &sessions,
        &workspaces,
        workspace_id.clone(),
        &coordinator,
        creation_intent.clone(),
    )
    .await
    .expect("Monitor thread/start acknowledgement");
    let thread_id = started
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .expect("full server Thread ID")
        .to_string();

    let first_turn_intent = TurnIntent {
        intent: intent(&coordinator),
        creation_intent: Some(creation_intent),
    };
    let full_access = send_user_message_core(
        &sessions,
        &workspaces,
        workspace_id.clone(),
        thread_id.clone(),
        "Phase 3.3.3c acceptance: reply only FULL_ACCESS_OK; do not use tools.".to_string(),
        Some(model.clone()),
        Some("low".to_string()),
        None,
        Some("full-access".to_string()),
        None,
        None,
        None,
        &coordinator,
        Some(first_turn_intent),
    )
    .await
    .expect("full-access turn/start");
    let full_access_turn = turn_id(&full_access);
    sink.wait_for_turn_completion(&thread_id, &full_access_turn)
        .await;

    let read_only = send_user_message_core(
        &sessions,
        &workspaces,
        workspace_id.clone(),
        thread_id.clone(),
        "Phase 3.3.3c acceptance: reply only READ_ONLY_OK; do not use tools.".to_string(),
        None,
        None,
        None,
        Some("read-only".to_string()),
        None,
        None,
        None,
        &coordinator,
        None,
    )
    .await
    .expect("read-only explicit-null turn/start");
    let read_only_turn = turn_id(&read_only);
    sink.wait_for_turn_completion(&thread_id, &read_only_turn)
        .await;

    let current = send_user_message_core(
        &sessions,
        &workspaces,
        workspace_id.clone(),
        thread_id.clone(),
        "Phase 3.3.3c acceptance: reply only CURRENT_OK; do not use tools.".to_string(),
        Some(model),
        Some("low".to_string()),
        None,
        None,
        None,
        None,
        None,
        &coordinator,
        None,
    )
    .await
    .expect("current/default turn/start");
    let current_turn = turn_id(&current);
    sink.wait_for_turn_completion(&thread_id, &current_turn)
        .await;

    let scratch = std::env::temp_dir().join(format!("codex-monitor-3-3-3c-{}", Uuid::new_v4()));
    fs::create_dir_all(&scratch).expect("acceptance scratch");
    let mut watcher = watcher_for(home.clone(), &scratch);
    let report = watcher
        .reconcile_now()
        .expect("real rollout reconciliation");
    let target_observations = report
        .execution_settings_turn_contexts
        .into_iter()
        .filter(|observation| observation.thread_key.thread_id == thread_id)
        .collect::<Vec<_>>();
    assert!(
        target_observations.len() >= 3,
        "expected persisted turn_context for each Monitor acceptance Turn"
    );
    runtime.observe_rollout_observations(target_observations.clone());

    let thread_key = CodexThreadKey::new(home.codex_home.identity.clone(), thread_id.clone());
    let full_access_key =
        ExecutionSettingsObservationKey::turn(thread_key.clone(), &full_access_turn);
    let read_only_key = ExecutionSettingsObservationKey::turn(thread_key.clone(), &read_only_turn);
    let current_key = ExecutionSettingsObservationKey::turn(thread_key.clone(), &current_turn);
    for (key, field) in [
        (&full_access_key, ExecutionSettingField::Model),
        (&full_access_key, ExecutionSettingField::Effort),
        (&full_access_key, ExecutionSettingField::ApprovalPolicy),
        (&full_access_key, ExecutionSettingField::SandboxPolicy),
        (&current_key, ExecutionSettingField::NetworkAccess),
    ] {
        assert_eq!(
            runtime.select(key, field).assessment,
            ExecutionSettingsAssessment::Match
        );
    }
    assert_eq!(
        runtime
            .select(&read_only_key, ExecutionSettingField::Model)
            .assessment,
        ExecutionSettingsAssessment::Mismatch
    );
    assert_eq!(
        runtime
            .select(&read_only_key, ExecutionSettingField::SandboxPolicy)
            .assessment,
        ExecutionSettingsAssessment::Match
    );
    assert_eq!(
        runtime
            .select(&current_key, ExecutionSettingField::WritableRoots)
            .assessment,
        ExecutionSettingsAssessment::RequestedOnly
    );
    assert!(sink.thread_default_notification_count(&thread_id) >= 1);
    assert_eq!(
        runtime
            .select(
                &ExecutionSettingsObservationKey::thread_default(thread_key.clone()),
                ExecutionSettingField::Model,
            )
            .assessment,
        ExecutionSettingsAssessment::EffectiveConfirmed
    );
    assert_eq!(
        started.pointer("/result/sandbox/writableRoots"),
        Some(&json!([]))
    );
    assert_eq!(
        started.pointer("/result/runtimeWorkspaceRoots"),
        Some(&json!([workspace_path]))
    );
    let turn_keys = [
        ("fullAccess", full_access_turn),
        ("readOnly", read_only_turn),
        ("current", current_turn),
    ];
    let summaries = turn_keys
        .iter()
        .map(|(name, turn)| {
            (
                name.to_string(),
                json!({
                    "turnId": turn,
                    "fields": evidence_summary(
                        &runtime,
                        &ExecutionSettingsObservationKey::turn(thread_key.clone(), turn),
                    ),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    let reconstructed =
        super::execution_settings_ingestion::ExecutionSettingsEvidenceRuntime::default();
    reconstructed.observe_rollout_observations(target_observations);
    for (_, turn) in &turn_keys {
        let key = ExecutionSettingsObservationKey::turn(thread_key.clone(), turn);
        assert!(reconstructed
            .history(&key, ExecutionSettingField::Model)
            .iter()
            .all(|record| record.layer == ExecutionSettingsEvidenceLayer::PersistedObserved));
    }

    println!(
        "PHASE_3_3_3C_REAL_EVIDENCE={}",
        serde_json::to_string_pretty(&json!({
            "threadId": thread_id,
            "codexHomeIdentity": home.codex_home.identity,
            "threadStartResponse": {
                "model": started.pointer("/result/model"),
                "reasoningEffort": started.pointer("/result/reasoningEffort"),
                "approvalPolicy": started.pointer("/result/approvalPolicy"),
                "cwd": started.pointer("/result/cwd"),
                "sandbox": started.pointer("/result/sandbox"),
                "runtimeWorkspaceRoots": started.pointer("/result/runtimeWorkspaceRoots"),
            },
            "threadDefaultEvidence": evidence_summary(
                &runtime,
                &ExecutionSettingsObservationKey::thread_default(thread_key.clone()),
            ),
            "turns": summaries,
            "threadDefaultSettingsNotificationsWithoutTurnId": sink.thread_default_notification_count(&thread_key.thread_id),
            "rolloutTurnContextCount": turn_keys.len(),
            "restartRequestedEvidence": "NOT_RECOVERABLE_PROCESS_LOCAL",
        }))
        .expect("serialize sanitized evidence")
    );

    let _ = session.child.lock().await.kill().await;
    let _ = fs::remove_dir_all(scratch);
}

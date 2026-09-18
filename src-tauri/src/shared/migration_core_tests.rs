use super::migration_core::{
    config_schema_version, inspect_migration, rollback_staging, stage_migration,
    stage_migration_with_failpoint, staging_path, validate_staging, MigrationFailPoint,
    MigrationState,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const FAKE_TOKEN: &str = "fake-token-DO-NOT-MIGRATE";
const FAKE_AUTH: &str = "fake-auth-secret";
const FAKE_IDENTITY: &str = "fake-host-identity";
const FAKE_THREAD: &str = "fake-thread-content";

struct TestRoots {
    base: PathBuf,
    source: PathBuf,
    target: PathBuf,
}

impl Drop for TestRoots {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn roots() -> TestRoots {
    let base = std::env::temp_dir().join(format!("codex-monitor-p4-1c-{}", Uuid::new_v4()));
    let source = base.join("legacy-source");
    let target = base.join("inactive-target");
    fs::create_dir_all(&source).unwrap();
    write_source(&source, false);
    TestRoots {
        base,
        source,
        target,
    }
}

fn write_source(source: &Path, unknown_required: bool) {
    let workspace_path = source.join("workspace");
    fs::create_dir_all(&workspace_path).unwrap();
    let mut settings: Value = serde_json::from_str(include_str!(
        "../../../docs/fixtures/phase-4-1c-whitelist-migration/legacy-settings.json"
    ))
    .unwrap();
    settings["requiredMigrationFields"] = if unknown_required {
        json!(["futureRequiredSetting"])
    } else {
        json!([])
    };
    let workspaces = json!([{
        "id": "workspace-1",
        "name": "Workspace One",
        "path": workspace_path.to_string_lossy(),
        "kind": "main",
        "settings": {
            "sidebarCollapsed": true,
            "sortOrder": 3,
            "futureWorkspaceSetting": "excluded-value"
        },
        "futureWorkspaceField": "excluded-value"
    }]);
    fs::write(
        source.join("settings.json"),
        serde_json::to_vec_pretty(&settings).unwrap(),
    )
    .unwrap();
    fs::write(
        source.join("workspaces.json"),
        serde_json::to_vec_pretty(&workspaces).unwrap(),
    )
    .unwrap();
    fs::write(source.join("auth.json"), FAKE_AUTH).unwrap();
    fs::write(source.join("credentials.json"), FAKE_AUTH).unwrap();
    fs::write(source.join("remote-host-identity.json"), FAKE_IDENTITY).unwrap();
    fs::write(source.join("daemon.pid"), "1234").unwrap();
    fs::write(source.join("remote-transport-generation.json"), "7").unwrap();
    let sessions = source.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(sessions.join("thread.jsonl"), FAKE_THREAD).unwrap();
    let codex_home = source.join("CODEX_HOME");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(codex_home.join("auth.json"), FAKE_AUTH).unwrap();
}

fn tree_bytes(root: &Path) -> Vec<u8> {
    fn visit(path: &Path, output: &mut Vec<u8>) {
        if !path.exists() {
            return;
        }
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(&entry, output);
            } else {
                output.extend(fs::read(&entry).unwrap());
            }
        }
    }
    let mut output = Vec::new();
    visit(root, &mut output);
    output
}

fn snapshot_source(source: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(root, &entry, output);
            } else {
                output.push((
                    entry.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry).unwrap(),
                ));
            }
        }
    }
    let mut output = Vec::new();
    visit(source, source, &mut output);
    output
}

fn staged_json(roots: &TestRoots, name: &str) -> Value {
    serde_json::from_slice(&fs::read(staging_path(&roots.target).join(name)).unwrap()).unwrap()
}

#[test]
fn migration_preview_is_read_only() {
    let roots = roots();
    let before = snapshot_source(&roots.source);
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(preview.state, MigrationState::Preflighted);
    assert_eq!(before, snapshot_source(&roots.source));
    assert!(!staging_path(&roots.target).exists());
    assert!(!roots.target.exists());
}

#[test]
fn legacy_source_is_never_modified() {
    let roots = roots();
    let before = snapshot_source(&roots.source);
    stage_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(before, snapshot_source(&roots.source));
}

#[test]
fn settings_are_rebuilt_from_field_allowlist() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let settings = staged_json(&roots, "settings.json");
    assert_eq!(settings["theme"], "dark");
    assert!(settings.get("futureUnknownField").is_none());
    assert!(settings["remoteBackends"][0]
        .get("futureNestedField")
        .is_none());
}

#[test]
fn remote_backend_token_is_not_migrated() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let settings = staged_json(&roots, "settings.json");
    assert!(settings["remoteBackendToken"].is_null());
    assert!(settings["remoteBackends"][0]["token"].is_null());
}

#[test]
fn credential_value_never_enters_staging() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let bytes = tree_bytes(&staging_path(&roots.target));
    assert!(!String::from_utf8_lossy(&bytes).contains(FAKE_TOKEN));
}

#[test]
fn credential_value_never_enters_backup() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let bytes = tree_bytes(&staging_path(&roots.target).join("backup"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains(FAKE_TOKEN));
    assert!(!text.contains(FAKE_AUTH));
}

#[test]
fn credential_value_never_enters_logs_or_report() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let report =
        fs::read_to_string(staging_path(&roots.target).join("migration-report.json")).unwrap();
    assert!(!report.contains(FAKE_TOKEN));
    assert!(!report.contains(FAKE_AUTH));
}

#[test]
fn auth_json_is_explicitly_excluded() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(preview
        .excluded_categories
        .contains(&"credentials".to_string()));
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(!tree_bytes(&staging_path(&roots.target))
        .windows(FAKE_AUTH.len())
        .any(|v| v == FAKE_AUTH.as_bytes()));
}

#[test]
fn credential_filenames_are_hard_excluded_without_copy() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(!preview
        .unknown_categories
        .contains(&"credentials.json".to_string()));
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target))).contains(FAKE_AUTH)
    );
}

#[test]
fn codex_home_is_explicitly_excluded() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(preview
        .excluded_categories
        .contains(&"codex_home".to_string()));
}

#[test]
fn canonical_thread_data_is_not_migrated() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target))).contains(FAKE_THREAD)
    );
}

#[test]
fn rollout_data_is_not_migrated() {
    let roots = roots();
    fs::write(roots.source.join("rollout-test.jsonl"), FAKE_THREAD).unwrap();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target))).contains(FAKE_THREAD)
    );
}

#[test]
fn unknown_fields_are_not_copied() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(preview
        .excluded_field_paths
        .iter()
        .any(|p| p == "settings.futureUnknownField"));
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target)))
            .contains("excluded-value")
    );
}

#[test]
fn unknown_required_field_blocks_preflight() {
    let roots = roots();
    write_source(&roots.source, true);
    let error = inspect_migration(&roots.source, &roots.target).unwrap_err();
    assert!(error.to_string().contains("PRECHECK_BLOCKED"));
}

#[test]
fn workspace_identity_is_preserved() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let workspaces = staged_json(&roots, "workspaces.json");
    assert_eq!(workspaces[0]["id"], "workspace-1");
    assert_eq!(workspaces[0]["name"], "Workspace One");
}

#[test]
fn duplicate_workspace_identity_fails_closed() {
    let roots = roots();
    let workspace_path = roots.source.join("workspace");
    let duplicate = json!([
        {"id":"same","name":"A","path":workspace_path,"kind":"main"},
        {"id":"same","name":"B","path":workspace_path,"kind":"main"}
    ]);
    fs::write(
        roots.source.join("workspaces.json"),
        serde_json::to_vec(&duplicate).unwrap(),
    )
    .unwrap();
    assert!(inspect_migration(&roots.source, &roots.target).is_err());
}

#[test]
fn existing_target_conflict_fails_closed() {
    let roots = roots();
    fs::create_dir_all(&roots.target).unwrap();
    fs::write(roots.target.join("unmanaged.txt"), "keep").unwrap();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(!preview.conflicts.is_empty());
    assert!(stage_migration(&roots.source, &roots.target).is_err());
    assert_eq!(
        fs::read_to_string(roots.target.join("unmanaged.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn remote_host_identity_is_deferred_not_copied() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(preview.legacy_remote_host_identity_present);
    stage_migration(&roots.source, &roots.target).unwrap();
    let output = String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target))).to_string();
    assert!(!output.contains(FAKE_IDENTITY));
    assert!(!staging_path(&roots.target)
        .join("remote-host-identity.json")
        .exists());
}

#[test]
fn runtime_generation_state_is_not_migrated() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target)))
            .contains("remote-transport-generation")
    );
}

#[test]
fn pending_mutation_state_is_not_migrated() {
    let roots = roots();
    fs::write(
        roots.source.join("pending-delete.json"),
        "{\"pending\":true}",
    )
    .unwrap();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(
        !String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target)))
            .contains("pending-delete")
    );
}

#[test]
fn config_schema_version_is_independent_from_build_number() {
    let version: Value = serde_json::from_str(include_str!("../../../VERSION.json")).unwrap();
    let identity: Value =
        serde_json::from_str(include_str!("../../../release-identity.json")).unwrap();
    assert_eq!(
        config_schema_version().unwrap(),
        identity["configSchemaVersion"].as_u64().unwrap() as u32
    );
    assert_ne!(
        config_schema_version().unwrap() as u64,
        version["build"].as_u64().unwrap() + 1
    );
}

#[test]
fn preflight_produces_sanitized_plan() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    let serialized = serde_json::to_string(&preview).unwrap();
    for forbidden in [FAKE_TOKEN, FAKE_AUTH, FAKE_IDENTITY, FAKE_THREAD] {
        assert!(!serialized.contains(forbidden));
    }
    assert!(preview.credential_fields_excluded > 0);
    assert!(preview.estimated_action_count >= 2);
}

#[test]
fn staging_does_not_touch_active_target() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(!roots.target.exists());
    assert!(staging_path(&roots.target).exists());
}

#[test]
fn validation_rejects_secret_material() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    fs::write(
        staging_path(&roots.target).join("secret.json"),
        json!({"authToken": FAKE_TOKEN}).to_string(),
    )
    .unwrap();
    assert!(validate_staging(&roots.target).is_err());
}

#[test]
fn validation_rejects_runtime_generation_state() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    fs::write(
        staging_path(&roots.target).join("runtime.json"),
        json!({"remoteTransportGeneration": 4}).to_string(),
    )
    .unwrap();
    assert!(validate_staging(&roots.target).is_err());
}

#[test]
fn validation_rejects_config_schema_mismatch() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let manifest_path = staging_path(&roots.target).join("migration-manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["configSchemaVersion"] = json!(999);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(validate_staging(&roots.target).is_err());
}

#[test]
fn validation_rejects_target_identity_mismatch() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let manifest_path = staging_path(&roots.target).join("migration-manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["targetDesktopIdentifier"] = json!("invalid.example");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(validate_staging(&roots.target).is_err());
}

#[test]
fn validation_failure_records_failed_state() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    fs::write(
        staging_path(&roots.target).join("secret.json"),
        json!({"authToken": FAKE_TOKEN}).to_string(),
    )
    .unwrap();
    assert!(validate_staging(&roots.target).is_err());
    let state: Value = staged_json(&roots, "migration-state.json");
    assert_eq!(state["state"], "failed");
}

#[test]
fn failed_migration_leaves_source_and_target_unchanged() {
    let roots = roots();
    let before = snapshot_source(&roots.source);
    fs::create_dir_all(&roots.target).unwrap();
    fs::write(roots.target.join("owned-by-user"), "unchanged").unwrap();
    assert!(stage_migration(&roots.source, &roots.target).is_err());
    assert_eq!(before, snapshot_source(&roots.source));
    assert_eq!(
        fs::read_to_string(roots.target.join("owned-by-user")).unwrap(),
        "unchanged"
    );
}

#[test]
fn rollback_only_removes_engine_owned_staging() {
    let roots = roots();
    let staging = staging_path(&roots.target);
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("not-owned"), "keep").unwrap();
    assert!(rollback_staging(&roots.target).is_err());
    assert!(staging.exists());
    fs::remove_dir_all(&staging).unwrap();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(rollback_staging(&roots.target).unwrap());
    assert!(!staging.exists());
}

#[test]
fn interruption_is_detected_on_next_run() {
    let roots = roots();
    let error =
        stage_migration_with_failpoint(&roots.source, &roots.target, MigrationFailPoint::MidStage)
            .unwrap_err();
    assert!(error.to_string().contains("INTERRUPTED"));
    assert!(staging_path(&roots.target).exists());
    let result = stage_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(result.state, MigrationState::ReadyForActivation);
}

#[test]
fn all_interruption_boundaries_never_activate() {
    for point in [
        MigrationFailPoint::AfterPreflight,
        MigrationFailPoint::MidStage,
        MigrationFailPoint::AfterStage,
        MigrationFailPoint::BeforeValidation,
        MigrationFailPoint::AfterValidation,
    ] {
        let roots = roots();
        assert!(stage_migration_with_failpoint(&roots.source, &roots.target, point).is_err());
        assert!(!roots.target.exists());
    }
}

#[test]
fn incomplete_staging_is_never_treated_as_active() {
    let roots = roots();
    let _ = stage_migration_with_failpoint(
        &roots.source,
        &roots.target,
        MigrationFailPoint::AfterStage,
    );
    assert!(!roots.target.exists());
    let state: Value = staged_json(&roots, "migration-state.json");
    assert_ne!(state["state"], "ready_for_activation");
}

#[test]
fn ready_for_activation_does_not_activate_runtime() {
    let roots = roots();
    let result = stage_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(result.state, MigrationState::ReadyForActivation);
    assert!(!roots.target.exists());
}

#[test]
fn empty_target_directory_remains_untouched() {
    let roots = roots();
    fs::create_dir_all(&roots.target).unwrap();
    stage_migration(&roots.source, &roots.target).unwrap();
    assert!(roots.target.is_dir());
    assert!(fs::read_dir(&roots.target).unwrap().next().is_none());
}

#[test]
fn ready_staging_is_deterministically_rebuilt() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let before = tree_bytes(&staging_path(&roots.target));
    stage_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(before, tree_bytes(&staging_path(&roots.target)));
}

#[test]
fn active_runtime_identifier_remains_legacy() {
    let config: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    assert_eq!(config["identifier"], "com.dimillian.codexmonitor");
}

#[test]
fn daemonctl_lookup_remains_legacy() {
    let daemonctl = include_str!("../bin/codex_monitor_daemonctl.rs");
    assert!(daemonctl.contains("com.dimillian.codexmonitor"));
    assert!(!daemonctl.contains("io.github.deanxie.codexmonitor"));
}

#[test]
fn migration_uses_only_explicit_source_and_target_roots() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert_eq!(preview.source_root, roots.source);
    assert_eq!(preview.target_root, roots.target);
}

#[test]
fn no_real_user_data_paths_are_accessed() {
    let roots = roots();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    let serialized = serde_json::to_string(&preview)
        .unwrap()
        .to_ascii_lowercase();
    assert!(!serialized.contains("com.dimillian.codexmonitor"));
    assert!(!serialized.contains("\\.codex\\"));
}

#[test]
fn unknown_root_artifact_is_reported_without_its_value() {
    let roots = roots();
    fs::write(roots.source.join("projection-state.json"), FAKE_THREAD).unwrap();
    let preview = inspect_migration(&roots.source, &roots.target).unwrap();
    assert!(preview
        .unknown_categories
        .contains(&"unrecognized_root_artifact".to_string()));
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains("projection-state.json"));
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains(FAKE_THREAD));
}

#[test]
fn backup_uses_same_sanitized_serializer() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let staging = staging_path(&roots.target);
    assert_eq!(
        fs::read(staging.join("settings.json")).unwrap(),
        fs::read(staging.join("backup/settings.json")).unwrap()
    );
    assert_eq!(
        fs::read(staging.join("workspaces.json")).unwrap(),
        fs::read(staging.join("backup/workspaces.json")).unwrap()
    );
}

#[test]
fn invalid_workspace_path_fails_closed() {
    let roots = roots();
    fs::write(
        roots.source.join("workspaces.json"),
        r#"[{"id":"w","name":"W","path":""}]"#,
    )
    .unwrap();
    assert!(inspect_migration(&roots.source, &roots.target).is_err());
}

#[test]
fn state_machine_has_no_activated_state() {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../docs/fixtures/phase-4-1c-whitelist-migration/contract.json"
    ))
    .unwrap();
    let states = contract["states"].as_array().unwrap();
    assert!(!states.iter().any(|value| value == "activated"));
}

#[test]
fn all_forbidden_sentinels_have_zero_output_occurrences() {
    let roots = roots();
    stage_migration(&roots.source, &roots.target).unwrap();
    let output = String::from_utf8_lossy(&tree_bytes(&staging_path(&roots.target))).to_string();
    for forbidden in [FAKE_TOKEN, FAKE_AUTH, FAKE_IDENTITY, FAKE_THREAD] {
        assert_eq!(
            output.matches(forbidden).count(),
            0,
            "forbidden sentinel leaked"
        );
    }
}

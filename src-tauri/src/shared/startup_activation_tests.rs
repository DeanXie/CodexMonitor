use super::startup_activation::{
    inspect_startup_roots, validate_activated_profile, validate_committed_profile,
    PersistedActivationState, StartupDisposition, LEGACY_DESKTOP_IDENTIFIER,
    TARGET_DESKTOP_IDENTIFIER,
};
use serde_json::json;
use std::fs;
use std::path::Path;

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "codex-monitor-p4-1d2-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_json(path: &Path, value: serde_json::Value) {
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn write_activated(root: &Path, transaction: &str) {
    write_profile(root, transaction, "runtime_validated");
}

fn write_committed(root: &Path, transaction: &str) {
    write_profile(root, transaction, "target_committed");
}

fn write_profile(root: &Path, transaction: &str, state: &str) {
    fs::create_dir_all(root).unwrap();
    write_json(&root.join("settings.json"), json!({}));
    write_json(&root.join("workspaces.json"), json!([]));
    write_json(
        &root.join("activation-manifest.json"),
        json!({
            "schemaVersion": 1,
            "transactionId": transaction,
            "profileState": "activated"
        }),
    );
    write_json(
        &root.join("remote-host-identity.json"),
        json!({
            "schemaVersion": 2,
            "state": "active",
            "remoteHostIdentity": "014383f2-41f8-4b13-b9d7-30c511e47cec",
            "transactionId": transaction
        }),
    );
    let canonical_root = fs::canonicalize(root).unwrap();
    let name = root.file_name().unwrap().to_string_lossy();
    write_json(
        &root
            .parent()
            .unwrap()
            .join(format!(".{name}.codexmonitor-activation-journal.json")),
        json!({
            "schemaVersion": 1,
            "transactionId": transaction,
            "sourceRoot": "",
            "targetRoot": canonical_root,
            "stagingRoot": root.parent().unwrap().join("prepared-no-longer-present"),
            "state": state
        }),
    );
}

#[test]
fn fresh_target_with_legacy_profile_requires_migration() {
    let base = temp_root("legacy");
    let target = base.join(TARGET_DESKTOP_IDENTIFIER);
    let legacy = base.join(LEGACY_DESKTOP_IDENTIFIER);
    fs::create_dir_all(&legacy).unwrap();
    write_json(&legacy.join("settings.json"), json!({}));
    write_json(&legacy.join("workspaces.json"), json!([]));

    let inspection = inspect_startup_roots(&target, &legacy).unwrap();
    assert_eq!(
        inspection.disposition,
        StartupDisposition::LegacyMigrationRequired
    );
    assert!(!target.exists());
    assert!(!target.join("remote-host-identity.json").exists());
}

#[test]
fn only_complete_bound_v2_profile_can_load_normally() {
    let base = temp_root("active");
    let target = base.join(TARGET_DESKTOP_IDENTIFIER);
    let legacy = base.join(LEGACY_DESKTOP_IDENTIFIER);
    write_activated(&target, "tx-active");

    let inspection = inspect_startup_roots(&target, &legacy).unwrap();
    assert_eq!(
        inspection.disposition,
        StartupDisposition::RuntimeValidationRequired
    );
    assert!(!inspection.normal_load_allowed);
    validate_activated_profile(&target).unwrap();
}

#[test]
fn target_committed_profile_enters_restricted_runtime_validation() {
    let base = temp_root("committed");
    let target = base.join(TARGET_DESKTOP_IDENTIFIER);
    let legacy = base.join(LEGACY_DESKTOP_IDENTIFIER);
    write_committed(&target, "tx-committed");

    let inspection = inspect_startup_roots(&target, &legacy).unwrap();
    assert_eq!(
        inspection.disposition,
        StartupDisposition::RuntimeValidationRequired
    );
    assert!(!inspection.normal_load_allowed);
    let metadata = validate_committed_profile(&target).unwrap();
    assert_eq!(
        metadata.persisted_state,
        PersistedActivationState::TargetCommitted
    );
    assert!(validate_activated_profile(&target).is_err());
}

#[test]
fn old_runtime_validated_history_still_requires_current_process_validation() {
    let base = temp_root("historical-runtime");
    let target = base.join(TARGET_DESKTOP_IDENTIFIER);
    let legacy = base.join(LEGACY_DESKTOP_IDENTIFIER);
    write_activated(&target, "tx-historical-runtime");

    let inspection = inspect_startup_roots(&target, &legacy).unwrap();
    assert_eq!(
        inspection.disposition,
        StartupDisposition::RuntimeValidationRequired
    );
    assert!(!inspection.normal_load_allowed);
    assert_eq!(
        validate_committed_profile(&target).unwrap().persisted_state,
        PersistedActivationState::RuntimeValidated
    );
}

#[test]
fn corrupt_or_future_activation_never_falls_back_to_fresh() {
    let base = temp_root("future");
    let target = base.join(TARGET_DESKTOP_IDENTIFIER);
    let legacy = base.join(LEGACY_DESKTOP_IDENTIFIER);
    write_activated(&target, "tx-future");
    let manifest = target.join("activation-manifest.json");
    write_json(
        &manifest,
        json!({"schemaVersion": 99, "transactionId": "tx-future", "profileState": "activated"}),
    );

    let inspection = inspect_startup_roots(&target, &legacy).unwrap();
    assert_eq!(inspection.disposition, StartupDisposition::BlockedCorrupt);
    assert!(validate_activated_profile(&target).is_err());
}

#[test]
fn explicit_data_root_does_not_bypass_activation() {
    let base = temp_root("explicit");
    let target = base.join("arbitrary-explicit-data-dir");
    fs::create_dir_all(&target).unwrap();
    write_json(&target.join("settings.json"), json!({}));
    write_json(&target.join("workspaces.json"), json!([]));

    assert!(validate_activated_profile(&target).is_err());
}

#[test]
fn identity_manifest_transaction_mismatch_is_blocked() {
    let root = temp_root("binding");
    write_activated(&root, "manifest-transaction");
    let identity = root.join("remote-host-identity.json");
    write_json(
        &identity,
        json!({
            "schemaVersion": 2,
            "state": "active",
            "remoteHostIdentity": "014383f2-41f8-4b13-b9d7-30c511e47cec",
            "transactionId": "different-transaction"
        }),
    );
    assert!(validate_activated_profile(&root).is_err());
}

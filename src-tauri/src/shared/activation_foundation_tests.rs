use super::activation_foundation::{
    activation_staging_path, commit_fresh_activation, commit_migration_activation,
    commit_migration_activation_with_stop_guard, complete_runtime_validation,
    inspect_bootstrap_profile, prepare_fresh_activation, prepare_migration_activation,
    recover_activation, write_activation_journal, ActivationFailPoint, ActivationJournal,
    ActivationJournalState, BootstrapProfileClassification, LoadPermission, RecoveryDisposition,
    RuntimeProcessGate, RuntimeProcessState, ServiceLifetimeLock,
};
use super::legacy_migration_entry::{LegacyProcessStopGuard, StopEvidenceState};
use super::legacy_remote_host_identity_loader_fixture::load_v1;
use super::remote_host_identity_activation::{
    load_v2_identity, write_v2_identity, HostIdentityV2State, ProtectedLegacyIdentity,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn write_migration_source(source: &Path) {
    fs::create_dir_all(source.join("workspace")).unwrap();
    write_json(
        &source.join("settings.json"),
        &serde_json::to_value(crate::types::AppSettings::default()).unwrap(),
    );
    write_json(
        &source.join("workspaces.json"),
        &json!([{
            "id":"workspace-1",
            "name":"Workspace",
            "path":source.join("workspace"),
            "kind":"main"
        }]),
    );
    write_json(
        &source.join("remote-host-identity.json"),
        &json!({
            "schemaVersion":1,
            "remoteHostIdentity":"6ba7b810-9dad-41d1-80b4-00c04fd430c8"
        }),
    );
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("codex-monitor-p4-1d-{label}-{}", Uuid::new_v4()))
}

fn write_json(path: &Path, value: &serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

struct TestVerifiedStopGuard;

impl LegacyProcessStopGuard for TestVerifiedStopGuard {
    fn revalidate(&mut self, _source_root: &Path) -> Result<StopEvidenceState, String> {
        Ok(StopEvidenceState::VerifiedQuiescentWithinSupportedScope)
    }
}

fn commit_migration_with_verified_stop(
    target: &Path,
    failpoint: Option<ActivationFailPoint>,
) -> Result<(), String> {
    let mut guard = TestVerifiedStopGuard;
    commit_migration_activation_with_stop_guard(target, failpoint, &mut guard)
}

#[test]
fn bootstrap_inspection_is_read_only_and_fresh_requires_explicit_activation() {
    let root = temp_dir("bootstrap-fresh");
    let inspection = inspect_bootstrap_profile(&root).unwrap();
    assert_eq!(
        inspection.classification,
        BootstrapProfileClassification::FreshProfile
    );
    assert_eq!(inspection.load_permission, LoadPermission::ActivationOnly);
    assert!(!root.exists());
}

#[test]
fn corrupt_profile_never_falls_back_to_defaults() {
    let root = temp_dir("bootstrap-corrupt");
    write_json(&root.join("settings.json"), &json!({"theme": "dark"}));
    fs::write(root.join("workspaces.json"), b"not-json").unwrap();
    let inspection = inspect_bootstrap_profile(&root).unwrap();
    assert_eq!(
        inspection.classification,
        BootstrapProfileClassification::CorruptProfile
    );
    assert_eq!(inspection.load_permission, LoadPermission::Denied);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v2_active_and_retired_are_distinct_and_only_active_loads() {
    let root = temp_dir("identity-v2");
    let path = root.join("remote-host-identity.json");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    write_v2_identity(&path, id, HostIdentityV2State::Active, None).unwrap();
    assert_eq!(load_v2_identity(&path).unwrap().remote_host_identity, id);
    write_v2_identity(
        &path,
        id,
        HostIdentityV2State::Retired,
        Some("activation-test"),
    )
    .unwrap();
    assert!(load_v2_identity(&path).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn frozen_v1_loader_accepts_v1_but_rejects_v2_without_rewrite() {
    let root = temp_dir("legacy-loader");
    let path = root.join("remote-host-identity.json");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    write_json(&path, &json!({"schemaVersion":1,"remoteHostIdentity":id}));
    assert_eq!(load_v1(&path).unwrap(), id);
    write_v2_identity(&path, id, HostIdentityV2State::Active, None).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(load_v1(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    write_v2_identity(&path, id, HostIdentityV2State::Retired, Some("tx")).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(load_v1(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::write(&path, b"broken").unwrap();
    assert!(load_v1(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"broken");
    let _ = fs::remove_dir_all(root);
}

#[cfg(desktop)]
#[test]
fn baseline_production_loader_rejects_v2_without_generating_or_rewriting() {
    let root = temp_dir("production-v1-loader");
    let path = root.join("remote-host-identity.json");
    write_v2_identity(
        &path,
        "6ba7b810-9dad-41d1-80b4-00c04fd430c8",
        HostIdentityV2State::Retired,
        Some("compatibility-test"),
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(super::remote_host_identity::load_or_initialize_remote_host_identity(&root).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fresh_profile_is_complete_before_activation_commit() {
    let root = temp_dir("fresh-order");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    prepare_fresh_activation(&target, id).unwrap();
    assert!(!target.exists());
    let staging = activation_staging_path(&target);
    for name in [
        "settings.json",
        "workspaces.json",
        "remote-host-identity.json",
        "activation-manifest.json",
    ] {
        assert!(staging.join(name).is_file());
    }
    commit_fresh_activation(&target).unwrap();
    let inspection = inspect_bootstrap_profile(&target).unwrap();
    assert_eq!(
        inspection.classification,
        BootstrapProfileClassification::CommittedValid
    );
    assert_eq!(inspection.load_permission, LoadPermission::ActivationOnly);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_commit_does_not_claim_business_runtime_validation() {
    let root = temp_dir("runtime-validation-boundary");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    prepare_fresh_activation(&target, "6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();

    commit_fresh_activation(&target).unwrap();

    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(super::activation_foundation::activation_journal_path(
            &target,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["state"], "target_committed");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_process_gate_requires_internal_validation_completion() {
    let gate = RuntimeProcessGate::new();
    assert_eq!(gate.state().unwrap(), RuntimeProcessState::Blocked);
    gate.begin_validation().unwrap();
    assert_eq!(gate.state().unwrap(), RuntimeProcessState::Validating);
    assert!(!gate.business_access_allowed().unwrap());
    assert!(gate.begin_validation().is_err());
    gate.complete_validation().unwrap();
    assert_eq!(gate.state().unwrap(), RuntimeProcessState::Ready);
    assert!(gate.business_access_allowed().unwrap());
}

#[test]
fn runtime_validation_failure_keeps_business_access_closed() {
    let gate = RuntimeProcessGate::new();
    gate.begin_validation().unwrap();
    gate.fail_validation().unwrap();
    assert_eq!(gate.state().unwrap(), RuntimeProcessState::Failed);
    assert!(!gate.business_access_allowed().unwrap());
    assert!(gate.complete_validation().is_err());
}

#[test]
fn runtime_completion_rechecks_transaction_binding_before_advancing_journal() {
    let root = temp_dir("runtime-binding-recheck");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    prepare_fresh_activation(&target, "6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();
    commit_fresh_activation(&target).unwrap();

    assert!(complete_runtime_validation(&target, "different-transaction", None).is_err());
    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(super::activation_foundation::activation_journal_path(
            &target,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["state"], "target_committed");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_journal_failure_does_not_publish_persistent_success() {
    let root = temp_dir("runtime-journal-failure");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    let prepared =
        prepare_fresh_activation(&target, "6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();
    commit_fresh_activation(&target).unwrap();

    assert!(complete_runtime_validation(
        &target,
        &prepared.transaction_id,
        Some(ActivationFailPoint::BeforeRuntimeValidatedJournal)
    )
    .is_err());
    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(super::activation_foundation::activation_journal_path(
            &target,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["state"], "target_committed");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn concurrent_runtime_validators_share_only_the_short_commit_section() {
    let root = temp_dir("concurrent-runtime-validation");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    let prepared =
        prepare_fresh_activation(&target, "6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();
    commit_fresh_activation(&target).unwrap();

    let first_target = target.clone();
    let first_transaction = prepared.transaction_id.clone();
    let first = std::thread::spawn(move || {
        complete_runtime_validation(&first_target, &first_transaction, None)
    });
    let second_target = target.clone();
    let second_transaction = prepared.transaction_id.clone();
    let second = std::thread::spawn(move || {
        complete_runtime_validation(&second_target, &second_transaction, None)
    });

    let outcomes = [first.join().unwrap(), second.join().unwrap()];
    assert!(outcomes.iter().any(Result::is_ok));
    for error in outcomes.iter().filter_map(|outcome| outcome.as_ref().err()) {
        assert!(error.contains("activation commit already in progress"));
    }
    complete_runtime_validation(&target, &prepared.transaction_id, None).unwrap();
    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(super::activation_foundation::activation_journal_path(
            &target,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["state"], "runtime_validated");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_change_after_prepare_blocks_fresh_commit() {
    let root = temp_dir("fresh-conflict");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    prepare_fresh_activation(&target, "6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("other-instance"), b"keep").unwrap();
    assert!(commit_fresh_activation(&target).is_err());
    assert_eq!(fs::read(target.join("other-instance")).unwrap(), b"keep");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn migration_commit_requires_backend_stop_guard_for_first_retirement() {
    let root = temp_dir("legacy-stop-gate");
    let source = root.join("source");
    let target = root.join("target");
    write_migration_source(&source);
    let identity_before = fs::read(source.join("remote-host-identity.json")).unwrap();
    super::migration_core::stage_migration(&source, &target).unwrap();
    prepare_migration_activation(&source, &target).unwrap();
    let error = commit_migration_activation(&target, None).unwrap_err();
    assert!(error.contains("controlled legacy process stop evidence is required"));
    assert_eq!(
        fs::read(source.join("remote-host-identity.json")).unwrap(),
        identity_before
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fresh_commit_rejects_manifest_or_identity_from_another_transaction() {
    let root = temp_dir("transaction-binding");
    fs::create_dir_all(&root).unwrap();
    let target = root.join("target");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    prepare_fresh_activation(&target, id).unwrap();
    let staging = activation_staging_path(&target);
    write_v2_identity(
        &staging.join("remote-host-identity.json"),
        id,
        HostIdentityV2State::Active,
        Some("different-transaction"),
    )
    .unwrap();
    assert!(commit_fresh_activation(&target).is_err());
    assert!(!target.exists());
    fs::rename(&staging, &target).unwrap();
    let inspection = inspect_bootstrap_profile(&target).unwrap();
    assert_eq!(
        inspection.classification,
        BootstrapProfileClassification::CorruptProfile
    );
    assert_eq!(inspection.load_permission, LoadPermission::Denied);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recovery_rejects_retired_source_when_prepared_recovery_is_invalid() {
    let root = temp_dir("journal-lag");
    let source = root.join("source");
    let target = root.join("target");
    let staging = root.join("staging");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&staging).unwrap();
    write_json(
        &source.join("remote-host-identity.json"),
        &json!({
            "schemaVersion":2,
            "state":"retired",
            "remoteHostIdentity":"6ba7b810-9dad-41d1-80b4-00c04fd430c8",
            "transactionId":"tx"
        }),
    );
    let journal = ActivationJournal {
        schema_version: 1,
        transaction_id: "tx".to_string(),
        source_root: fs::canonicalize(&source).unwrap(),
        target_root: fs::canonicalize(&root).unwrap().join("target"),
        staging_root: fs::canonicalize(&staging).unwrap(),
        state: ActivationJournalState::Prepared,
    };
    write_activation_journal(&target, &journal).unwrap();
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::BlockedInconsistent
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn service_lifetime_lock_excludes_second_new_service() {
    let root = temp_dir("service-lock");
    let first = ServiceLifetimeLock::acquire(&root).unwrap();
    assert!(ServiceLifetimeLock::acquire(&root).is_err());
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "try {$f=[IO.File]::OpenRead($args[0]); $f.Dispose(); exit 0} catch {exit 24}",
        ])
        .arg(&first.path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(24));
    drop(first);
    assert!(ServiceLifetimeLock::acquire(&root).is_ok());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_protected_replace_has_no_missing_path_and_v1_loader_stays_blocked() {
    let root = temp_dir("replace");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("remote-host-identity.json");
    let replacement = root.join("retired.json");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    write_json(&path, &json!({"schemaVersion":1,"remoteHostIdentity":id}));
    write_v2_identity(
        &replacement,
        id,
        HostIdentityV2State::Retired,
        Some("replace-test"),
    )
    .unwrap();
    let guard = ProtectedLegacyIdentity::acquire(&path).unwrap();
    assert!(path.exists());
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "try {[IO.File]::ReadAllText($args[0]) | Out-Null; exit 0} catch {exit 23}",
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(23));
    guard.replace_with(&replacement).unwrap();
    assert!(path.exists());
    assert!(load_v1(&path).is_err());
    drop(guard);
    assert!(path.exists());
    assert!(load_v1(&path).is_err());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_replace_failure_preserves_original_path() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

    let root = temp_dir("replace-blocked");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("remote-host-identity.json");
    let replacement = root.join("retired.json");
    let id = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    write_json(&path, &json!({"schemaVersion":1,"remoteHostIdentity":id}));
    write_v2_identity(&replacement, id, HostIdentityV2State::Retired, Some("tx")).unwrap();
    let guard = ProtectedLegacyIdentity::acquire(&path).unwrap();
    let blocking = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&replacement)
        .unwrap();
    assert!(guard.replace_with(&replacement).is_err());
    assert!(path.exists());
    drop(guard);
    drop(blocking);
    assert_eq!(load_v1(&path).unwrap(), id);
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn journal_lag_after_real_retirement_preserves_recovery_and_resumes_without_new_uuid() {
    let root = temp_dir("retirement-recovery");
    let source = root.join("source");
    let target = root.join("target");
    write_migration_source(&source);
    super::migration_core::stage_migration(&source, &target).unwrap();
    prepare_migration_activation(&source, &target).unwrap();
    let error = commit_migration_with_verified_stop(
        &target,
        Some(ActivationFailPoint::AfterIdentityReplaceBeforeJournal),
    )
    .unwrap_err();
    assert!(error.contains("simulated interruption"));
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::ContinueAfterRetirement
    );
    assert!(super::migration_core::rollback_staging(&target).is_err());
    commit_migration_activation(&target, None).unwrap();
    assert_eq!(
        load_v2_identity(&target.join("remote-host-identity.json"))
            .unwrap()
            .remote_host_identity,
        "6ba7b810-9dad-41d1-80b4-00c04fd430c8"
    );
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::ValidateCommittedTarget
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn target_move_before_journal_advance_is_recoverable() {
    let root = temp_dir("target-journal-lag");
    let source = root.join("source");
    let target = root.join("target");
    write_migration_source(&source);
    super::migration_core::stage_migration(&source, &target).unwrap();
    prepare_migration_activation(&source, &target).unwrap();
    assert!(commit_migration_with_verified_stop(
        &target,
        Some(ActivationFailPoint::AfterTargetMoveBeforeJournal)
    )
    .is_err());
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::ValidateCommittedTarget
    );
    commit_migration_activation(&target, None).unwrap();
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::ValidateCommittedTarget
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn concurrent_migration_commit_has_one_successful_committer() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let root = temp_dir("concurrent-commit");
    let source = root.join("source");
    let target = root.join("target");
    write_migration_source(&source);
    super::migration_core::stage_migration(&source, &target).unwrap();
    prepare_migration_activation(&source, &target).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let target = target.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                commit_migration_with_verified_stop(&target, None)
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        recover_activation(&target).unwrap(),
        RecoveryDisposition::ValidateCommittedTarget
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn every_activation_journal_boundary_has_a_deterministic_recovery_disposition() {
    for (point, expected) in [
        (
            ActivationFailPoint::AfterPreparedJournal,
            RecoveryDisposition::ContinueBeforeRetirement,
        ),
        (
            ActivationFailPoint::AfterIdentityJournal,
            RecoveryDisposition::ContinueAfterRetirement,
        ),
        (
            ActivationFailPoint::AfterTargetJournal,
            RecoveryDisposition::ValidateCommittedTarget,
        ),
    ] {
        let root = temp_dir("journal-boundary");
        let source = root.join("source");
        let target = root.join("target");
        write_migration_source(&source);
        super::migration_core::stage_migration(&source, &target).unwrap();
        prepare_migration_activation(&source, &target).unwrap();
        assert!(commit_migration_with_verified_stop(&target, Some(point)).is_err());
        assert_eq!(recover_activation(&target).unwrap(), expected);
        let _ = fs::remove_dir_all(root);
    }
}

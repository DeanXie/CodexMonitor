use super::legacy_migration_entry::{
    LegacyMigrationCoordinator, LegacyProcessStopGuard, LegacyProcessStopProvider,
    StopEvidenceAcquisition, StopEvidenceState,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("codex-monitor-p4-1d4b-{label}-{}", Uuid::new_v4()))
}

fn write_json(path: &Path, value: serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn write_legacy_profile(root: &Path) {
    write_json(
        &root.join("settings.json"),
        serde_json::to_value(crate::types::AppSettings::default()).unwrap(),
    );
    write_json(&root.join("workspaces.json"), json!([]));
    write_json(
        &root.join("remote-host-identity.json"),
        json!({"schemaVersion": 1, "remoteHostIdentity": Uuid::new_v4().to_string()}),
    );
}

struct NeverCalledProvider;

impl LegacyProcessStopProvider for NeverCalledProvider {
    fn acquire(&self, _source_root: &Path) -> Result<StopEvidenceAcquisition, String> {
        panic!("read-only preview must not inspect or stop processes")
    }
}

struct RunningProvider;

impl LegacyProcessStopProvider for RunningProvider {
    fn acquire(&self, _source_root: &Path) -> Result<StopEvidenceAcquisition, String> {
        Ok(StopEvidenceAcquisition::Blocked(StopEvidenceState::Running))
    }
}

struct NeverValidGuard;

struct InitiallyVerifiedThenRunningProvider;

impl LegacyProcessStopGuard for NeverValidGuard {
    fn revalidate(&mut self, _source_root: &Path) -> Result<StopEvidenceState, String> {
        Ok(StopEvidenceState::Unknown)
    }
}

impl LegacyProcessStopProvider for InitiallyVerifiedThenRunningProvider {
    fn acquire(&self, _source_root: &Path) -> Result<StopEvidenceAcquisition, String> {
        Ok(StopEvidenceAcquisition::Verified(Box::new(NeverValidGuard)))
    }
}

struct VerifiedProvider {
    revalidations: Arc<AtomicUsize>,
}

struct VerifiedGuard {
    revalidations: Arc<AtomicUsize>,
}

impl LegacyProcessStopProvider for VerifiedProvider {
    fn acquire(&self, _source_root: &Path) -> Result<StopEvidenceAcquisition, String> {
        Ok(StopEvidenceAcquisition::Verified(Box::new(VerifiedGuard {
            revalidations: self.revalidations.clone(),
        })))
    }
}

impl LegacyProcessStopGuard for VerifiedGuard {
    fn revalidate(&mut self, _source_root: &Path) -> Result<StopEvidenceState, String> {
        self.revalidations.fetch_add(1, Ordering::SeqCst);
        Ok(StopEvidenceState::VerifiedQuiescentWithinSupportedScope)
    }
}

#[test]
fn preview_is_read_only_and_returns_only_sanitized_categories() {
    let base = temp_root("preview");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let identity_before = fs::read(source.join("remote-host-identity.json")).unwrap();

    let coordinator = LegacyMigrationCoordinator::new(
        source.clone(),
        target.clone(),
        Arc::new(NeverCalledProvider),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();

    assert!(!preview.preview_id.is_empty());
    assert_eq!(preview.source_schema_version, 0);
    assert!(preview
        .migratable_categories
        .contains(&"settings".to_string()));
    assert!(!target.exists());
    assert!(!super::migration_core::staging_path(&target).exists());
    assert_eq!(
        fs::read(source.join("remote-host-identity.json")).unwrap(),
        identity_before
    );
    let serialized = serde_json::to_string(&preview).unwrap();
    assert!(!serialized.contains(source.to_string_lossy().as_ref()));
    assert!(!serialized.contains("remoteHostIdentity"));
}

#[test]
fn user_confirmation_cannot_substitute_for_native_stop_evidence() {
    let base = temp_root("running");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let identity_before = fs::read(source.join("remote-host-identity.json")).unwrap();
    let coordinator = LegacyMigrationCoordinator::new(
        source.clone(),
        target.clone(),
        Arc::new(RunningProvider),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();

    let error = coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap_err();

    assert!(error.contains("RUNNING"));
    assert_eq!(
        fs::read(source.join("remote-host-identity.json")).unwrap(),
        identity_before
    );
    assert!(!target.exists());
}

#[test]
fn unknown_guard_is_not_treated_as_verified_quiescence() {
    let mut guard = NeverValidGuard;
    assert_eq!(
        guard.revalidate(Path::new("C:/fake-root")).unwrap(),
        StopEvidenceState::Unknown
    );
}

#[test]
fn verified_guard_is_revalidated_during_protected_retirement_and_stops_at_target_committed() {
    let base = temp_root("commit");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let revalidations = Arc::new(AtomicUsize::new(0));
    let coordinator = LegacyMigrationCoordinator::new(
        source.clone(),
        target.clone(),
        Arc::new(VerifiedProvider {
            revalidations: revalidations.clone(),
        }),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();

    coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap();

    assert_eq!(revalidations.load(Ordering::SeqCst), 1);
    let metadata = super::startup_activation::validate_committed_profile(&target).unwrap();
    assert_eq!(
        metadata.persisted_state,
        super::startup_activation::PersistedActivationState::TargetCommitted
    );
    let source_identity: serde_json::Value =
        serde_json::from_slice(&fs::read(source.join("remote-host-identity.json")).unwrap())
            .unwrap();
    assert_eq!(source_identity["schemaVersion"], 2);
    assert_eq!(source_identity["state"], "retired");
}

#[test]
fn changed_source_invalidates_preview_before_stop_evidence_or_staging() {
    let base = temp_root("changed");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let coordinator = LegacyMigrationCoordinator::new(
        source.clone(),
        target.clone(),
        Arc::new(NeverCalledProvider),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();
    fs::write(source.join("settings.json"), b"{}\n").unwrap();

    let error = coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap_err();

    assert!(error.contains("source changed"));
    assert!(!target.exists());
    assert!(!super::migration_core::staging_path(&target).exists());
}

#[test]
fn stopped_then_running_inside_retirement_protection_fails_without_legacy_write() {
    let base = temp_root("revalidation-running");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let identity_before = fs::read(source.join("remote-host-identity.json")).unwrap();
    let coordinator = LegacyMigrationCoordinator::new(
        source.clone(),
        target.clone(),
        Arc::new(InitiallyVerifiedThenRunningProvider),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();

    let error = coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap_err();

    assert!(error.contains("stop evidence changed inside retirement protection"));
    assert_eq!(
        fs::read(source.join("remote-host-identity.json")).unwrap(),
        identity_before
    );
    assert!(!target.exists());
}

#[test]
fn one_time_preview_cannot_be_reused_after_a_blocked_confirmation() {
    let base = temp_root("one-time");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    let coordinator = LegacyMigrationCoordinator::new(
        source,
        target,
        Arc::new(RunningProvider),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();
    assert!(coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap_err()
        .contains("RUNNING"));
    assert!(coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap_err()
        .contains("absent or was already consumed"));
}

#[test]
fn prepared_interruption_can_resume_with_fresh_stop_evidence_without_restaging() {
    let base = temp_root("prepared-recovery");
    let source = base.join("legacy");
    let target = base.join("target");
    write_legacy_profile(&source);
    super::migration_core::stage_migration(&source, &target).unwrap();
    let prepared =
        super::activation_foundation::prepare_migration_activation(&source, &target).unwrap();
    let revalidations = Arc::new(AtomicUsize::new(0));
    let coordinator = LegacyMigrationCoordinator::new(
        source,
        target.clone(),
        Arc::new(VerifiedProvider {
            revalidations: revalidations.clone(),
        }),
        Duration::from_secs(60),
    );
    let preview = coordinator.preview().unwrap();

    coordinator
        .confirm(&preview.preview_id, "confirm_legacy_migration")
        .unwrap();

    assert_eq!(revalidations.load(Ordering::SeqCst), 1);
    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(super::activation_foundation::activation_journal_path(
            &target,
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(journal["transactionId"], prepared.transaction_id);
    assert_eq!(journal["state"], "target_committed");
}

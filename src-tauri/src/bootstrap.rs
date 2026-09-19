use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

use crate::shared::activation_foundation::{
    activation_recovery_is_migration, commit_fresh_activation, commit_migration_activation,
    complete_runtime_validation, converge_recovered_committed_target, prepare_fresh_activation,
    recover_activation, RecoveryDisposition, RuntimeProcessGate, RuntimeProcessState,
};
use crate::shared::legacy_migration_entry::{
    LegacyMigrationCoordinator, LegacyMigrationPreview, LegacyProcessStopProvider,
};
use crate::shared::legacy_process_stop::WindowsLegacyProcessStopProvider;
use crate::shared::startup_activation::{
    inspect_startup_roots, legacy_root_for_target, StartupDisposition, StartupInspection,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapStatus {
    pub(crate) inspection: StartupInspection,
    pub(crate) restart_required: bool,
    pub(crate) runtime_state: RuntimeProcessState,
}

pub(crate) struct BootstrapState {
    target_root: PathBuf,
    legacy_root: PathBuf,
    status: Mutex<Result<BootstrapStatus, String>>,
    runtime_gate: RuntimeProcessGate,
    migration: Result<LegacyMigrationCoordinator, String>,
}

impl BootstrapState {
    pub(crate) fn inspect(target_root: PathBuf) -> Self {
        let provider = std::env::current_exe()
            .map_err(|error| {
                format!("resolve current executable for legacy stop evidence: {error}")
            })
            .and_then(|executable| {
                WindowsLegacyProcessStopProvider::new(executable, std::process::id())
                    .map(|provider| Arc::new(provider) as Arc<dyn LegacyProcessStopProvider>)
            });
        Self::inspect_with_provider(target_root, provider)
    }

    fn inspect_with_provider(
        target_root: PathBuf,
        provider: Result<Arc<dyn LegacyProcessStopProvider>, String>,
    ) -> Self {
        let (legacy_root, status) = match legacy_root_for_target(&target_root) {
            Ok(legacy_root) => {
                let status = inspect_startup_roots(&target_root, &legacy_root, |root| {
                    recover_activation(root).map(Into::into)
                })
                .map(|inspection| BootstrapStatus {
                    inspection,
                    restart_required: false,
                    runtime_state: RuntimeProcessState::Blocked,
                });
                (legacy_root, status)
            }
            Err(error) => (PathBuf::new(), Err(error)),
        };
        let migration = provider.map(|provider| {
            LegacyMigrationCoordinator::new(
                legacy_root.clone(),
                target_root.clone(),
                provider,
                Duration::from_secs(300),
            )
        });
        Self {
            target_root,
            legacy_root,
            status: Mutex::new(status),
            runtime_gate: RuntimeProcessGate::new(),
            migration,
        }
    }

    pub(crate) fn initial_status(&self) -> Result<BootstrapStatus, String> {
        self.status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())?
            .clone()
    }

    fn refresh(&self, restart_required: bool) -> Result<BootstrapStatus, String> {
        let inspection = inspect_startup_roots(&self.target_root, &self.legacy_root, |root| {
            recover_activation(root).map(Into::into)
        })?;
        let status = BootstrapStatus {
            inspection,
            restart_required,
            runtime_state: self.runtime_gate.state()?,
        };
        *self
            .status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())? = Ok(status.clone());
        Ok(status)
    }

    pub(crate) fn begin_runtime_validation(&self) -> Result<(), String> {
        self.runtime_gate.begin_validation()?;
        self.refresh(false).map(|_| ())
    }

    pub(crate) fn complete_runtime_validation(&self, transaction_id: &str) -> Result<(), String> {
        if let Err(error) = complete_runtime_validation(&self.target_root, transaction_id, None) {
            let _ = self.runtime_gate.fail_validation();
            let _ = self.record_runtime_failure(&error);
            return Err(error);
        }
        self.runtime_gate.complete_validation()?;
        let mut status = self.refresh(false)?;
        status.inspection.disposition = StartupDisposition::Ready;
        status.inspection.normal_load_allowed = true;
        status.inspection.reason = Some("current process runtime validation succeeded".to_string());
        status.runtime_state = RuntimeProcessState::Ready;
        *self
            .status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())? = Ok(status);
        Ok(())
    }

    pub(crate) fn fail_runtime_validation(&self, error: &str) -> Result<(), String> {
        self.runtime_gate.fail_validation()?;
        self.record_runtime_failure(error)
    }

    pub(crate) fn business_access_allowed(&self) -> Result<bool, String> {
        self.runtime_gate.business_access_allowed()
    }

    fn record_runtime_failure(&self, error: &str) -> Result<(), String> {
        let mut status = self.refresh(false)?;
        status.inspection.normal_load_allowed = false;
        status.inspection.reason = Some(format!("runtime validation failed: {error}"));
        status.runtime_state = RuntimeProcessState::Failed;
        *self
            .status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())? = Ok(status);
        Ok(())
    }

    fn preview_legacy_migration(&self) -> Result<LegacyMigrationPreview, String> {
        let current = self.refresh(false)?;
        let resumable = current.inspection.disposition == StartupDisposition::RecoveryRequired
            && recover_activation(&self.target_root)?
                == RecoveryDisposition::ContinueBeforeRetirement;
        if current.inspection.disposition != StartupDisposition::LegacyMigrationRequired
            && !resumable
        {
            return Err(
                "legacy migration is not allowed for the current profile state".to_string(),
            );
        }
        self.migration.as_ref().map_err(Clone::clone)?.preview()
    }

    fn confirm_legacy_migration(
        &self,
        preview_id: &str,
        intent: &str,
    ) -> Result<BootstrapStatus, String> {
        let current = self.refresh(false)?;
        let resumable = current.inspection.disposition == StartupDisposition::RecoveryRequired
            && recover_activation(&self.target_root)?
                == RecoveryDisposition::ContinueBeforeRetirement;
        if current.inspection.disposition != StartupDisposition::LegacyMigrationRequired
            && !resumable
        {
            return Err(
                "legacy migration is not allowed for the current profile state".to_string(),
            );
        }
        self.migration
            .as_ref()
            .map_err(Clone::clone)?
            .confirm(preview_id, intent)?;
        let committed = self.refresh(true)?;
        if committed.inspection.disposition != StartupDisposition::RuntimeValidationRequired {
            return Err("legacy migration did not produce a committed profile".to_string());
        }
        Ok(committed)
    }
}

#[tauri::command]
pub(crate) fn get_bootstrap_status(
    state: tauri::State<'_, BootstrapState>,
) -> Result<BootstrapStatus, String> {
    state.initial_status()
}

#[tauri::command]
pub(crate) fn activate_fresh_profile(
    intent: String,
    state: tauri::State<'_, BootstrapState>,
) -> Result<BootstrapStatus, String> {
    if intent != "create_fresh_profile" {
        return Err("explicit fresh-profile intent is required".to_string());
    }
    let current = state.refresh(false)?;
    if current.inspection.disposition != StartupDisposition::FreshActivationRequired {
        return Err("fresh activation is not allowed for the current profile state".to_string());
    }
    let identity = Uuid::new_v4().hyphenated().to_string();
    prepare_fresh_activation(&state.target_root, &identity)?;
    commit_fresh_activation(&state.target_root)?;
    let activated = state.refresh(true)?;
    if activated.inspection.disposition != StartupDisposition::RuntimeValidationRequired {
        return Err("fresh activation did not produce a committed profile".to_string());
    }
    Ok(activated)
}

#[tauri::command]
pub(crate) fn recover_profile_activation(
    intent: String,
    state: tauri::State<'_, BootstrapState>,
) -> Result<BootstrapStatus, String> {
    recover_profile_activation_inner(&intent, &state)
}

#[tauri::command]
pub(crate) fn preview_legacy_migration(
    state: tauri::State<'_, BootstrapState>,
) -> Result<LegacyMigrationPreview, String> {
    state.preview_legacy_migration()
}

#[tauri::command]
pub(crate) fn confirm_legacy_migration(
    preview_id: String,
    intent: String,
    state: tauri::State<'_, BootstrapState>,
) -> Result<BootstrapStatus, String> {
    state.confirm_legacy_migration(&preview_id, &intent)
}

fn recover_profile_activation_inner(
    intent: &str,
    state: &BootstrapState,
) -> Result<BootstrapStatus, String> {
    if intent != "recover_activation" {
        return Err("explicit activation-recovery intent is required".to_string());
    }
    let current = state.refresh(false)?;
    if current.inspection.disposition != StartupDisposition::RecoveryRequired {
        return Err("activation recovery is not required".to_string());
    }
    match recover_activation(&state.target_root)? {
        RecoveryDisposition::ContinueAfterRetirement => {
            commit_migration_activation(&state.target_root, None)?;
        }
        RecoveryDisposition::ValidateCommittedTarget => {
            converge_recovered_committed_target(&state.target_root)?;
        }
        RecoveryDisposition::Complete => {}
        RecoveryDisposition::ContinueBeforeRetirement => {
            if activation_recovery_is_migration(&state.target_root)? {
                return Err(
                    "recovery would retire a legacy identity; controlled stop evidence is required"
                        .to_string(),
                );
            }
            commit_fresh_activation(&state.target_root)?;
        }
        RecoveryDisposition::BlockedInconsistent => {
            return Err("activation recovery evidence is inconsistent".to_string());
        }
    }
    let recovered = state.refresh(true)?;
    if recovered.inspection.disposition != StartupDisposition::RuntimeValidationRequired {
        return Err("activation recovery did not produce a committed profile".to_string());
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::activation_foundation::{
        commit_migration_activation_with_stop_guard, prepare_migration_activation,
        ActivationFailPoint,
    };
    use crate::shared::legacy_migration_entry::{LegacyProcessStopGuard, StopEvidenceState};
    use serde_json::{json, Value};
    use std::fs;
    use std::path::Path;

    struct TestVerifiedStopGuard;

    struct TestRunningProvider;

    struct TestVerifiedProvider;

    impl LegacyProcessStopProvider for TestRunningProvider {
        fn acquire(
            &self,
            _source_root: &Path,
        ) -> Result<crate::shared::legacy_migration_entry::StopEvidenceAcquisition, String>
        {
            Ok(
                crate::shared::legacy_migration_entry::StopEvidenceAcquisition::Blocked(
                    StopEvidenceState::Running,
                ),
            )
        }
    }

    impl LegacyProcessStopProvider for TestVerifiedProvider {
        fn acquire(
            &self,
            _source_root: &Path,
        ) -> Result<crate::shared::legacy_migration_entry::StopEvidenceAcquisition, String>
        {
            Ok(
                crate::shared::legacy_migration_entry::StopEvidenceAcquisition::Verified(Box::new(
                    TestVerifiedStopGuard,
                )),
            )
        }
    }

    impl LegacyProcessStopGuard for TestVerifiedStopGuard {
        fn revalidate(&mut self, _source_root: &Path) -> Result<StopEvidenceState, String> {
            Ok(StopEvidenceState::VerifiedQuiescentWithinSupportedScope)
        }
    }

    fn commit_with_verified_stop(
        target: &Path,
        failpoint: Option<ActivationFailPoint>,
    ) -> Result<(), String> {
        let mut guard = TestVerifiedStopGuard;
        commit_migration_activation_with_stop_guard(target, failpoint, &mut guard)
    }

    fn write_json(path: &Path, value: Value) {
        fs::create_dir_all(path.parent().expect("fixture parent")).unwrap();
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn write_migration_source(source: &Path) {
        fs::create_dir_all(source.join("workspace")).unwrap();
        write_json(
            &source.join("settings.json"),
            serde_json::to_value(crate::types::AppSettings::default()).unwrap(),
        );
        write_json(
            &source.join("workspaces.json"),
            json!([{
                "id":"workspace-1",
                "name":"Workspace",
                "path":source.join("workspace"),
                "kind":"main"
            }]),
        );
        write_json(
            &source.join("remote-host-identity.json"),
            json!({
                "schemaVersion":1,
                "remoteHostIdentity":"6ba7b810-9dad-41d1-80b4-00c04fd430c8"
            }),
        );
    }

    #[test]
    fn product_migration_entry_exposes_sanitized_preview_and_backend_stop_failure() {
        let base = std::env::temp_dir().join(format!(
            "codex-monitor-p4-1d4b-bootstrap-{}",
            Uuid::new_v4()
        ));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        let state = BootstrapState::inspect_with_provider(
            target.clone(),
            Ok(Arc::new(TestRunningProvider)),
        );

        let preview = state.preview_legacy_migration().unwrap();
        let serialized = serde_json::to_string(&preview).unwrap();
        assert!(!serialized.contains(legacy.to_string_lossy().as_ref()));
        assert!(!serialized.contains("remoteHostIdentity"));
        let error = state
            .confirm_legacy_migration(&preview.preview_id, "confirm_legacy_migration")
            .unwrap_err();
        assert!(error.contains("RUNNING"));
        assert!(!target.exists());
        assert!(!state.business_access_allowed().unwrap());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn product_migration_entry_rejects_untrusted_intent_without_process_evidence() {
        let base =
            std::env::temp_dir().join(format!("codex-monitor-p4-1d4b-intent-{}", Uuid::new_v4()));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        let state = BootstrapState::inspect_with_provider(
            target.clone(),
            Ok(Arc::new(TestRunningProvider)),
        );
        let preview = state.preview_legacy_migration().unwrap();

        let error = state
            .confirm_legacy_migration(&preview.preview_id, "confirmedStopped=true")
            .unwrap_err();
        assert!(error.contains("explicit legacy migration intent"));
        assert!(!target.exists());
        let _ = fs::remove_dir_all(base);
    }

    #[cfg(windows)]
    #[test]
    fn product_migration_success_stops_at_restart_required_target_committed() {
        let base =
            std::env::temp_dir().join(format!("codex-monitor-p4-1d4b-success-{}", Uuid::new_v4()));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        let state = BootstrapState::inspect_with_provider(
            target.clone(),
            Ok(Arc::new(TestVerifiedProvider)),
        );
        let preview = state.preview_legacy_migration().unwrap();

        let committed = state
            .confirm_legacy_migration(&preview.preview_id, "confirm_legacy_migration")
            .unwrap();

        assert_eq!(
            committed.inspection.disposition,
            StartupDisposition::RuntimeValidationRequired
        );
        assert!(committed.restart_required);
        assert!(!committed.inspection.normal_load_allowed);
        assert!(!state.business_access_allowed().unwrap());
        let journal: Value = serde_json::from_slice(
            &fs::read(
                base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(journal["state"], "target_committed");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn recovery_adapter_converges_lagging_committed_target_without_opening_business_access() {
        let base =
            std::env::temp_dir().join(format!("codex-monitor-p4-1d4a-recovery-{}", Uuid::new_v4()));
        let target = base.join("io.github.deanxie.codexmonitor");
        let transaction = "tx-lagging-adapter";
        let identity = "014383f2-41f8-4b13-b9d7-30c511e47cec";
        fs::create_dir_all(&target).unwrap();
        write_json(
            &target.join("settings.json"),
            json!({"backendMode":"local"}),
        );
        write_json(&target.join("workspaces.json"), json!([]));
        write_json(
            &target.join("activation-manifest.json"),
            json!({
                "schemaVersion": 1,
                "transactionId": transaction,
                "profileState": "activated"
            }),
        );
        write_json(
            &target.join("remote-host-identity.json"),
            json!({
                "schemaVersion": 2,
                "state": "active",
                "remoteHostIdentity": identity,
                "transactionId": transaction
            }),
        );
        let target_canonical = fs::canonicalize(&target).unwrap();
        write_json(
            &base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            json!({
                "schemaVersion": 1,
                "transactionId": transaction,
                "sourceRoot": "",
                "targetRoot": target_canonical,
                "stagingRoot": base.join("missing-staging"),
                "state": "prepared"
            }),
        );

        let state = BootstrapState::inspect(target.clone());
        let recovered = recover_profile_activation_inner("recover_activation", &state).unwrap();

        assert_eq!(
            recovered.inspection.disposition,
            StartupDisposition::RuntimeValidationRequired
        );
        assert!(!recovered.inspection.normal_load_allowed);
        assert!(!state.business_access_allowed().unwrap());
        let journal: Value = serde_json::from_slice(
            &fs::read(
                base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(journal["state"], "target_committed");
        let identity_after: Value =
            serde_json::from_slice(&fs::read(target.join("remote-host-identity.json")).unwrap())
                .unwrap();
        assert_eq!(identity_after["remoteHostIdentity"], identity);

        let repeated = recover_profile_activation_inner("recover_activation", &state)
            .expect_err("a converged journal is no longer a recovery operation");
        assert!(repeated.contains("not required"));
        let identity_repeated: Value =
            serde_json::from_slice(&fs::read(target.join("remote-host-identity.json")).unwrap())
                .unwrap();
        assert_eq!(identity_repeated["remoteHostIdentity"], identity);
        let _ = fs::remove_dir_all(base);
    }

    #[cfg(windows)]
    #[test]
    fn migration_target_move_with_lagging_journal_converges_without_repeating_migration() {
        let base = std::env::temp_dir().join(format!(
            "codex-monitor-p4-1d4a-migration-{}",
            Uuid::new_v4()
        ));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        crate::shared::migration_core::stage_migration(&legacy, &target).unwrap();
        let prepared = prepare_migration_activation(&legacy, &target).unwrap();
        let expected_identity = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
        let interrupted = commit_with_verified_stop(
            &target,
            Some(ActivationFailPoint::AfterTargetMoveBeforeJournal),
        )
        .expect_err("fixture must interrupt after target move");
        assert!(interrupted.contains("simulated interruption"));

        let state = BootstrapState::inspect(target.clone());
        let initial = state.initial_status().unwrap();
        assert_eq!(
            initial.inspection.disposition,
            StartupDisposition::RecoveryRequired
        );
        assert!(!initial.inspection.normal_load_allowed);
        let recovered = recover_profile_activation_inner("recover_activation", &state).unwrap();

        assert_eq!(
            recovered.inspection.disposition,
            StartupDisposition::RuntimeValidationRequired
        );
        assert!(!state.business_access_allowed().unwrap());
        let journal: Value = serde_json::from_slice(
            &fs::read(
                base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(journal["transactionId"], prepared.transaction_id);
        assert_eq!(journal["state"], "target_committed");
        let target_identity: Value =
            serde_json::from_slice(&fs::read(target.join("remote-host-identity.json")).unwrap())
                .unwrap();
        assert_eq!(target_identity["remoteHostIdentity"], expected_identity);
        let retired_identity: Value =
            serde_json::from_slice(&fs::read(legacy.join("remote-host-identity.json")).unwrap())
                .unwrap();
        assert_eq!(retired_identity["state"], "retired");
        assert_eq!(retired_identity["transactionId"], prepared.transaction_id);
        assert!(!state.business_access_allowed().unwrap());
        let _ = fs::remove_dir_all(base);
    }

    #[cfg(windows)]
    #[test]
    fn migration_recovery_blocks_when_identity_recovery_material_is_missing() {
        let base = std::env::temp_dir().join(format!(
            "codex-monitor-p4-1d4a-missing-recovery-{}",
            Uuid::new_v4()
        ));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        crate::shared::migration_core::stage_migration(&legacy, &target).unwrap();
        prepare_migration_activation(&legacy, &target).unwrap();
        commit_with_verified_stop(
            &target,
            Some(ActivationFailPoint::AfterTargetMoveBeforeJournal),
        )
        .expect_err("fixture must interrupt after target move");
        fs::remove_file(target.join("identity-recovery/remote-host-identity.v2.active.json"))
            .unwrap();

        let state = BootstrapState::inspect(target.clone());
        let blocked = state.initial_status().unwrap();
        assert_eq!(
            blocked.inspection.disposition,
            StartupDisposition::BlockedCorrupt
        );
        assert!(!blocked.inspection.normal_load_allowed);
        assert!(recover_profile_activation_inner("recover_activation", &state).is_err());
        assert!(!state.business_access_allowed().unwrap());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn migration_recovery_before_retirement_requires_new_stop_evidence() {
        let base = std::env::temp_dir().join(format!(
            "codex-monitor-p4-1d4a-stop-evidence-{}",
            Uuid::new_v4()
        ));
        let legacy = base.join("com.dimillian.codexmonitor");
        let target = base.join("io.github.deanxie.codexmonitor");
        write_migration_source(&legacy);
        crate::shared::migration_core::stage_migration(&legacy, &target).unwrap();
        let prepared = prepare_migration_activation(&legacy, &target).unwrap();
        let state = BootstrapState::inspect(target.clone());
        assert_eq!(
            state.initial_status().unwrap().inspection.disposition,
            StartupDisposition::RecoveryRequired
        );

        let error = recover_profile_activation_inner("recover_activation", &state)
            .expect_err("recovery must not reuse old stop evidence");
        assert!(error.contains("controlled stop evidence is required"));
        let journal: Value = serde_json::from_slice(
            &fs::read(
                base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(journal["state"], "prepared");
        assert_eq!(journal["transactionId"], prepared.transaction_id);
        let source_identity: Value =
            serde_json::from_slice(&fs::read(legacy.join("remote-host-identity.json")).unwrap())
                .unwrap();
        assert_eq!(source_identity["schemaVersion"], 1);
        assert!(!state.business_access_allowed().unwrap());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn recovery_adapter_rechecks_changed_evidence_before_writing() {
        let base =
            std::env::temp_dir().join(format!("codex-monitor-p4-1d4a-tamper-{}", Uuid::new_v4()));
        let target = base.join("io.github.deanxie.codexmonitor");
        let transaction = "tx-tamper";
        fs::create_dir_all(&target).unwrap();
        write_json(&target.join("settings.json"), json!({}));
        write_json(&target.join("workspaces.json"), json!([]));
        write_json(
            &target.join("activation-manifest.json"),
            json!({"schemaVersion":1,"transactionId":transaction,"profileState":"activated"}),
        );
        write_json(
            &target.join("remote-host-identity.json"),
            json!({
                "schemaVersion":2,
                "state":"active",
                "remoteHostIdentity":"014383f2-41f8-4b13-b9d7-30c511e47cec",
                "transactionId":transaction
            }),
        );
        write_json(
            &base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            json!({
                "schemaVersion":1,
                "transactionId":transaction,
                "sourceRoot":"",
                "targetRoot":fs::canonicalize(&target).unwrap(),
                "stagingRoot":base.join("missing-staging"),
                "state":"prepared"
            }),
        );
        let state = BootstrapState::inspect(target.clone());
        assert_eq!(
            state.initial_status().unwrap().inspection.disposition,
            StartupDisposition::RecoveryRequired
        );
        write_json(
            &target.join("remote-host-identity.json"),
            json!({
                "schemaVersion":2,
                "state":"active",
                "remoteHostIdentity":"014383f2-41f8-4b13-b9d7-30c511e47cec",
                "transactionId":"changed-after-inspection"
            }),
        );

        let error = recover_profile_activation_inner("recover_activation", &state)
            .expect_err("changed evidence must fail closed");
        assert!(error.contains("not required"));
        let journal: Value = serde_json::from_slice(
            &fs::read(
                base.join(".io.github.deanxie.codexmonitor.codexmonitor-activation-journal.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(journal["state"], "prepared");
        assert!(!state.business_access_allowed().unwrap());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn app_bootstrap_state_rejects_relative_root_without_touching_cwd() {
        let relative = PathBuf::from("relative-app-profile");
        assert!(!relative.exists());
        let state = BootstrapState::inspect(relative.clone());
        let error = state
            .initial_status()
            .expect_err("App bootstrap must reject a relative root");
        assert!(error.contains("absolute data root"));
        assert!(!relative.exists());
    }
}

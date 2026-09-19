use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::shared::activation_foundation::{
    activation_recovery_is_migration, commit_fresh_activation, commit_migration_activation,
    complete_runtime_validation, prepare_fresh_activation, recover_activation, RecoveryDisposition,
    RuntimeProcessGate, RuntimeProcessState,
};
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
}

impl BootstrapState {
    pub(crate) fn inspect(target_root: PathBuf) -> Self {
        let (legacy_root, status) = match legacy_root_for_target(&target_root) {
            Ok(legacy_root) => {
                let status = inspect_startup_roots(&target_root, &legacy_root).map(|inspection| {
                    BootstrapStatus {
                        inspection,
                        restart_required: false,
                        runtime_state: RuntimeProcessState::Blocked,
                    }
                });
                (legacy_root, status)
            }
            Err(error) => (PathBuf::new(), Err(error)),
        };
        Self {
            target_root,
            legacy_root,
            status: Mutex::new(status),
            runtime_gate: RuntimeProcessGate::new(),
        }
    }

    pub(crate) fn initial_status(&self) -> Result<BootstrapStatus, String> {
        self.status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())?
            .clone()
    }

    fn refresh(&self, restart_required: bool) -> Result<BootstrapStatus, String> {
        let inspection = inspect_startup_roots(&self.target_root, &self.legacy_root)?;
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
        RecoveryDisposition::ValidateCommittedTarget | RecoveryDisposition::Complete => {}
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

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::shared::activation_foundation::{
    activation_recovery_is_migration, commit_fresh_activation, commit_migration_activation,
    prepare_fresh_activation, recover_activation, RecoveryDisposition,
};
use crate::shared::startup_activation::{
    inspect_startup_roots, legacy_root_for_target, StartupDisposition, StartupInspection,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapStatus {
    pub(crate) inspection: StartupInspection,
    pub(crate) restart_required: bool,
}

pub(crate) struct BootstrapState {
    target_root: PathBuf,
    legacy_root: PathBuf,
    status: Mutex<Result<BootstrapStatus, String>>,
}

impl BootstrapState {
    pub(crate) fn inspect(target_root: PathBuf) -> Self {
        let (legacy_root, status) = match legacy_root_for_target(&target_root) {
            Ok(legacy_root) => {
                let status = inspect_startup_roots(&target_root, &legacy_root).map(|inspection| {
                    BootstrapStatus {
                        inspection,
                        restart_required: false,
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
        };
        *self
            .status
            .lock()
            .map_err(|_| "bootstrap state lock is poisoned".to_string())? = Ok(status.clone());
        Ok(status)
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
    if activated.inspection.disposition != StartupDisposition::Ready {
        return Err("fresh activation did not produce a valid profile".to_string());
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
        RecoveryDisposition::ContinueAfterRetirement
        | RecoveryDisposition::ValidateCommittedTarget => {
            commit_migration_activation(&state.target_root, None)?;
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
    if recovered.inspection.disposition != StartupDisposition::Ready {
        return Err("activation recovery did not produce a valid profile".to_string());
    }
    Ok(recovered)
}
